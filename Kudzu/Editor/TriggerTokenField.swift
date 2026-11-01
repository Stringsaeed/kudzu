import KudzuCore
import SwiftUI

/// Edits a snippet's triggers as removable capsules followed by a text entry.
///
/// ↵ or space adds the typed trigger, ⌫ in the empty entry removes the last one. Invalid input
/// stays in the entry with an inline explanation instead of being silently dropped.
struct TriggerTokenField: View {
    @Binding var triggers: [String]
    /// Whether duplicates are compared case-sensitively (snippets using exact case).
    var caseSensitive: Bool
    /// Lower-cased triggers that conflict with another snippet; tinted orange.
    var conflicted: Set<String>
    @Binding var focusToken: Int

    @State private var text = ""
    @State private var issue: TriggerIssue?
    @State private var rejectedText: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            FlowLayout(spacing: 6, lineSpacing: 6) {
                ForEach(triggers, id: \.self) { trigger in
                    token(trigger)
                }
                TriggerEntryField(
                    text: $text,
                    focusToken: focusToken,
                    placeholder: triggers.isEmpty ? "Add a trigger, e.g. ;sig" : "Add another…",
                    onSubmit: submit,
                    onDeleteWhenEmpty: removeLast
                )
                .frame(minWidth: 120)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(issue == nil ? Color.secondary.opacity(0.25) : Color.red.opacity(0.7)))
            .contentShape(Rectangle())
            .onTapGesture { focusToken += 1 }

            if let issue {
                Label(issue.message, systemImage: "exclamationmark.circle.fill")
                    .font(.caption)
                    .foregroundStyle(.red)
                    .transition(.opacity)
            }
        }
        .animation(.snappy(duration: 0.15), value: issue)
        .onChange(of: text) { _, value in textChanged(value) }
    }

    private func token(_ trigger: String) -> some View {
        let isConflicted = conflicted.contains(trigger.lowercased())
        return HStack(spacing: 4) {
            Text(trigger)
                .font(.system(.body, design: .monospaced))
                .lineLimit(1)
            Button {
                triggers.removeAll { $0 == trigger }
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 8, weight: .bold))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Remove trigger")
            .accessibilityLabel("Remove \(trigger)")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background((isConflicted ? Color.orange : Color.accentColor).opacity(0.16), in: Capsule())
        .overlay(Capsule().strokeBorder((isConflicted ? Color.orange : Color.accentColor).opacity(0.35)))
    }

    // MARK: - Editing

    private func textChanged(_ value: String) {
        if value != rejectedText {
            rejectedText = nil
            // Warn as soon as it's too long: it can't be added however it's finished.
            issue = value.count > TriggerValidator.maxLength ? .tooLong(length: value.count) : nil
        }
        guard value.contains(where: \.isWhitespace) else { return }
        let parts = TriggerValidator.tokens(from: value)
        if parts.isEmpty {
            text = ""
            issue = .whitespaceOnly
        } else {
            commit(parts)
        }
    }

    private func submit() {
        if text.isEmpty {
            issue = .empty
            return
        }
        let parts = TriggerValidator.tokens(from: text)
        if parts.isEmpty {
            text = ""
            issue = .whitespaceOnly
        } else {
            commit(parts)
        }
    }

    private func commit(_ parts: [String]) {
        var working = triggers
        var firstFailure: (text: String, issue: TriggerIssue)?
        for part in parts {
            switch TriggerValidator.validate(part, existing: working, caseSensitive: caseSensitive) {
            case .success(let trigger): working.append(trigger)
            case .failure(let failure): if firstFailure == nil { firstFailure = (part, failure) }
            }
        }
        if working != triggers { triggers = working }
        if let firstFailure {
            rejectedText = firstFailure.text
            text = firstFailure.text
            issue = firstFailure.issue
        } else {
            text = ""
            issue = nil
        }
    }

    private func removeLast() {
        guard !triggers.isEmpty else { return }
        triggers.removeLast()
        issue = nil
    }
}
