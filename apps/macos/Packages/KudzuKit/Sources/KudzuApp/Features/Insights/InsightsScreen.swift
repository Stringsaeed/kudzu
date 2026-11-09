import KudzuCore
import KudzuDesign
import SwiftUI

/// Compatibility alias for `InsightsScreen`.
public typealias InsightsView = InsightsScreen

/// Local usage insights (`insights.tsx`).
public struct InsightsScreen: View {
    @Environment(AppModel.self) private var model

    public init() {}

    private var report: InsightsLogic.Report {
        InsightsLogic.buildReport(
            library: model.library,
            usage: model.usage,
            wpm: model.settings.typingSpeedWPM
        )
    }

    public var body: some View {
        let current = report
        VStack(spacing: 0) {
            header
            ScrollView {
                // `.stagger` on the column: its four sections rise 30 ms apart.
                VStack(alignment: .leading, spacing: 32) {
                    tiles(current).stagger(0)
                    columns(current).stagger(1)
                    typingSpeed.stagger(2)
                    footer.stagger(3)
                }
                .frame(maxWidth: 880, alignment: .leading)
                .padding(.horizontal, 32)
                .padding(.vertical, 32)
                .frame(maxWidth: .infinity, alignment: .top)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.kz.background)
    }

    private var header: some View {
        HStack {
            Text("Insights")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.kz.foreground)
            Spacer()
        }
        .frame(height: 48)
        .padding(.horizontal, 24)
        .overlay(alignment: .bottom) { Hairline() }
    }

    private func tiles(_ report: InsightsLogic.Report) -> some View {
        let words = InsightsLogic.wordEstimate(chars: report.totals.chars)
        return LazyVGrid(
            columns: Array(repeating: GridItem(.flexible(minimum: 140), spacing: 12), count: 4),
            spacing: 12
        ) {
            tile("Expansions", Stats.formatCount(report.totals.expansions), "\(report.totals.usedSnippets) snippets used")
            tile("Characters saved", Stats.formatCount(report.totals.chars), "≈ \(Stats.formatCount(words)) words")
            tile(
                "Time saved",
                Stats.formatDuration(report.totals.minutesSaved),
                "at \(model.settings.typingSpeedWPM) words per minute"
            )
            tile("Active snippets", String(report.totals.activeSnippets), "\(report.turnedOff) turned off")
        }
    }

    private func tile(_ label: String, _ value: String, _ sub: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.system(size: 12))
                .mutedForeground()
            Text(value)
                .font(.system(size: 26, weight: .semibold).monospacedDigit())
                .tracking(-0.5)
                .foregroundStyle(.kz.foreground)
            Text(sub)
                .font(.system(size: 11))
                .foregroundStyle(.kz.faint)
                .lineLimit(2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
    }

    /// `grid grid-cols-[1.4fr_1fr] gap-6`.
    private func columns(_ report: InsightsLogic.Report) -> some View {
        FractionColumns(fractions: [1.4, 1], spacing: 24) {
            mostUsed(report)
            rightColumn(report)
        }
    }

    private func mostUsed(_ report: InsightsLogic.Report) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Most used")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.kz.foreground)
                .accessibilityAddTraits(.isHeader)

            if report.top.isEmpty {
                Text("Expand a snippet and it'll show up here.")
                    .font(.system(size: 14))
                    .mutedForeground()
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                    .padding(24)
                    .background(Color.kz.mix(.foreground, alpha: 0.03), in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
            } else {
                VStack(spacing: 2) {
                    ForEach(Array(report.top.enumerated()), id: \.element.id) { index, row in
                        Button {
                            open(row.snippet.id)
                        } label: {
                            HStack(alignment: .center, spacing: 12) {
                                Text("\(index + 1)")
                                    .font(.system(size: 11).monospacedDigit())
                                    .foregroundStyle(.kz.faint)
                                    .frame(width: 18, alignment: .leading)
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack(spacing: 8) {
                                        Text(row.snippet.name.isEmpty ? "Untitled" : row.snippet.name)
                                            .font(.system(size: 13))
                                            .foregroundStyle(.kz.foreground)
                                            .lineLimit(1)
                                        if let trigger = InsightsLogic.primaryTrigger(for: row.snippet) {
                                            TriggerChip(trigger, muted: true)
                                        }
                                    }
                                    InsightBar(
                                        fraction: InsightsLogic.barFraction(count: row.usage.count, maxCount: report.maxCount),
                                        rank: index
                                    )
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                Text(Stats.formatCount(row.usage.count))
                                    .font(.system(size: 12).monospacedDigit())
                                    .mutedForeground()
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                        }
                        .buttonStyle(InsightRowStyle())
                    }
                }
            }
        }
    }

    private func rightColumn(_ report: InsightsLogic.Report) -> some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 12) {
                Text("Recently used")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.kz.foreground)
                    .accessibilityAddTraits(.isHeader)
                if report.recent.isEmpty {
                    Text("Nothing yet.")
                        .font(.system(size: 12))
                        .mutedForeground()
                        .padding(.horizontal, 8)
                } else {
                    VStack(spacing: 2) {
                        ForEach(report.recent) { row in
                            Button {
                                open(row.snippet.id)
                            } label: {
                                HStack(spacing: 8) {
                                    Text(row.snippet.name.isEmpty ? "Untitled" : row.snippet.name)
                                        .font(.system(size: 13))
                                        .foregroundStyle(.kz.foreground)
                                        .lineLimit(1)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                    Text(Stats.relativeTime(row.usage.last))
                                        .font(.system(size: 11))
                                        .foregroundStyle(.kz.faint)
                                }
                                .padding(.horizontal, 8)
                                .padding(.vertical, 6)
                            }
                            .buttonStyle(InsightRowStyle())
                        }
                    }
                }
            }

            if !report.unused.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("Never used")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(.kz.foreground)
                        Text("\(report.unused.count)")
                            .font(.system(size: 12))
                            .foregroundStyle(.kz.faint)
                    }
                    .accessibilityAddTraits(.isHeader)
                    FlowLayout(spacing: 6, lineSpacing: 6) {
                        ForEach(report.unused.prefix(InsightsLogic.unusedLimit), id: \.id) { snippet in
                            Button {
                                open(snippet.id)
                            } label: {
                                Text(unusedLabel(snippet))
                                    .font(.system(size: 12))
                                    .lineLimit(1)
                            }
                            .buttonStyle(UnusedChipStyle())
                        }
                    }
                    Text("Forgotten triggers? Try the quick-search tray, or tidy these up.")
                        .font(.system(size: 11))
                        .foregroundStyle(.kz.faint)
                }
            }
        }
    }

    private var typingSpeed: some View {
        let wpm = model.settings.typingSpeedWPM
        return HStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Your typing speed")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.kz.foreground)
                Text("Used to estimate time saved.")
                    .font(.system(size: 12))
                    .mutedForeground()
            }
            Spacer(minLength: 0)
            HStack(spacing: 12) {
                Slider(
                    value: Binding(
                        get: { Double(wpm) },
                        set: { next in
                            let clamped = InsightsLogic.clampWPM(Int(next.rounded()))
                            Task { await model.patchSettings { $0.typingSpeedWPM = clamped } }
                        }
                    ),
                    in: 10...150,
                    step: 5
                )
                .tint(Color.kz.iris)
                .frame(width: 192)
                .accessibilityLabel("Typing speed in words per minute")
                Text("\(wpm) wpm")
                    .font(.system(size: 14).monospacedDigit())
                    .foregroundStyle(.kz.foreground)
                    .frame(width: 64, alignment: .trailing)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
    }

    private var footer: some View {
        HStack(spacing: 6) {
            Icon.lock.icon(12)
            Text("Kudzu counts expansions only. What you type is never recorded or sent anywhere.")
                .font(.system(size: 11))
        }
        .foregroundStyle(.kz.faint)
        .frame(maxWidth: .infinity)
    }

    private func unusedLabel(_ snippet: Snippet) -> String {
        if !snippet.name.isEmpty { return snippet.name }
        return snippet.triggers.first ?? "Untitled"
    }

    private func open(_ id: UUID) {
        model.setView(.all)
        model.select(id)
    }
}

/// `Item … hover:bg-foreground/[0.04]` (`rounded-lg`, `transition-colors duration-100`).
private struct InsightRowStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        InsightRow(label: configuration.label)
    }

    private struct InsightRow<Label: View>: View {
        var label: Label
        @State private var hovered = false

        var body: some View {
            let shape = RoundedRectangle(cornerRadius: Radius.lg, style: .continuous)
            label
                .background(hovered ? Color.kz.mix(.foreground, alpha: 0.04) : Color.clear, in: shape)
                .contentShape(shape)
                .onHover { hovered = $0 }
                .kzAnimation(.kz.ease(duration: 0.1), value: hovered)
        }
    }
}

/// The "Never used" chips: `rounded-md bg-surface-raised px-2 py-1 text-xs text-muted-foreground
/// shadow-raised press hover:text-foreground hover:shadow-raised-hover active:shadow-pressed`.
private struct UnusedChipStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        Chip(label: configuration.label, pressed: configuration.isPressed)
    }

    private struct Chip<Label: View>: View {
        var label: Label
        var pressed: Bool
        @State private var hovered = false

        var body: some View {
            let shape = RoundedRectangle(cornerRadius: Radius.md, style: .continuous)
            label
                .foregroundStyle(hovered ? Color.kz.foreground : Color.kz.mutedForeground)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .depth(pressed ? .pressed : (hovered ? .raisedHover : .raised), fill: Color.kz.surfaceRaised, in: shape)
                .contentShape(shape)
                .onHover { hovered = $0 }
                .themeAwareAnimation(.kz.hover, value: hovered)
                .themeAwareAnimation(.kz.hover, value: pressed)
                .press(pressed)
        }
    }
}

/// Columns sized like CSS grid `fr` tracks: the width left after the gaps, split by `fractions`.
private struct FractionColumns: Layout {
    var fractions: [CGFloat]
    var spacing: CGFloat

    private func widths(_ total: CGFloat, count: Int) -> [CGFloat] {
        let parts = Array(fractions.prefix(count))
        let sum = parts.reduce(0, +)
        let free = max(0, total - spacing * CGFloat(max(0, count - 1)))
        return parts.map { sum > 0 ? free * $0 / sum : 0 }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 640
        let columns = widths(width, count: subviews.count)
        let height = zip(subviews, columns).map { $0.sizeThatFits(ProposedViewSize(width: $1, height: nil)).height }.max() ?? 0
        return CGSize(width: width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX
        for (subview, width) in zip(subviews, widths(bounds.width, count: subviews.count)) {
            subview.place(at: CGPoint(x: x, y: bounds.minY), anchor: .topLeading, proposal: ProposedViewSize(width: width, height: nil))
            x += width + spacing
        }
    }
}

#Preview("Insights") {
    let model = AppModel(backend: MockBackend(), isDev: true)
    InsightsScreen()
        .environment(model)
        .frame(width: 960, height: 680)
        .task { await model.start() }
}
