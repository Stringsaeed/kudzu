import KudzuCore
import KudzuUI
import SwiftUI

/// Detail pane for the Stats scope: totals, the ten most-used snippets and the typing-speed assumption.
struct StatsView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    var body: some View {
        if snapshotRendering {
            // Offscreen renderers don't draw scroll views' content.
            content.frame(minHeight: 0, maxHeight: .infinity, alignment: .top).clipped()
        } else {
            ScrollView { content }.scrollContentBackground(.hidden)
        }
    }

    private var content: some View {
        let summary = library.summary
        let top = topEntries(summary)
        return VStack(alignment: .leading, spacing: 0) {
            KzPaneHeader(leading: 0, trailing: 0) {
                Text("All time")
                    .font(WorkspaceTheme.title)
                    .foregroundStyle(KzColor.ink)
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: 0)
            }

            ViewThatFits(in: .horizontal) {
                HStack(spacing: 28) { totals(summary) }
                VStack(alignment: .leading, spacing: 18) { totals(summary) }
            }
            .padding(.vertical, 24)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .bottom) { WorkspaceTheme.line.frame(height: 1) }

            WorkspaceSection("Most used") {
                if top.isEmpty {
                    KzEmptyState("No expansions yet", icon: .chart,
                                 message: "Stats appear after Kudzu expands your first snippet. The text you type is never recorded.")
                        .frame(maxWidth: .infinity)
                } else {
                    let maximum = top.map(\.count).max() ?? 1
                    VStack(alignment: .leading, spacing: KzSpace.s1) {
                        ForEach(top) { entry in
                            HStack(spacing: KzSpace.s3) {
                                KzUsageBar(entry.trigger, value: entry.count, maximum: maximum)
                                Text(entry.name)
                                    .font(WorkspaceTheme.body)
                                    .foregroundStyle(KzColor.inkMuted)
                                    .lineLimit(1)
                                    .frame(width: 140, alignment: .leading)
                            }
                            .accessibilityElement(children: .combine)
                        }
                    }
                    .padding(.horizontal, KzSpace.s4)
                    .padding(.vertical, KzSpace.s3)
                }
            }
            .padding(.top, KzSpace.s6)

            HStack(spacing: KzSpace.s2) {
                KzIcon(.keyboard, size: 14)
                Text("Time saved assumes \(library.typingSpeedWPM) words per minute. Change it in Settings › General.")
                    .font(WorkspaceTheme.caption)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .foregroundStyle(KzColor.inkMuted)
            .padding(.top, KzSpace.s4)
            .padding(.horizontal, 2)
        }
        .padding(.horizontal, KzSpace.s6)
        .padding(.bottom, KzSpace.s6)
        .frame(maxWidth: 760, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func totals(_ summary: StatsSummary) -> some View {
        total("Expansions", summary.totalExpansions.formatted())
        total("Characters saved", summary.totalCharsSaved.formatted())
        total("Time saved", Self.format(seconds: summary.estimatedTimeSaved))
    }

    private func total(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(value).font(.system(size: 24, weight: .medium)).monospacedDigit()
                .foregroundStyle(WorkspaceTheme.ink)
            Text(title).font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary)
        }.accessibilityElement(children: .combine)
    }

    private struct Entry: Identifiable {
        var id: UUID
        var trigger: String
        var name: String
        var count: Int
    }

    private func topEntries(_ summary: StatsSummary) -> [Entry] {
        summary.topSnippets(10).compactMap { stats in
            guard let snippet = library.snippetsByID[stats.snippetID] else { return nil }
            let name = snippet.name.isEmpty ? "Untitled" : snippet.name
            return Entry(id: stats.snippetID, trigger: snippet.triggers.first ?? name, name: name, count: stats.count)
        }
    }

    /// "1h 5m", "42s": the two most significant units.
    static func format(seconds: TimeInterval) -> String {
        guard seconds >= 1 else { return "0s" }
        return Duration.seconds(Int(seconds.rounded()))
            .formatted(.units(allowed: [.hours, .minutes, .seconds], width: .narrow, maximumUnitCount: 2))
    }
}

/// List pane while Stats is selected: every used snippet ranked; choosing one opens it.
struct StatsLeaderboardView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    var body: some View {
        let ranked = library.summary.perSnippet.filter { $0.count > 0 && library.snippetsByID[$0.snippetID] != nil }
        HStack {
            Text("Stats").font(WorkspaceTheme.label)
            Spacer()
            Text("Ranked by use").font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary)
        }.padding(.horizontal, 14).frame(height: 52).kzWindowDragArea()
        Group {
            if snapshotRendering {
                VStack(spacing: 1) {
                    ForEach(ranked, id: \.snippetID) { row($0) }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, KzSpace.s2)
            } else {
                // Choosing a row reveals the snippet in the library, so the list keeps no selection of its own.
                List(selection: Binding<UUID?>(get: { nil }, set: { id in if let id { browser.reveal(id, in: library) } })) {
                    ForEach(ranked, id: \.snippetID) { stats in
                        row(stats).kzListRow().tag(stats.snippetID)
                    }
                }
                .kzSelectionList()
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .overlay {
            if ranked.isEmpty {
                KzEmptyState("Nothing used yet", icon: .chart, message: "Used snippets are ranked here.")
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, KzSpace.s8)
            }
        }
    }

    @ViewBuilder
    private func row(_ stats: SnippetStats) -> some View {
        if let snippet = library.snippetsByID[stats.snippetID] {
            let name = snippet.name.isEmpty ? "Untitled" : snippet.name
            HStack {
                VStack(alignment: .leading, spacing: 5) {
                    Text(name).font(WorkspaceTheme.label).lineLimit(1)
                    Text(snippet.triggers.first ?? "No trigger").font(WorkspaceTheme.mono)
                        .foregroundStyle(WorkspaceTheme.secondary)
                    if let last = stats.lastUsedAt {
                        Text("Last used \(last.formatted(.relative(presentation: .named)))")
                            .font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary).lineLimit(1)
                    }
                }
                Spacer()
                Text(stats.count.formatted()).font(WorkspaceTheme.body).monospacedDigit()
            }
            .padding(12).accessibilityElement(children: .combine)

        }
    }
}
