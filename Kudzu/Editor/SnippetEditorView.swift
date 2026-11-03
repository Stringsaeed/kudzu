import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// The detail pane for one snippet. Recreate with `.id(snippet.id)` when the selection changes.
///
/// A document surface with an optional preview and options drawer. The retained model owns explicit saves and navigation guards.
struct SnippetEditorView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser
    @Environment(\.kzSnapshotRendering) private var snapshotRendering
    @Environment(\.workspaceInspectorSnapshot) private var inspectorSnapshot

    /// Identifies this editor; the library reconciles stored changes into the retained model.
    let snippet: SnippetRecord
    @State private var model: SnippetEditorModel
    @State private var triggerAnchor = ViewAnchor()
    @State private var editor = TemplateEditorController()
    @FocusState private var nameFocused: Bool
    @State private var inspectorPresented = false
    @AppStorage("editor.previewExpanded") private var previewExpanded = false

    init(snippet: SnippetRecord, library: LibraryModel) {
        self.snippet = snippet
        _model = State(initialValue: library.editor(for: snippet))
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            form
                .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
                .overlay(alignment: .trailing) {
                    if snapshotRendering && inspectorSnapshot { inspector }
                }
            footer
        }
        .onAppear {
            model.start()
            installEditorCommands()
            consumeFocusRequest()
        }
        .onDisappear {
            browser.removeLeaveGuard(owner: snippet.id)
            if browser.editorBridge?.owner == snippet.id {
                browser.editorBridge = nil
                browser.editorCanDiscard = false
            }
            model.stop()
        }
        .onChange(of: model.draft) { _, _ in
            let dirty = model.isDirty || browser.uncommittedTriggers[snippet.id] != nil
            browser.editorCanDiscard = dirty
        }
        .onChange(of: browser.uncommittedTriggers[snippet.id]) { _, entry in
            browser.editorCanDiscard = model.isDirty || entry != nil
        }
        .onChange(of: library.pendingFocus) { _, target in
            guard target != nil else { return }
            consumeFocusRequest()
        }
    }

    /// Saves before navigation, retaining failed drafts and invalid trigger input in place.
    private func installEditorCommands() {
        browser.installLeaveGuard(owner: snippet.id) {
            browser.commitTriggerEntry(snippet.id, in: library) && model.flush()
        }
        browser.editorCanDiscard = model.isDirty || browser.uncommittedTriggers[snippet.id] != nil
        browser.editorBridge = EditorCommandBridge(
            owner: snippet.id,
            save: saveChanges,
            copyExpanded: copyExpandedText,
            copySource: { copy(model.draft.content) },
            discard: discardChanges,
            toggleFavorite: { model.edit { $0.isFavorite.toggle() } },
            delete: { browser.requestDelete([snippet.id], in: library) }
        )
    }

    private func saveChanges() {
        guard browser.commitTriggerEntry(snippet.id, in: library), model.flush() else { return }
        browser.editorCanDiscard = false
        browser.completeDeferredNavigation()
    }

    private func discardChanges() {
        guard model.discardChanges() else { return }
        browser.clearTriggerEntry(snippet.id)
        browser.editorCanDiscard = false
        browser.completeDeferredNavigation()
    }

    private var form: some View {
        VStack(alignment: .leading, spacing: 12) {
            nameField(model.draft)
            WorkspaceTriggerArea { triggersField(model.draft) }
            contentField
        }
        .padding(.horizontal, 24)
        .padding(.top, 18)
        .padding(.bottom, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var inspector: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Snippet options").font(WorkspaceTheme.label)
                Spacer()
                WorkspaceIconButton("xmark", "Close snippet options") {
                    inspectorPresented = false
                }
            }
            .padding(12)
            Divider()
            if snapshotRendering {
                inspectorContent.frame(maxHeight: .infinity, alignment: .top).clipped()
            } else {
                ScrollView { inspectorContent }
            }
        }
        .frame(width: 310)
        .frame(maxHeight: .infinity)
        .background(WorkspaceTheme.navigation)
        .overlay(alignment: .leading) { WorkspaceTheme.line.frame(width: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Snippet options")
    }

    private var inspectorContent: some View {
        VStack(alignment: .leading, spacing: 24) {
            WorkspaceSection("Rules", description: rulesSummary(model.draft)) {
                RulesSection(
                    rule: model.binding(\.rule), insertion: model.binding(\.insertion),
                    inherited: library.tree.inheritedSettings(forGroup: model.draft.groupID),
                    defaultInsertion: AppMatchingSettings.shared.defaultInsertion
                )
            }
            WorkspaceSection("Advanced") { AdvancedSection(model: model) }
            Text("Created \(model.draft.createdAt.formatted())\nEdited \(model.draft.updatedAt.formatted())")
                .font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary)
                .textSelection(.enabled)
        }
        .padding(16)
    }

    // MARK: - Header

    private var header: some View {
        let draft = model.draft
        let groupName = draft.groupID.flatMap { library.tree.group($0)?.name } ?? "No group"
        let groupPath = library.groupPath(of: draft) ?? "No group"
        let mainTrigger = draft.triggers.first ?? "No trigger"
        return HStack(spacing: 4) {
            WorkspaceIconButton("doc.on.doc", "Duplicate (⌘D)") {
                let copies = browser.duplicateSnippets([snippet.id], in: library)
                if !copies.isEmpty { browser.replaceSelection(Set(copies)) }
            }
            WorkspaceMenu {
                Button { model.edit { $0.groupID = nil } } label: {
                    if draft.groupID == nil { Label("No group", systemImage: "checkmark") }
                    else { Text("No group") }
                }
                Divider()
                ForEach(library.tree.allGroups) { group in
                    Button { model.edit { $0.groupID = group.id } } label: {
                        let path = library.tree.path(to: group.id).joined(separator: " / ")
                        if draft.groupID == group.id { Label(path, systemImage: "checkmark") }
                        else { Text(path) }
                    }
                }
            } label: { Image(systemName: "folder").frame(width: 28, height: 28) }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .tint(WorkspaceTheme.secondary)
            .help("Move to group").accessibilityLabel("Move to group")
            WorkspaceIconButton(draft.isEnabled ? "pause" : "play", draft.isEnabled ? "Disable snippet" : "Enable snippet") {
                model.edit { $0.isEnabled.toggle() }
            }
            .accessibilityValue(draft.isEnabled ? "Enabled" : "Disabled")
            WorkspaceIconButton("slider.horizontal.3", "Rules and advanced options") {
                inspectorPresented.toggle()
            }
            .accessibilityValue(inspectorPresented ? "Shown" : "Hidden")
            .popover(isPresented: $inspectorPresented, arrowEdge: .bottom) {
                inspector.frame(height: 420)
            }
            Spacer(minLength: 8)
            HStack(spacing: 6) {
                Text(groupName).lineLimit(1).truncationMode(.head).help(groupPath)
                Text("/").accessibilityHidden(true)
                Text(mainTrigger).font(WorkspaceTheme.mono).lineLimit(1).help(mainTrigger)
                WorkspaceIconButton(draft.isFavorite ? "star.fill" : "star", draft.isFavorite ? "Remove from favorites" : "Add to favorites") {
                    model.edit { $0.isFavorite.toggle() }
                }
            }
            .font(WorkspaceTheme.caption)
            .foregroundStyle(WorkspaceTheme.secondary)
        }
        .padding(.horizontal, 14).frame(height: 52)
        .overlay(alignment: .bottom) { WorkspaceTheme.line.frame(height: 1) }
        .kzWindowDragArea()
    }

    // MARK: - Fields

    @ViewBuilder
    private func nameField(_ draft: SnippetRecord) -> some View {
        if snapshotRendering {
            Text(draft.name.isEmpty ? "Untitled" : draft.name)
                .font(WorkspaceTheme.title)
                .foregroundStyle(KzColor.ink)
                .lineLimit(2)
        } else {
            TextField("Name", text: model.binding(\.name), prompt: Text("Untitled").foregroundStyle(KzColor.inkFaint))
                .textFieldStyle(.plain)
                .font(WorkspaceTheme.title)
                .foregroundStyle(KzColor.ink)
                .focused($nameFocused)
                .accessibilityIdentifier("kudzu.editor.name")
                .accessibilityLabel("Name")
        }
    }

    private func triggersField(_ draft: SnippetRecord) -> some View {
        let notes = conflictNotes(for: draft)
        let caseSensitive = draft.caseMode == .exact
        return VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                KzTriggerInput(
                    triggers: model.binding(\.triggers),
                    placeholder: draft.triggers.isEmpty ? "Add a trigger, e.g. ;sig" : "Add trigger…",
                    unboxed: true,
                    draft: browser.triggerText(for: snippet.id),
                    error: browser.triggerError(for: snippet.id),
                    onMove: { forward in
                        library.pendingFocus = forward ? .content : .name
                    }
                ) { candidate in
                    Self.rejection(for: candidate, existing: model.draft.triggers, caseSensitive: caseSensitive)
                }
                .viewAnchor(triggerAnchor)
                .help("Triggers — Space, Tab or Return to add")
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
        LibraryModel.triggerRejection(for: candidate, existing: existing, caseSensitive: caseSensitive)
    }

    private var contentField: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text("Content").font(WorkspaceTheme.label)
                    .foregroundStyle(WorkspaceTheme.secondary).accessibilityAddTraits(.isHeader)
                InsertTagButton { editor.insert($0) }
                Spacer(minLength: 0)
                Button { previewExpanded.toggle() } label: {
                    Label("Preview", systemImage: previewExpanded ? "eye.fill" : "eye")
                }
                .buttonStyle(.plain).font(WorkspaceTheme.caption)
                .foregroundStyle(previewExpanded ? WorkspaceTheme.accent : WorkspaceTheme.secondary)
                .accessibilityLabel(previewExpanded ? "Hide preview" : "Show preview")
                .accessibilityValue(previewExpanded ? "Shown" : "Hidden")
            }
            .padding(.vertical, 10)
            // This editor keeps its structural identity when preview and options change.
            Group {
                if snapshotRendering {
                    Text(model.draft.content)
                        .font(.system(size: 14)).foregroundStyle(WorkspaceTheme.ink)
                        .padding(.vertical, 12)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                } else {
                    TemplateTextEditor(source: model.binding(\.content), snippets: library.snippets, currentSnippetID: snippet.id)
                        .templateEditorController(editor)
                }
            }
            .frame(minHeight: 100, maxHeight: .infinity)
            if previewExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Expanded text").font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary)
                    if snapshotRendering {
                        SnippetPreviewView(content: model.draft.content, rootSnippetID: snippet.id, showsLabel: false)
                    } else {
                        ScrollView {
                            SnippetPreviewView(content: model.draft.content, rootSnippetID: snippet.id, showsLabel: false)
                        }
                    }
                }
                .padding(.top, 12)
                .frame(maxHeight: 160)
                .overlay(alignment: .top) { WorkspaceTheme.line.frame(height: 1) }
                .accessibilityElement(children: .contain).accessibilityLabel("Expanded text preview")
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
        let hasChanges = model.isDirty || browser.uncommittedTriggers[snippet.id] != nil
        return VStack(alignment: .leading, spacing: 0) {
            if let error = model.saveError {
                Text(error)
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(KzColor.danger)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
                    .padding(.horizontal, KzSpace.s4)
                    .padding(.top, KzSpace.s3)
            }
            HStack(spacing: KzSpace.s2) {
                if hasChanges || model.saveError != nil {
                    HStack(spacing: 6) {
                        Circle().fill(model.saveError == nil ? KzColor.inkMuted : KzColor.danger)
                            .frame(width: 5, height: 5)
                            .accessibilityHidden(true)
                        Text(model.saveError == nil ? "Edited" : "Not saved")
                            .font(WorkspaceTheme.caption)
                    }
                    .foregroundStyle(KzColor.inkMuted)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(model.saveError == nil ? "Unsaved changes" : "Changes could not be saved")
                    Spacer(minLength: KzSpace.s2)
                    Button("Discard", action: discardChanges).buttonStyle(.plain).padding(.horizontal, 6)
                        .accessibilityLabel("Discard changes")
                        .help("Discard unsaved changes")
                    Button("Save", action: saveChanges).buttonStyle(.borderedProminent).controlSize(.small).tint(WorkspaceTheme.accent)
                        .accessibilityLabel("Save changes")
                        .help("Save changes (⌘S)")
                } else {
                    Text(text)
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                        .help(text)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(.horizontal, KzSpace.s4)
            .frame(height: 38)
        }
        .background(KzColor.surfaceRaised)
        .overlay(alignment: .top) { Rectangle().fill(KzColor.line).frame(height: 1) }
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
        let result = SnippetPreviewView.render(model.draft.content, rootSnippetID: snippet.id,
                                             contents: library.contentByID, placeholders: false)
        copy(result.text)
    }

    private func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    /// New snippets ask for focus before the editor exists; honor it once it appears.
    ///
    /// The first try is immediate. A second try waits for layout, and only runs when the field
    /// still isn't focused, so a later keystroke is not pulled back.
    private func consumeFocusRequest() {
        guard let target = library.pendingFocus else { return }
        library.pendingFocus = nil
        if applyFocus(target) { return }
        let responderAtAttempt = NSApp.keyWindow?.firstResponder
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(150))
            if isFocused(target) { return }
            // The field took focus and the user has since moved to another text control.
            let responderNow = NSApp.keyWindow?.firstResponder
            if responderNow !== responderAtAttempt, responderNow is NSTextView { return }
            applyFocus(target)
        }
    }

    @discardableResult
    private func applyFocus(_ target: EditorFocusTarget) -> Bool {
        switch target {
        case .name:
            nameFocused = true
            return nameFieldIsFirstResponder()
        case .trigger:
            return focusTriggerField()
        case .content:
            return editor.focus()
        }
    }

    private func isFocused(_ target: EditorFocusTarget) -> Bool {
        switch target {
        case .name: return nameFieldIsFirstResponder()
        case .trigger: return triggerField()?.currentEditor() != nil
        case .content: return editor.isFocused
        }
    }

    private func nameFieldIsFirstResponder() -> Bool {
        guard let window = triggerAnchor.nearest(NSView.self)?.window,
              let editor = window.firstResponder as? NSTextView,
              let field = editor.delegate as? NSTextField else { return false }
        return field.accessibilityIdentifier() == "kudzu.editor.name" || field.accessibilityLabel() == "Name"
    }

    /// `KzTriggerInput`'s entry is an `NSTextField` labelled "Add trigger" for VoiceOver.
    @discardableResult
    private func focusTriggerField() -> Bool {
        guard let field = triggerField() else { return false }
        return field.window?.makeFirstResponder(field) ?? false
    }

    private func triggerField() -> NSTextField? {
        triggerAnchor.nearest(NSTextField.self, where: {
            $0.accessibilityIdentifier() == "kudzu.trigger" || $0.accessibilityLabel() == "Add trigger"
        })
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
        WorkspaceMenu {
            Section("Dates") { item(.date); item(.time) }
            Section("Text") { item(.clipboard); item(.cursor) }
            Section("Ask when expanding") { item(.field); item(.choice) }
            Section("Snippets") { item(.snippet) }
        } label: {
            HStack(spacing: 4) {
                Label("Insert tag", systemImage: "plus")
                Image(systemName: "chevron.down").font(.system(size: 8))
            }.font(WorkspaceTheme.caption)
        }
        .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
        .foregroundStyle(WorkspaceTheme.secondary)
        .help("Insert a date, field, snippet or other tag")
    }

    private func item(_ item: TemplateInsertItem) -> some View {
        Button(item.menuTitle) { insert(item) }
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
