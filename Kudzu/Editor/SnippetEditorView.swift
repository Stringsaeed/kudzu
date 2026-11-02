import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// The detail pane for one snippet. Recreate with `.id(snippet.id)` when the selection changes.
///
/// A floating toolbar (favorite, debug, more), the name as a mono title, Triggers, Content, Preview,
/// then Rules and Advanced as disclosures, and a footer of dates and use count. Edits go to
/// `SnippetEditorModel`, which saves explicitly or when leaving the editor.
struct SnippetEditorView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    /// Identifies this editor; the library reconciles stored changes into the retained model.
    let snippet: SnippetRecord
    @State private var model: SnippetEditorModel
    @State private var triggerAnchor = ViewAnchor()
    @State private var editor = TemplateEditorController()
    @FocusState private var nameFocused: Bool
    @AppStorage("editor.rulesExpanded") private var rulesExpanded = false
    @AppStorage("editor.advancedExpanded") private var advancedExpanded = false
    @AppStorage("editor.previewExpanded") private var previewExpanded = false

    init(snippet: SnippetRecord, library: LibraryModel) {
        self.snippet = snippet
        _model = State(initialValue: library.editor(for: snippet))
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            if model.isDirty || model.saveError != nil {
                VStack(alignment: .leading, spacing: KzSpace.s2) {
                    Text(model.saveError ?? "Unsaved changes. Leaving the editor also attempts to save.")
                        .kzTextStyle(.caption)
                    HStack {
                        KzButton("Save", size: .sm) { model.flush() }
                            .keyboardShortcut("s", modifiers: .command)
                        KzButton("Discard Changes", variant: .ghost, size: .sm) { model.discardChanges() }
                    }
                }
                .padding(.horizontal, KzSpace.s4)
                .padding(.vertical, KzSpace.s2)
            }
            if snapshotRendering {
                // Offscreen renderers don't draw scroll views' content.
                form.frame(minHeight: 0, maxHeight: .infinity, alignment: .top).clipped()
            } else {
                ScrollView { form }
                    .scrollContentBackground(.hidden)
            }
            footer
        }
        .onAppear {
            model.start()
            consumeFocusRequest()
        }
        .onDisappear { model.stop() }
    }

    private var form: some View {
        let draft = model.draft
        return VStack(alignment: .leading, spacing: KzSpace.s5) {
            nameField(draft)
            triggersField(draft)
            contentField
            KzDisclosure("Preview", isExpanded: $previewExpanded) {
                SnippetPreviewView(content: draft.content, rootSnippetID: snippet.id, showsLabel: false)
            }
            KzDisclosure("Rules", summary: rulesSummary(draft), isExpanded: $rulesExpanded) {
                RulesSection(
                    rule: model.binding(\.rule),
                    insertion: model.binding(\.insertion),
                    inherited: library.tree.inheritedSettings(forGroup: draft.groupID),
                    defaultInsertion: AppMatchingSettings.shared.defaultInsertion
                )
            }
            KzDisclosure("Advanced", summary: AdvancedSection.summary(for: draft), isExpanded: $advancedExpanded) {
                AdvancedSection(model: model)
            }
        }
        .padding(.horizontal, KzSpace.s4)
        .padding(.top, KzSpace.s2)
        .padding(.bottom, KzSpace.s6)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Header

    private var header: some View {
        let draft = model.draft
        return KzPaneHeader {
            HStack(spacing: 6) {
                KzIcon(.folder, size: 12)
                Text(library.groupPath(of: draft) ?? "No group")
                    .kzTextStyle(.caption)
                    .lineLimit(1)
                    .truncationMode(.head)
            }
            .foregroundStyle(KzColor.inkMuted)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Group: \(library.groupPath(of: draft) ?? "none")")
            Spacer(minLength: 0)
            KzToolbarGroup(label: "Snippet actions") {
                KzIconButton(draft.isFavorite ? .starFill : .star,
                             label: draft.isFavorite ? "Remove from favorites" : "Add to favorites",
                             tone: .bloom) {
                    model.edit { $0.isFavorite.toggle() }
                }
                KzIconButton(.bug, label: "Debug", caret: true, menuTitle: "Debug") {
                    KzMenuItem("Copy expanded text", icon: .copy) { copyExpandedText() }
                    KzMenuItem("Copy template source", icon: .braces) { copy(model.draft.content) }
                }
                KzToolbarSeparator()
                KzIconButton(.more, label: "More", placement: .pullDown(.trailing)) { moreMenu }
            }
        }
    }

    @KzMenuBuilder
    private var moreMenu: [KzMenuElement] {
        let draft = model.draft
        let groups = library.tree.allGroups
        KzMenuItem("Duplicate", icon: .copy) {
            let copies = library.duplicate([snippet.id])
            if !copies.isEmpty { browser.selection = Set(copies) }
        }
        .keyboardShortcut("d")
        KzMenuItem("Favorite", icon: .star, isChecked: draft.isFavorite) { model.edit { $0.isFavorite.toggle() } }
        KzMenuItem("Enabled", icon: draft.isEnabled ? .pause : .play, isChecked: draft.isEnabled) {
            model.edit { $0.isEnabled.toggle() }
        }
        KzSubmenu("Move to group", icon: .folder) {
            KzMenuItem("No group", isChecked: draft.groupID == nil) { model.edit { $0.groupID = nil } }
            if !groups.isEmpty { KzMenuSeparator() }
            for group in groups {
                KzMenuItem(library.tree.path(to: group.id).joined(separator: " / "), isChecked: draft.groupID == group.id) {
                    model.edit { $0.groupID = group.id }
                }
            }
        }
        KzMenuSeparator()
        KzMenuItem("Delete…", icon: .trash, role: .danger) { browser.requestDelete([snippet.id], in: library) }
            .keyboardShortcut(.delete, modifiers: [])
    }

    // MARK: - Fields

    @ViewBuilder
    private func nameField(_ draft: SnippetRecord) -> some View {
        if snapshotRendering {
            Text(draft.name.isEmpty ? "Untitled" : draft.name)
                .kzTextStyle(.titleLg)
                .foregroundStyle(KzColor.ink)
                .lineLimit(2)
        } else {
            TextField("Name", text: model.binding(\.name), prompt: Text("Untitled").foregroundStyle(KzColor.inkFaint))
                .textFieldStyle(.plain)
                .kzTextStyle(.titleLg)
                .foregroundStyle(KzColor.ink)
                .focused($nameFocused)
                .accessibilityLabel("Name")
        }
    }

    private func triggersField(_ draft: SnippetRecord) -> some View {
        let notes = conflictNotes(for: draft)
        let caseSensitive = draft.caseMode == .exact
        return KzField("Triggers", aside: "Space or return to add") {
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                KzTriggerInput(
                    triggers: model.binding(\.triggers),
                    placeholder: draft.triggers.isEmpty ? "Add a trigger, e.g. ;sig" : "Add trigger…"
                ) { candidate in
                    Self.rejection(for: candidate, existing: model.draft.triggers, caseSensitive: caseSensitive)
                }
                .viewAnchor(triggerAnchor)
                if draft.triggers.isEmpty && draft.isEnabled {
                    EditorNote(icon: .warning, tone: .warning, text: AttributedString("Add a trigger, or this snippet will never expand."))
                }
                ForEach(notes) { note in
                    ConflictRow(note: note) { browser.reveal(note.otherID, in: library) }
                }
            }
        }
    }

    /// The same checks the old token field made: no spaces, the length limit, and duplicates (case-insensitive unless exact).
    static func rejection(for candidate: String, existing: [String], caseSensitive: Bool) -> String? {
        if candidate.contains(where: \.isWhitespace) { return "A trigger can't contain spaces." }
        switch TriggerValidator.validate(candidate, existing: existing, caseSensitive: caseSensitive) {
        case .success: return nil
        case .failure(let issue): return issue.message
        }
    }

    private var contentField: some View {
        KzField("Content") {
            KzEditorFrame(minHeight: 160, isFocused: editor.isFocused) {
                InsertTagButton { editor.insert($0) }
            } content: {
                if snapshotRendering {
                    Text(model.draft.content)
                        .kzTextStyle(.content)
                        .foregroundStyle(KzColor.ink)
                        .padding(.horizontal, KzEditorStyle.containerInset.width)
                        .padding(.vertical, KzEditorStyle.containerInset.height)
                        .frame(maxWidth: .infinity, alignment: .topLeading)
                } else {
                    TemplateTextEditor(source: model.binding(\.content), snippets: library.snippets, currentSnippetID: snippet.id)
                        .frame(minHeight: 180, idealHeight: 220)
                        .templateEditorController(editor)
                }
            }
        }
    }

    // MARK: - Footer

    private var footer: some View {
        let draft = model.draft
        let uses = library.stats[draft.id]?.count ?? 0
        let format = Date.FormatStyle.dateTime.day().month(.abbreviated).year()
        let text = "Created \(draft.createdAt.formatted(format)) · Edited \(draft.updatedAt.formatted(format)) · "
            + "\(uses.formatted()) \(uses == 1 ? "expansion" : "expansions")"
        return Text(text)
            .kzTextStyle(.caption)
            .foregroundStyle(KzColor.inkMuted)
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, KzSpace.s3)
            .overlay(alignment: .top) { Rectangle().fill(KzColor.line).frame(height: 1) }
            .padding(.horizontal, KzSpace.s4)
    }

    // MARK: - Summaries

    private func rulesSummary(_ draft: SnippetRecord) -> String {
        guard let rule = draft.rule else {
            return library.tree.inheritedSettings(forGroup: draft.groupID).ruleDescription { AppCatalog.info(for: $0).name }
        }
        let count = rule.bundleIDs.count
        let apps = "\(count) app\(count == 1 ? "" : "s")"
        switch rule.mode {
        case .allow: return count == 0 ? "Expands nowhere" : "Only in \(apps)"
        case .deny: return count == 0 ? "Works in every app" : "Blocked in \(apps)"
        }
    }

    // MARK: - Actions

    private func copyExpandedText() {
        let result = SnippetPreviewView.render(model.draft.content, rootSnippetID: snippet.id, contents: library.contentByID)
        copy(result.text)
    }

    private func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    /// New snippets ask for focus before the editor exists; honor it once it appears.
    private func consumeFocusRequest() {
        guard let target = library.pendingFocus else { return }
        library.pendingFocus = nil
        Task {
            // The field isn't a first-responder candidate until the window has laid it out.
            try? await Task.sleep(for: .milliseconds(150))
            switch target {
            case .name: nameFocused = true
            case .trigger: focusTriggerField()
            }
        }
    }

    /// `KzTriggerInput`'s entry is an `NSTextField` labelled "Add trigger" for VoiceOver.
    private func focusTriggerField() {
        guard let field = triggerAnchor.nearest(NSTextField.self, where: { $0.accessibilityLabel() == "Add trigger" }) else { return }
        field.window?.makeFirstResponder(field)
    }

    // MARK: - Conflicts

    /// Builds a one-line message where the triggers are set in mono, in `vine-ink`.
    private static func sentence(code first: String, _ middle: String, code second: String? = nil, _ tail: String = "") -> AttributedString {
        func code(_ text: String) -> AttributedString {
            var part = AttributedString(text)
            part.font = KzTextStyle.trigger.font
            return part
        }
        var result = code(first) + AttributedString(middle)
        if let second { result += code(second) + AttributedString(tail) }
        return result
    }

    private func conflictNotes(for draft: SnippetRecord) -> [ConflictNote] {
        library.conflicts(for: draft.id).compactMap { conflict in
            let iAmFirst = conflict.snippetID == draft.id
            let otherID = iAmFirst ? conflict.otherSnippetID : conflict.snippetID
            guard let other = library.snippetsByID[otherID] else { return nil }
            let mine = iAmFirst ? conflict.trigger : conflict.otherTrigger
            let theirs = iAmFirst ? conflict.otherTrigger : conflict.trigger
            let text: AttributedString
            switch (conflict.kind, iAmFirst) {
            case (.shadowing, true): text = Self.sentence(code: mine, " blocks ", code: theirs, " (\(other.name))")
            case (.shadowing, false): text = Self.sentence(code: mine, " is blocked by ", code: theirs, " (\(other.name))")
            case (.duplicate, _): text = Self.sentence(code: mine, " is also used by \(other.name)")
            }
            return ConflictNote(
                id: "\(conflict.kind)-\(conflict.snippetID)-\(conflict.trigger)-\(conflict.otherSnippetID)-\(conflict.otherTrigger)",
                text: text, otherID: otherID
            )
        }
    }
}

private struct ConflictNote: Identifiable {
    var id: String
    var text: AttributedString
    var otherID: UUID
}

/// A trigger conflict; choosing it shows the other snippet.
private struct ConflictRow: View {
    var note: ConflictNote
    var jump: () -> Void

    var body: some View {
        Button(action: jump) {
            HStack(spacing: 6) {
                EditorNote(icon: .warning, tone: .warning, text: note.text)
                KzIcon(.chevronRight, size: 12).foregroundStyle(KzColor.inkMuted)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(NoteButtonStyle())
        .help("Show the other snippet")
    }

    private struct NoteButtonStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            KzInteractive(RoundedRectangle(cornerRadius: KzRadius.sm), isPressed: configuration.isPressed) { _ in
                configuration.label.padding(.horizontal, 4).padding(.vertical, 2)
            }
            .padding(.horizontal, -4)
        }
    }
}

/// A one-line note under a field: an icon in its tone and `callout` text.
struct EditorNote: View {
    enum Tone { case muted, warning, danger }

    var icon: KzIconName
    var tone: Tone
    var text: AttributedString

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            KzIcon(icon, size: 14)
                .foregroundStyle(iconColor)
                .padding(.top, 1)
            Text(text)
                .kzTextStyle(.callout)
                .foregroundStyle(tone == .danger ? KzColor.danger : KzColor.ink)
                .fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
    }

    private var iconColor: Color {
        switch tone {
        case .muted: KzColor.inkMuted
        case .warning: KzColor.warning
        case .danger: KzColor.danger
        }
    }
}

/// The editor frame's Insert pull-down: every tag the `/` menu offers, grouped.
private struct InsertTagButton: View {
    let insert: @MainActor (TemplateInsertItem) -> Void

    var body: some View {
        KzButton("Insert", variant: .ghost, size: .sm, icon: .plus, iconEnd: .chevronDown, menuTitle: "Insert tag") {
            KzMenuHeader("Dates")
            item(.date)
            item(.time)
            KzMenuHeader("Text")
            item(.clipboard)
            item(.cursor)
            KzMenuHeader("Ask when expanding")
            item(.field)
            item(.choice)
            KzMenuHeader("Snippets")
            item(.snippet)
        }
        .help("Insert a date, field, snippet or other tag")
    }

    private func item(_ item: TemplateInsertItem) -> KzMenuItem {
        KzMenuItem(item.menuTitle, icon: item.kzIcon) { insert(item) }
    }
}

extension TemplateInsertItem {
    /// The menu title in sentence case.
    var menuTitle: String {
        switch self {
        case .date: "Date"
        case .time: "Time"
        case .clipboard: "Clipboard"
        case .cursor: "Cursor position"
        case .field: "Text field…"
        case .choice: "Dropdown…"
        case .snippet: "Snippet…"
        }
    }

    /// The Kudzu glyph for the item.
    var kzIcon: KzIconName {
        switch self {
        case .date: .clock
        case .time: .timer
        case .clipboard: .copy
        case .cursor: .cursor
        case .field: .type
        case .choice: .list
        case .snippet: .textPlus
        }
    }
}
