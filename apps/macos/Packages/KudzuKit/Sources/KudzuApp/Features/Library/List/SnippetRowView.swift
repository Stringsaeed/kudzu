// One snippet row (`Row` in `snippet-list.tsx`): name with search marks, trigger, summary, usage.
// The row is 60 pt. Selection and the keyboard cursor belong to the native `List`. The row draws the
// highlight: gray in a multi-selection, iris for the selected row, never the system accent color.

import KudzuCore
import KudzuDesign
import SwiftUI

struct SnippetRowView: View {
    var hit: SearchHit
    var usage: Int
    var summary: String
    var groupOff: Bool
    /// Part of the selection (`inSelection`).
    var inSelection = false
    /// The row the editor shows (`selected`).
    var selected = false
    /// The list is focused or holds one selection (`active`).
    var active = true
    @State private var hovered = false

    var body: some View {
        let snippet = hit.snippet
        let off = !snippet.isEnabled || groupOff
        let untitled = snippet.name.isEmpty
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                title(snippet.name.isEmpty ? "Untitled" : snippet.name, hits: untitled ? [] : hit.nameHits, off: off, untitled: untitled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if off {
                    Icon.circleOff.icon(12).foregroundStyle(Color.kz.faint).accessibilityLabel("Turned off")
                }
                if let trigger = snippet.triggers.first, !trigger.isEmpty {
                    HStack(spacing: 2) {
                        Text(trigger).lineLimit(1)
                        if snippet.triggers.count > 1 {
                            Text("+\(snippet.triggers.count - 1)").foregroundStyle(Color.kz.faint)
                        }
                    }
                    .font(.kz.mono(11.5))
                    .foregroundStyle(off ? Color.kz.faint : Color.kz.irisForeground.opacity(0.9))
                    .frame(maxWidth: 140, alignment: .trailing)
                }
            }
            HStack(spacing: 8) {
                Text(summary.isEmpty ? "Empty" : summary)
                    .font(.system(size: 12))
                    .italic(summary.isEmpty)
                    .foregroundStyle(Color.kz.mutedForeground.opacity(0.8))
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if usage > 0 {
                    Text("\(Stats.formatCount(usage))×")
                        .font(.system(size: 10.5).monospacedDigit())
                        .foregroundStyle(Color.kz.faint)
                }
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 60)
        .background { highlight }
        .opacity(off ? 0.92 : 1)
        .onHover { hovered = $0 }
        .animation(.easeOut(duration: 0.075), value: hovered) // `duration-75`
        .kzOwnSelection()
        .accessibilityElement(children: .combine)
        .accessibilityLabel(snippet.name.isEmpty ? "Untitled" : snippet.name)
        .accessibilityValue(snippet.isFavorite ? "Favorite" : "")
    }

    /// `hover:bg-foreground/[0.035]`; `bg-foreground/[0.06] shadow-selected` in the selection;
    /// `bg-iris-soft` with an inset `ring-iris/35` on the selected row while the list is active.
    @ViewBuilder private var highlight: some View {
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        if selected && active {
            Color.clear
                .depth(.selected, fill: Color.kz.irisSoft, in: shape)
                .overlay(shape.strokeBorder(Color.kz.iris.opacity(0.35), lineWidth: 1))
        } else if inSelection {
            Color.clear.depth(.selected, fill: Color.kz.foreground.opacity(hovered ? 0.07 : 0.06), in: shape)
        } else if hovered {
            shape.fill(Color.kz.foreground.opacity(0.035))
        }
    }

    private func title(_ text: String, hits: [Int], off: Bool, untitled: Bool) -> some View {
        HitText(text: text, hits: hits)
            .font(.system(size: 13, weight: untitled ? .regular : .medium))
            .italic(untitled)
            .foregroundStyle(off || untitled ? Color.kz.mutedForeground : Color.kz.foreground)
            .lineLimit(1)
    }
}

/// Search marks (`Highlighted`): iris underline on the UTF-16 indexes `SearchHit.nameHits` reports.
struct HitText: View {
    var text: String
    var hits: [Int]

    var body: some View { marked }

    private var marked: Text {
        guard !hits.isEmpty else { return Text(text) }
        let set = Set(hits)
        var result = Text("")
        var run = ""
        var runHit = false
        var unit = 0
        func push() {
            guard !run.isEmpty else { return }
            if runHit {
                result = result + Text(run).foregroundColor(Color.kz.irisForeground).underline(true, color: Color.kz.iris.opacity(0.5))
            } else {
                result = result + Text(run)
            }
            run = ""
        }
        for character in text {
            let hit = set.contains(unit)
            if hit != runHit { push(); runHit = hit }
            run.append(character)
            unit += character.utf16.count
        }
        push()
        return result
    }
}
