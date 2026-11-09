import KudzuCore
import KudzuDesign
import SwiftUI

/// The right-hand "Will insert" pane (`palette.tsx`'s preview aside).
struct PalettePreviewPane: View {
    var palette: PaletteModel
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let theme = KZTheme(scheme)
        VStack(spacing: 0) {
            if let snippet = palette.current {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        Text("Will insert")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(theme[.faint])
                            .padding(.bottom, 8)
                        previewBody(snippet, theme: theme)
                        formNote(snippet, theme: theme)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 20)
                    .padding(.top, 16)
                    .padding(.bottom, 12)
                }
                facts(snippet, theme: theme)
            } else {
                Text(palette.createRow ? "Opens Kudzu with a new snippet" : "")
                    .font(.system(size: 12))
                    .foregroundStyle(theme[.faint])
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme[.foreground].opacity(0.015))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Preview")
    }

    @ViewBuilder private func previewBody(_ snippet: Snippet, theme: KZTheme) -> some View {
        if let preview = palette.preview {
            if preview.text.isEmpty {
                Text("Empty snippet").font(.system(size: 13.5)).foregroundStyle(theme[.faint])
            } else {
                FormattedText(preview)
                    .font(.system(size: 13.5))
                    .lineSpacing(6)
                    .foregroundStyle(theme[.foreground].opacity(0.9))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
        } else {
            // `block h-4 w-2/3`.
            GeometryReader { geo in
                Skeleton(radius: 4, strength: 0.08)
                    .frame(width: geo.size.width * 2 / 3, height: 16)
            }
            .frame(height: 16)
        }
    }

    @ViewBuilder private func formNote(_ snippet: Snippet, theme: KZTheme) -> some View {
        let facts = Template.analyze(snippet.content)
        if facts.isForm {
            HStack(spacing: 6) {
                Icon.textCursorInput.icon(12)
                Text("Asks for \(facts.fieldNames.joined(separator: ", "))")
            }
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(theme[.warning])
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(theme[.warning, alpha: 0.12], in: RoundedRectangle(cornerRadius: Radius.md))
            .padding(.top, 12)
        }
    }

    private func facts(_ snippet: Snippet, theme: KZTheme) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            factRow("Triggers", theme: theme) {
                if snippet.triggers.isEmpty {
                    Text("None").foregroundStyle(theme[.mutedForeground])
                } else {
                    FlowChips(triggers: snippet.triggers)
                }
            }
            if snippet.groupID != nil {
                factRow("Group", theme: theme) {
                    HStack(spacing: 4) {
                        Icon.folder.icon(12)
                        Text(palette.path(of: snippet)).lineLimit(1)
                    }
                    .foregroundStyle(theme[.mutedForeground])
                }
            }
            factRow("Used", theme: theme) {
                Text(palette.usedLabel(snippet))
                    .foregroundStyle(theme[.mutedForeground])
                    .monospacedDigit()
            }
        }
        .font(.system(size: 11.5))
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
        .overlay(alignment: .top) { Rectangle().fill(theme[.hairline]).frame(height: 1) }
    }

    private func factRow(_ title: String, theme: KZTheme, @ViewBuilder value: () -> some View) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 16) {
            Text(title).foregroundStyle(theme[.faint]).frame(width: 58, alignment: .leading)
            value().frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// Wrapping trigger chips. A flow layout, because the web wraps them and a single `HStack` clips.
private struct FlowChips: View {
    var triggers: [String]

    var body: some View {
        FlexibleChips(triggers: triggers)
    }
}

private struct FlexibleChips: View {
    var triggers: [String]
    @State private var width: CGFloat = 0

    var body: some View {
        let rows = Self.rows(triggers, width: width)
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                HStack(spacing: 4) {
                    ForEach(row, id: \.self) { trigger in
                        TriggerChip(trigger, muted: true)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            GeometryReader { proxy in
                Color.clear.preference(key: ChipWidthKey.self, value: proxy.size.width)
            }
        }
        .onPreferenceChange(ChipWidthKey.self) { width = $0 }
    }

    /// Greedy wrap. Chip width is a stand-in (mono 11.5 pt plus padding) so the first layout pass
    /// still wraps instead of drawing one endless line.
    static func rows(_ triggers: [String], width: CGFloat) -> [[String]] {
        let limit = width > 1 ? width : 220
        var rows: [[String]] = [[]]
        var used: CGFloat = 0
        for trigger in triggers {
            let chip = CGFloat(trigger.count) * 7 + 16
            if used > 0, used + chip > limit {
                rows.append([trigger])
                used = chip
            } else {
                rows[rows.count - 1].append(trigger)
                used += chip + 4
            }
        }
        return rows
    }
}

private struct ChipWidthKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}
