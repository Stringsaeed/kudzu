import AppKit
import KudzuCore
import KudzuDesign
import KudzuEditor
import SwiftUI

/// T8's public detail pane. T12 hosts it with `.environment(model)`; selection comes from AppModel.
public struct SnippetEditorPane: View {
    @Environment(AppModel.self) private var model
    private var onRequestDelete: (([UUID]) -> Void)?
    /// Supply T7's deletion action to keep its selection, focus, and undo toast behavior.
    public init(onRequestDelete: (([UUID]) -> Void)? = nil) { self.onRequestDelete = onRequestDelete }

    public var body: some View {
        let session = model.editorSession
        SwiftUI.Group {
            if let draft = session.draft {
                SnippetEditorContent(session: session, snippet: draft, onRequestDelete: onRequestDelete).id(draft.id)
            } else {
                EditorEmpty()
            }
        }
        .background(Color.kz.background)
        .foregroundStyle(Color.kz.foreground)
        .task { if session.draft == nil { await session.select(model.selectedId) } }
        .onChange(of: model.selectedId) { _, id in
            guard id != session.draft?.id else { return }
            Task { await session.select(id) }
        }
        .onChange(of: model.library) { _, _ in session.reconcile() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { _ in
            Task { await session.flush() }
        }
        .onDisappear { Task { await session.flush() } }
    }
}

private struct SnippetEditorContent: View {
    var session: EditorSession
    var snippet: Snippet
    var onRequestDelete: (([UUID]) -> Void)?
    @Environment(AppModel.self) private var model
    @State private var handle = TemplateEditorHandle()
    @State private var actionError: String?
    @State private var favoriteBurst = 0
    @State private var discardOpen = false
    @State private var insertOpen = false
    @State private var slashAt: Int?
    @State private var chip: TagToken?
    @State private var chipOpen = false
    @State private var showPreview = false
    @State private var anchor = CGRect(x: 12, y: 48, width: 1, height: 20)
    @FocusState private var focus: EditorField?

    private var conflicts: [TriggerConflict] {
        TriggerConflicts.find(snippets: model.library.snippets.map { $0.id == snippet.id ? snippet : $0 }, groups: model.library.groups)
            .filter { $0.snippetID == snippet.id || $0.otherSnippetID == snippet.id }
    }
    private var groups: [GroupNode] { Groups.flatten(Groups.buildTree(model.library.groups)) }
    private var groupPath: String { Groups.groupPath(model.library.groups, id: snippet.groupID).joined(separator: " › ") }
    private var hasTemplate: Bool {
        TemplateParser.parse(snippet.content).nodes.contains { node in
            guard case let .tag(tag) = node else { return false }
            return tag.typed != .unknown
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            GeometryReader { geometry in toolbar(compact: geometry.size.width < 560) }.frame(height: 48)
            Hairline() // `border-b border-hairline`
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if !snippet.isEnabled || !Groups.isGroupEnabled(model.library.groups, id: snippet.groupID) { disabledBanner }
                    if let error = session.error { recovery(error) }
                    header
                    content
                    BehaviorSection(snippet: snippet, groups: model.library.groups) { edit($0) }
                    footer
                }
                .padding(.horizontal, 32).padding(.top, 28).padding(.bottom, 64) // `px-8 pt-7 pb-16`
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
                .rise()
            }
        }
        .font(.system(size: 13))
        .task(id: PreviewKey(content: snippet.content, formatting: snippet.formatting, library: model.library)) {
            guard hasTemplate else { return }
            do {
                try await Task.sleep(for: .milliseconds(120))
                try await session.refreshPreview()
            } catch is CancellationError { } catch { actionError = "Couldn't render preview: \(error.localizedDescription)" }
        }
        .onChange(of: hasTemplate) { _, available in if !available { showPreview = false } }
        .onAppear {
            session.beforeFlush = { [handle] in handle.commitComposition() }
            session.isComposing = { [handle] in handle.isComposing }
        }
        .onDisappear {
            // The text view is still alive here. Commit before the pane's own flush task runs,
            // in case this editor is the one being torn down.
            handle.commitComposition()
            Task { await session.flush() }
        }
        .onChange(of: model.focusNameOf, initial: true) { _, id in
            if id == snippet.id { focus = .name; model.consumeFocusName() }
        }
        .alert("Couldn't complete action", isPresented: Binding(get: { actionError != nil }, set: { if !$0 { actionError = nil } })) {
            Button("OK", role: .cancel) { actionError = nil }
        } message: { Text(actionError ?? "") }
        .confirmationDialog("Discard unsaved changes?", isPresented: $discardOpen, titleVisibility: .visible) {
            Button("Discard Changes", role: .destructive) { session.discard() }
        }
        .background {
            Button("Save") { Task { await session.flush() } }.keyboardShortcut("s").hidden()
        }
    }

    private func toolbar(compact: Bool) -> some View {
        HStack(spacing: 8) {
            Menu { Section("Move to") { groupChoices } } label: {
                HStack(spacing: 5) {
                    Icon.folder.icon(14)
                    if !compact { Text(groupPath.isEmpty ? "No group" : groupPath).lineLimit(1).truncationMode(.middle).frame(maxWidth: 180) }
                }
            }.kzMenuTrigger(.ghost, size: .sm).accessibilityLabel("Move to group")
            Spacer(minLength: 0)
            HStack(spacing: 4) {
                saveIndicator.padding(.trailing, 8)
                Button {
                    if !snippet.isFavorite { favoriteBurst += 1 }
                    withAnimation(.kz.springPop()) { edit { $0.isFavorite.toggle() } }
                } label: {
                    Icon.star.icon(15).symbolVariant(snippet.isFavorite ? .fill : .none)
                        .foregroundStyle(snippet.isFavorite ? Color.kz.favorite : Color.kz.mutedForeground)
                        .starBurst(favoriteBurst)
                }.buttonStyle(.kz(.ghost, size: .iconSm)).keyboardShortcut("f", modifiers: [.command, .shift])
                    .kzHelp("Favorite", shortcut: "mod+shift+f").accessibilityLabel(snippet.isFavorite ? "Remove from favorites" : "Add to favorites")
                enabledSwitch(compact: compact)
                Menu {
                    Button { copy() } label: { Label { Text("Copy expanded snippet") } icon: { Icon.clipboard } }
                    Button { run { guard await session.flush() else { return }; try await model.duplicate(snippet.id) } } label: {
                        Label { Text("Duplicate") } icon: { Icon.copy }
                    }.keyboardShortcut("d")
                    Menu {
                        Button("No group") { edit { $0.groupID = nil } }
                        ForEach(groups, id: \.group.id) { node in
                            Button(indented(node)) { edit { $0.groupID = node.group.id } }
                        }
                    } label: { Label { Text("Move to group") } icon: { Icon.folderInput } }
                    if session.isDirty {
                        Button("Save now") { Task { await session.flush() } }
                        Button("Discard Changes…") { discardOpen = true }.disabled(session.isSaving)
                    }
                    Divider()
                    // ⌘⌫ belongs to the library (it deletes the list selection and leaves text fields
                    // alone), so the item doesn't claim the shortcut itself.
                    Button(role: .destructive) { requestDelete() } label: { Label { Text("Delete") } icon: { Icon.trash2 } }
                } label: { Icon.moreHorizontal.icon(16) }.kzMenuTrigger(.ghost, size: .iconSm).accessibilityLabel("More actions")
            }
        }.padding(.horizontal, 16).frame(height: 48) // `h-12 px-4`
    }

    /// "Saving…" / "Saved" with a checked tick (`animate-check`); always laid out, faded when idle,
    /// so the header never shifts.
    private var saveIndicator: some View {
        let saving = session.isSaving || session.isDirty
        let failed = session.error != nil
        let visible = saving || session.didSave
        return HStack(spacing: 4) {
            if !saving && !failed { Icon.check.icon(12).foregroundStyle(Color.kz.success).checkIn() }
            Text(failed ? "Not saved" : saving ? "Saving…" : "Saved")
        }
        .font(.system(size: 11)).foregroundStyle(Color.kz.faint)
        .opacity(visible ? 1 : 0)
        .kzAnimation(.kz.colors, value: visible)
        .accessibilityHidden(!visible)
        .onChange(of: visible ? (failed ? "Not saved" : saving ? "Saving…" : "Saved") : "") { _, announcement in
            // The web's `aria-live="polite"` on the same indicator.
            if !announcement.isEmpty { AccessibilityNotification.Announcement(announcement).post() }
        }
    }

    /// The switch with its fixed-width state label (`w-12`), toggled from either.
    private func enabledSwitch(compact: Bool) -> some View {
        HStack(spacing: 8) {
            Toggle("Enabled", isOn: binding(\.isEnabled)).labelsHidden().kzSwitch(size: .sm)
                .keyboardShortcut("e", modifiers: [.command, .shift])
            if !compact {
                Text(snippet.isEnabled ? "Enabled" : "Off").frame(width: 48, alignment: .leading)
                    .contentShape(Rectangle())
                    .onTapGesture { edit { $0.isEnabled.toggle() } }
                    .accessibilityHidden(true)
            }
        }
        .font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
        .padding(.horizontal, 8).frame(height: 28).fixedSize()
        .kzHelp("Turn this snippet on or off", shortcut: "mod+shift+e")
    }

    /// The group picker's items: a check on the current group, folders indented by depth.
    @ViewBuilder private var groupChoices: some View {
        Toggle(isOn: inGroup(nil)) { Text("No group") }
        ForEach(groups, id: \.group.id) { node in
            Toggle(isOn: inGroup(node.group.id)) { Label { Text(indented(node)) } icon: { Icon.folder } }
        }
    }

    private func inGroup(_ id: UUID?) -> Binding<Bool> {
        Binding(get: { snippet.groupID == id }, set: { _ in edit { $0.groupID = id } })
    }

    private func indented(_ node: GroupNode) -> String { String(repeating: "    ", count: node.depth) + node.group.name }

    private var disabledBanner: some View {
        HStack(spacing: 8) {
            Circle().fill(Color.kz.faint).frame(width: 6, height: 6)
            Text(!snippet.isEnabled ? "This snippet is turned off. It still shows in search but won't expand."
                 : "This snippet's group “\(groupPath)” is turned off, so it won't expand.")
                .fixedSize(horizontal: false, vertical: true)
        }
        .font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
        .padding(.horizontal, 12).padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.kz.mix(.foreground, alpha: 0.04), in: RoundedRectangle(cornerRadius: Radius.lg))
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 16) {
            TextField("Untitled snippet", text: binding(\.name), prompt: Text("Untitled snippet").foregroundStyle(Color.kz.faint.opacity(0.7)))
                .textFieldStyle(.plain)
                .font(.system(size: 22, weight: .semibold)).tracking(-0.33) // `tracking-[-0.015em]`
                .focused($focus, equals: .name)
                .accessibilityLabel("Snippet name").onSubmit { focus = .trigger }
            VStack(alignment: .leading, spacing: 6) {
                (Text("Triggers").fontWeight(.medium).foregroundStyle(Color.kz.mutedForeground)
                    + Text(" — type any of these to expand").foregroundStyle(Color.kz.faint))
                    .font(.system(size: 12))
                TriggerInput(triggers: snippet.triggers, onChange: { triggers in edit { $0.triggers = triggers } },
                             caseSensitive: snippet.caseMode == .exact, prefix: model.settings.defaultTriggerPrefix,
                             conflicted: Set(conflicts.map { ($0.snippetID == snippet.id ? $0.trigger : $0.otherTrigger).lowercased() }), focus: $focus)
                if snippet.triggers.isEmpty, !snippet.content.isEmpty {
                    Text("Add a trigger so this snippet can expand as you type. You can still insert it from search.").font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
                }
                ForEach(Array(conflicts.enumerated()), id: \.offset) { _, c in
                    let otherID = c.snippetID == snippet.id ? c.otherSnippetID : c.snippetID
                    HStack(alignment: .firstTextBaseline, spacing: 4) {
                        Icon.triangleAlert.icon(14)
                        Text(EditorLogic.conflictMessage(c, snippetID: snippet.id)).fixedSize(horizontal: false, vertical: true)
                        Button { Task { await session.select(otherID) } } label: {
                            Text(model.snippet(otherID)?.name.nonempty ?? "another snippet").fontWeight(.medium).underline()
                        }.buttonStyle(.plain)
                    }.font(.system(size: 12)).foregroundStyle(Color.kz.warning)
                }
            }
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                if hasTemplate {
                    Segmented("Content view", selection: Binding(get: { showPreview }, set: selectPreview), size: .sm, options: [
                        SegmentOption("Editor", value: false), SegmentOption("Preview", value: true),
                    ])
                } else {
                    Text("Expands to").font(.system(size: 12, weight: .medium)).foregroundStyle(Color.kz.mutedForeground)
                }
                Spacer()
                HStack(spacing: 2) {
                    ForEach(Self.quickInserts, id: \.id) { item in
                        if let variable = EditorVariable.all.first(where: { $0.id == item.id }) {
                            MutedGhostButton(size: .iconXs) { pick(variable.choice) } label: { variable.icon.icon() }
                                .kzHelp("Insert \(item.label.lowercased())").accessibilityLabel("Insert \(item.label)")
                        }
                    }
                    MutedGhostButton(size: .xs) {
                        slashAt = nil; anchor = CGRect(x: 12, y: 0, width: 1, height: 24); insertOpen = true
                    } label: {
                        HStack(spacing: 4) { Icon.plus.icon(); Text("Insert"); Shortcut(keys: ["/"]).padding(.leading, 2) }
                    }
                }
                .opacity(showPreview ? 0 : 1)
                .disabled(showPreview)
                .accessibilityHidden(showPreview)
            }
            ZStack(alignment: .topLeading) {
                // Keep the text view mounted so its selection, undo stack and editor mode survive.
                templateEditor
                    .opacity(showPreview ? 0 : 1)
                    .frame(height: showPreview ? 0 : nil, alignment: .top)
                    .disabled(showPreview)
                    .allowsHitTesting(!showPreview)
                    .accessibilityHidden(showPreview)
                if showPreview { previewSection }
            }
        }
    }

    private var templateEditor: some View {
        TemplateEditor(value: binding(\.content), formatting: binding(\.formatting), allowFormatting: true,
                           placeholder: "Type the text to expand. Press / to add a date, clipboard, fill-in field…", handle: handle,
                           snippetName: { id in UUID(uuidString: id).flatMap(model.snippet)?.name.nonempty },
                           onChipClick: { tag, _ in
                               chip = tag; anchor = editorAnchor(tag.from); chipOpen = true
                           }, onSlashOpen: { position, _ in
                               slashAt = position; anchor = editorAnchor(position); insertOpen = true
                           }, minHeight: 168) // `min-h-[168px]`, growing with the text; the pane scrolls
                // `shadow-card`, replaced by the `focus-within` ring while the text has focus.
                .depth(handle.isFocused ? .none : .card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl))
                .overlay { if handle.isFocused { focusRing } }
                .kzAnimation(.kz.colors, value: handle.isFocused)
                .popover(isPresented: $insertOpen, attachmentAnchor: .rect(.rect(anchor)), arrowEdge: .bottom) {
                    InsertMenu(snippets: model.library.snippets, excludeID: snippet.id, pick: pick)
                }
                .popover(isPresented: $chipOpen, attachmentAnchor: .rect(.rect(anchor)), arrowEdge: .bottom) {
                    if let chip {
                        ChipEditor(tag: chip, snippets: model.library.snippets, excludeID: snippet.id, change: { source in
                            let range = handle.replace(chip.from, chip.to, source, focus: false)
                            self.chip = handle.tag(at: range.lowerBound)
                        }, remove: {
                            handle.replace(chip.from, chip.to, ""); chipOpen = false
                        }, done: { chipOpen = false }).id(chip.from)
                    }
                }
                .onChange(of: insertOpen) { _, open in if !open, !showPreview { handle.focus() } }
                .onChange(of: chipOpen) { _, open in if !open { chip = nil; if !showPreview { handle.focus() } } }
    }

    private func selectPreview(_ preview: Bool) {
        if preview {
            handle.commitComposition()
            insertOpen = false
            chipOpen = false
            if NSApp.keyWindow?.firstResponder is NSTextView { NSApp.keyWindow?.makeFirstResponder(nil) }
        }
        showPreview = preview
        Task { @MainActor in
            await Task.yield()
            if showPreview {
                do { try await session.refreshPreview() }
                catch { actionError = "Couldn't render preview: \(error.localizedDescription)" }
            } else { handle.focus() }
        }
    }

    /// The four one-click inserts beside "Expands to", with the web's short labels.
    private static let quickInserts: [(id: String, label: String)] = [("date", "Date"), ("clipboard", "Clipboard"), ("cursor", "Cursor"), ("field", "Fill-in")]

    /// `focus-within:shadow-[0_0_0_1px_var(--ring),0_0_0_4px_var(--iris-soft)]`.
    private var focusRing: some View {
        ZStack {
            RoundedRectangle(cornerRadius: Radius.xl + 4).strokeBorder(Color.kz.irisSoft, lineWidth: 4).padding(-4)
            RoundedRectangle(cornerRadius: Radius.xl + 1).strokeBorder(Color.kz.ring, lineWidth: 1).padding(-1)
        }.allowsHitTesting(false)
    }

    private var previewSection: some View {
        let facts = Template.analyze(snippet.content)
        return VStack(alignment: .leading, spacing: 6) {
            if facts.isForm || facts.usesClipboard {
                FlowLayout(spacing: 8, lineSpacing: 4) {
                    if facts.isForm { FactPill(text: "Asks for \(facts.fieldNames.joined(separator: ", "))", tone: .warning) }
                    if facts.usesClipboard { FactPill(text: "Uses clipboard", tone: .success) }
                }
            }
            SwiftUI.Group {
                if let preview = session.preview { FormattedText(preview).textSelection(.enabled) }
                else { Text("Rendering preview…").foregroundStyle(Color.kz.faint) }
            }.font(.system(size: 13.5)).lineSpacing(6) // `leading-relaxed`
                .frame(maxWidth: .infinity, minHeight: 168, alignment: .topLeading)
                .padding(16)
                .depth(.well, fill: Color.kz.mix(.foreground, alpha: 0.025), in: RoundedRectangle(cornerRadius: Radius.xl))
        }
    }

    private var footer: some View {
        let usage = model.usage[snippet.id]
        return FlowLayout(spacing: 16, lineSpacing: 4) {
            Text("Used \(usage?.count ?? 0) \(usage?.count == 1 ? "time" : "times")")
            if let last = usage?.last, last > 0 { Text("Last used \(Stats.relativeTime(last).lowercased())") }
            Text("Edited \(Stats.relativeTime(snippet.updatedAt.timeIntervalSince1970 * 1000).lowercased())")
            Text("Created \(snippet.createdAt.formatted(date: .abbreviated, time: .omitted))")
        }.font(.system(size: 11)).foregroundStyle(Color.kz.faint).monospacedDigit()
    }

    private func recovery(_ error: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(error).foregroundStyle(Color.kz.destructive)
            HStack {
                Button("Retry Save") { Task { await session.flush() } }.buttonStyle(.kz(size: .sm))
                Button("Discard Changes…") { discardOpen = true }.buttonStyle(.kz(.outline, size: .sm)).disabled(session.isSaving)
            }
        }.font(.system(size: 12))
    }
    private func edit(_ change: (inout Snippet) -> Void) {
        guard session.draft?.id == snippet.id else { return }
        session.update(change)
    }
    private func binding<Value>(_ key: WritableKeyPath<Snippet, Value>) -> Binding<Value> {
        Binding(get: { (session.draft ?? snippet)[keyPath: key] }, set: { value in edit { $0[keyPath: key] = value } })
    }
    private func editorAnchor(_ position: Int) -> CGRect {
        handle.anchorRect(at: position) ?? CGRect(x: 12, y: 0, width: 1, height: 20)
    }
    private func pick(_ choice: EditorInsertChoice) {
        guard !handle.isComposing else { return }
        let range = slashAt.map { handle.replace($0, $0 + 1, choice.text) } ?? handle.insert(choice.text)
        slashAt = nil; insertOpen = false
        if choice.configure, let tag = handle.tag(at: range.lowerBound) {
            Task { @MainActor in
                await Task.yield()
                chip = tag; anchor = editorAnchor(tag.from); chipOpen = true
            }
        }
    }
    private func copy() {
        Task {
            do {
                try await model.backend.copyPreview(content: snippet.content, snippetId: snippet.id, formatting: snippet.formatting)
                toast.success("Copied expanded snippet")
            } catch { toast.error("Couldn't copy snippet", description: error.localizedDescription) }
        }
    }
    /// Deletes right away; the host's toast offers Undo (`requestDelete` in main-app.tsx).
    private func requestDelete() {
        run {
            guard await session.flush() else { return }
            if let onRequestDelete { onRequestDelete([snippet.id]); return }
            LibraryActions(model: model, focus: LibraryFocus()).requestDelete([snippet.id])
        }
    }
    private func run(_ action: @escaping @MainActor () async throws -> Void) {
        Task { do { try await action() } catch { actionError = error.localizedDescription } }
    }
    private struct PreviewKey: Hashable { var content: String; var formatting: Formatting?; var library: Library }
}

/// A preview fact: `rounded bg-warning/12 px-1.5 py-px text-[10.5px] font-medium text-warning`.
private struct FactPill: View {
    var text: String
    var tone: KZToken
    var body: some View {
        Text(text).font(.system(size: 10.5, weight: .medium)).foregroundStyle(Color.kz[tone])
            .padding(.horizontal, 6).padding(.vertical, 1)
            .background(Color.kz.mix(tone, alpha: 0.12), in: RoundedRectangle(cornerRadius: 4))
    }
}

private extension String { var nonempty: String? { isEmpty ? nil : self } }
