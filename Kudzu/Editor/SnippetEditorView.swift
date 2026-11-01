import KudzuCore
import SwiftUI

/// Editor pane for one snippet. Recreate with `.id(snippet.id)` when the selection changes.
struct SnippetEditorView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser

    /// The stored version, used to pick up outside changes.
    let snippet: SnippetRecord
    @State private var model: SnippetEditorModel
    @State private var triggerFocusToken = 0
    @FocusState private var nameFocused: Bool
    @AppStorage("editor.rulesExpanded") private var rulesExpanded = false
    @AppStorage("editor.advancedExpanded") private var advancedExpanded = false

    init(snippet: SnippetRecord, library: LibraryModel) {
        self.snippet = snippet
        _model = State(initialValue: SnippetEditorModel(snippet: snippet, library: library))
    }

    var body: some View {
        let draft = model.draft
        let notes = conflictNotes(for: draft)
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                section("Name") {
                    TextField("Name", text: model.binding(\.name))
                        .textFieldStyle(.roundedBorder)
                        .focused($nameFocused)
                        .labelsHidden()
                }

                section("Triggers") {
                    TriggerTokenField(
                        triggers: model.binding(\.triggers),
                        caseSensitive: draft.caseMode == .exact,
                        conflicted: Set(notes.map { $0.myTrigger.lowercased() }),
                        focusToken: $triggerFocusToken
                    )
                    if draft.triggers.isEmpty && draft.isEnabled {
                        Label("Add a trigger, or this snippet will never expand.", systemImage: "exclamationmark.triangle.fill")
                            .font(.callout)
                            .foregroundStyle(.orange)
                    }
                    ForEach(notes) { note in
                        ConflictRow(note: note) { browser.reveal(note.otherID, in: library) }
                    }
                }

                section("Content") {
                    TemplateTextEditor(source: model.binding(\.content), snippets: library.snippets, currentSnippetID: snippet.id)
                        .frame(minHeight: 180, idealHeight: 220)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Color.secondary.opacity(0.25)))
                }

                SnippetPreviewView(content: draft.content, rootSnippetID: snippet.id)

                DisclosureGroup("Rules", isExpanded: $rulesExpanded) {
                    RulesSection(model: model)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, 10)
                }
                .font(.headline)

                DisclosureGroup("Advanced", isExpanded: $advancedExpanded) {
                    AdvancedSection(model: model)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, 10)
                }
                .font(.headline)
            }
            .padding(20)
            .frame(maxWidth: 760, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .scrollContentBackground(.hidden)
        .onAppear {
            model.start()
            consumeFocusRequest()
        }
        .onDisappear { model.stop() }
        .onChange(of: snippet) { _, newValue in model.adoptExternal(newValue) }
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.headline)
            content()
        }
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
            case .trigger: triggerFocusToken += 1
            }
        }
    }

    // MARK: - Conflicts

    /// Builds a one-line message where the triggers are set in monospace.
    private static func sentence(code first: String, _ middle: String, code second: String? = nil, _ tail: String = "") -> AttributedString {
        func code(_ text: String) -> AttributedString {
            var part = AttributedString(text)
            part.font = .system(.callout, design: .monospaced)
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
                text: text, myTrigger: mine, otherID: otherID
            )
        }
    }
}

private struct ConflictNote: Identifiable {
    var id: String
    var text: AttributedString
    var myTrigger: String
    var otherID: UUID
}

private struct ConflictRow: View {
    var note: ConflictNote
    var jump: () -> Void

    var body: some View {
        Button(action: jump) {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(note.text)
                Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.tertiary)
            }
            .font(.callout)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Show the other snippet")
    }
}
