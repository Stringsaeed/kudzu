import Charts
import KudzuCore
import SwiftUI

/// Detail pane for the Stats scope: totals, the ten most-used snippets and the typing-speed assumption.
struct StatsView: View {
    @Environment(LibraryModel.self) private var library

    private struct Entry: Identifiable {
        var id: UUID
        var label: String
        var count: Int
    }

    var body: some View {
        let summary = library.summary
        let entries = topEntries(summary)
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(spacing: 12) {
                    StatCard(title: "Expansions", value: summary.totalExpansions.formatted(), systemImage: "text.insert")
                    StatCard(title: "Characters saved", value: summary.totalCharsSaved.formatted(), systemImage: "character.cursor.ibeam")
                    StatCard(title: "Time saved", value: Self.format(seconds: summary.estimatedTimeSaved), systemImage: "timer")
                }

                VStack(alignment: .leading, spacing: 10) {
                    Text("Most used").font(.headline)
                    if entries.isEmpty {
                        ContentUnavailableView(
                            "No Expansions Yet",
                            systemImage: "chart.bar",
                            description: Text("Stats appear after Kudzu expands your first snippet. The text you type is never recorded.")
                        )
                        .frame(maxWidth: .infinity, minHeight: 200)
                    } else {
                        Chart(entries) { entry in
                            BarMark(x: .value("Expansions", entry.count), y: .value("Snippet", entry.label))
                                .foregroundStyle(Color.accentColor.gradient)
                                .cornerRadius(4)
                                // Name and count sit after the bar: the default axis labels collide with short bars.
                                .annotation(position: .trailing, alignment: .leading, spacing: 8) {
                                    HStack(spacing: 6) {
                                        Text(entry.label).lineLimit(1)
                                        Text(entry.count.formatted()).foregroundStyle(.secondary)
                                    }
                                    .font(.callout.monospacedDigit())
                                    .fixedSize()
                                }
                        }
                        .chartYScale(domain: entries.map(\.label))
                        .chartXScale(domain: 0...(Double(entries.map(\.count).max() ?? 1) * 1.8))
                        .chartYAxis(.hidden)
                        .chartXAxis(.hidden)
                        .frame(height: max(140, CGFloat(entries.count) * 32))
                        .accessibilityLabel("Bar chart of the most used snippets")
                    }
                }

                Label {
                    Text("Time saved assumes you type \(library.typingSpeedWPM) words per minute. Change it in Settings.")
                } icon: {
                    Image(systemName: "keyboard")
                }
                .font(.callout)
                .foregroundStyle(.secondary)
            }
            .padding(24)
            .frame(maxWidth: 760, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .navigationTitle("Stats")
    }

    private func topEntries(_ summary: StatsSummary) -> [Entry] {
        var labels: [String] = []
        return summary.topSnippets(10).compactMap { stats in
            guard let snippet = library.snippetsByID[stats.snippetID] else { return nil }
            // Chart categories must be unique, but two snippets can share a name.
            let label = UniqueName.make(base: snippet.name.isEmpty ? "Untitled" : snippet.name, existing: labels)
            labels.append(label)
            return Entry(id: stats.snippetID, label: label, count: stats.count)
        }
    }

    /// "1h 5m", "42s": the two most significant units.
    static func format(seconds: TimeInterval) -> String {
        guard seconds >= 1 else { return "0s" }
        return Duration.seconds(Int(seconds.rounded()))
            .formatted(.units(allowed: [.hours, .minutes, .seconds], width: .narrow, maximumUnitCount: 2))
    }
}

private struct StatCard: View {
    var title: String
    var value: String
    var systemImage: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(title, systemImage: systemImage)
                .font(.callout)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(size: 26, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .minimumScaleFactor(0.6)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 12))
        .accessibilityElement(children: .combine)
    }
}

/// Content column while Stats is selected: every used snippet ranked, click to open it.
struct StatsLeaderboardView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser

    var body: some View {
        let ranked = library.summary.perSnippet.filter { $0.count > 0 }
        List {
            ForEach(Array(ranked.enumerated()), id: \.element.id) { index, stats in
                if let snippet = library.snippetsByID[stats.snippetID] {
                    Button {
                        browser.reveal(snippet.id, in: library)
                    } label: {
                        HStack(spacing: 10) {
                            Text("\(index + 1)")
                                .font(.callout.monospacedDigit())
                                .foregroundStyle(.tertiary)
                                .frame(width: 26, alignment: .trailing)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(snippet.name.isEmpty ? "Untitled" : snippet.name).lineLimit(1)
                                if let last = stats.lastUsedAt {
                                    Text("Last used \(last.formatted(.relative(presentation: .named)))")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Spacer()
                            Text(stats.count.formatted())
                                .font(.callout.monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .overlay {
            if ranked.isEmpty {
                ContentUnavailableView("Nothing Used Yet", systemImage: "chart.bar",
                                       description: Text("Used snippets are ranked here."))
            }
        }
        .navigationTitle("Stats")
        .navigationSubtitle("Ranked by use")
    }
}
