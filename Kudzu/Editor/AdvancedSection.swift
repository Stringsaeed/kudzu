import KudzuCore
import SwiftUI

/// Firing and matching options, plus read-only metadata and usage.
struct AdvancedSection: View {
    @Environment(LibraryModel.self) private var library
    var model: SnippetEditorModel

    var body: some View {
        let snippet = model.draft
        let stats = library.stats[snippet.id]
        VStack(alignment: .leading, spacing: 12) {
            LabeledContent("Fire") {
                Picker("Fire", selection: model.binding(\.fireMode)) {
                    Text("Immediately").tag(FireMode.immediate)
                    Text("Whole word only").tag(FireMode.wholeWord)
                }
                .labelsHidden()
                .fixedSize()
            }
            .help("Immediately expands the moment the trigger is typed. Whole word only waits for a space or punctuation and keeps it.")

            LabeledContent("Case") {
                Picker("Case", selection: model.binding(\.caseMode)) {
                    Text("Match my capitalization").tag(CaseMode.propagate)
                    Text("Exact").tag(CaseMode.exact)
                }
                .labelsHidden()
                .fixedSize()
            }
            .help("Match my capitalization turns ;Sig into “Best regards” and ;SIG into “BEST REGARDS”.")

            Toggle("Enabled", isOn: model.binding(\.isEnabled))
            Toggle("Favorite", isOn: model.binding(\.isFavorite))

            Divider()

            LabeledContent("Created") { Text(snippet.createdAt.formatted(date: .abbreviated, time: .shortened)) }
            LabeledContent("Updated") { Text(snippet.updatedAt.formatted(date: .abbreviated, time: .shortened)) }
            LabeledContent("Used") {
                Text(stats.map { "\($0.count.formatted()) time\($0.count == 1 ? "" : "s")" } ?? "Never")
            }
            LabeledContent("Characters saved") { Text((stats?.charsSaved ?? 0).formatted()) }
            LabeledContent("Last used") {
                if let last = stats?.lastUsedAt {
                    Text(last.formatted(.relative(presentation: .named)))
                } else {
                    Text("Never")
                }
            }
        }
        .foregroundStyle(.primary)
    }
}
