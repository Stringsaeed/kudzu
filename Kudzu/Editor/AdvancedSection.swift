import KudzuCore
import KudzuUI
import SwiftUI

/// Firing and matching options, the enabled and favorite switches, and read-only usage.
struct AdvancedSection: View {
    @Environment(LibraryModel.self) private var library
    var model: SnippetEditorModel

    private static let labelWidth: CGFloat = 120

    var body: some View {
        let snippet = model.draft
        let stats = library.stats[snippet.id]
        VStack(alignment: .leading, spacing: KzSpace.s3) {
            RulesOptionRow("Fire", width: Self.labelWidth) {
                KzSelect("Fire", selection: model.binding(\.fireMode), options: [
                    (FireMode.immediate, "Immediately"),
                    (FireMode.wholeWord, "Whole word only"),
                ])
                .help("Immediately expands the moment the trigger is typed. Whole word only waits for a space or punctuation and keeps it.")
            }
            RulesOptionRow("Case", width: Self.labelWidth) {
                KzSelect("Case", selection: model.binding(\.caseMode), options: [
                    (CaseMode.propagate, "Match my capitalization"),
                    (CaseMode.exact, "Exact"),
                ])
                .help("Match my capitalization turns ;Sig into “Best regards” and ;SIG into “BEST REGARDS”.")
            }
            RulesOptionRow("Enabled", width: Self.labelWidth) {
                KzToggle("Enabled", isOn: model.binding(\.isEnabled))
            }
            RulesOptionRow("Favorite", width: Self.labelWidth) {
                KzToggle("Favorite", isOn: model.binding(\.isFavorite))
            }

            Rectangle().fill(KzColor.line).frame(height: 1)

            value("Characters saved", (stats?.charsSaved ?? 0).formatted())
            value("Last used", stats?.lastUsedAt.map { $0.formatted(.relative(presentation: .named)) } ?? "Never")
        }
    }

    private func value(_ label: String, _ value: String) -> some View {
        RulesOptionRow(label, width: Self.labelWidth) {
            Text(value)
                .kzTextStyle(.body)
                .monospacedDigit()
                .foregroundStyle(KzColor.inkMuted)
        }
        .accessibilityElement(children: .combine)
    }

    /// The one-line summary shown while the disclosure is closed.
    static func summary(for snippet: SnippetRecord) -> String {
        var parts = [
            snippet.fireMode == .wholeWord ? "Whole word" : "Immediately",
            snippet.caseMode == .exact ? "exact case" : "matches capitalization",
        ]
        if !snippet.isEnabled { parts.append("disabled") }
        return parts.joined(separator: ", ")
    }
}
