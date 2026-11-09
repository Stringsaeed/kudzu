import KudzuCore
import KudzuDesign
import SwiftUI

/// One ranked hit, or the "Create snippet" row (`ResultRow` in `palette.tsx`).
struct PaletteHitRow: View {
    var hit: SearchHit
    var index: Int
    var active: Bool
    var showNumbers: Bool
    var path: String
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let theme = KZTheme(scheme)
        let snippet = hit.snippet
        let title = snippet.name.isEmpty ? "Untitled" : snippet.name
        HStack(spacing: 12) {
            kindIcon(snippet.content, theme: theme)
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 6) {
                    HighlightedTitle(text: title, hits: snippet.name.isEmpty ? [] : hit.nameHits, theme: theme)
                    if snippet.isFavorite {
                        Icon.starFill.icon(12).foregroundStyle(theme[.favorite])
                    }
                }
                if !path.isEmpty {
                    Text(path).font(.system(size: 11)).lineLimit(1).foregroundStyle(theme[.faint])
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let trigger = snippet.triggers.first {
                TriggerChip(trigger, muted: !active)
            }
            if index < 9 {
                Shortcut("mod+\(index + 1)", tone: showNumbers ? .accent : .default)
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 44)
        .opacity(snippet.isEnabled ? 1 : 0.6)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(active ? .isSelected : [])
    }

    private func kindIcon(_ content: String, theme: KZTheme) -> some View {
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        return kindImage(PaletteKind.classify(content))
            .foregroundStyle(active ? theme[.irisContrast] : theme[.mutedForeground])
            .frame(width: 28, height: 28)
            .depth(active ? .accent : .none, fill: active ? theme[.iris] : theme[.foreground, alpha: 0.06], in: shape)
            .overlay { if active { shape.fill(Sheen.strong.gradient).allowsHitTesting(false) } }
    }

    private func kindImage(_ kind: PaletteKind) -> some View {
        switch kind {
        case .form: Icon.textCursorInput.icon(14)
        case .clipboard: Icon.clipboard.icon(14)
        case .date: Icon.calendarDays.icon(14)
        case .plain: Icon.fileText.icon(14)
        }
    }
}

/// The trailing "Create snippet “…”" row.
struct PaletteCreateRow: View {
    var query: String
    var active: Bool
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let theme = KZTheme(scheme)
        HStack(spacing: 12) {
            Icon.plus.icon(14)
                .foregroundStyle(active ? theme[.foreground] : theme[.mutedForeground])
                .frame(width: 28, height: 28)
                .background(theme[.foreground, alpha: 0.06], in: RoundedRectangle(cornerRadius: Radius.lg))
            (
                Text("Create snippet ").foregroundColor(active ? theme[.foreground] : theme[.mutedForeground])
                    + Text("“\(query)”").foregroundColor(theme[.foreground])
            )
            .font(.system(size: 13.5))
            .lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 10)
        .frame(height: 44)
        .accessibilityLabel("Create snippet \(query)")
        .accessibilityAddTraits(active ? .isSelected : [])
    }
}

/// Title with the matched characters drawn in iris (`Highlight`).
struct HighlightedTitle: View {
    var text: String
    var hits: [Int]
    var theme: KZTheme

    var body: some View {
        let runs = PaletteHighlight.runs(text, hits: hits)
        runs.reduce(Text(verbatim: "")) { partial, run in
            let piece = Text(verbatim: run.text).foregroundColor(run.hit ? theme[.irisForeground] : theme[.foreground])
            return Text("\(partial)\(piece)")
        }
        .font(.system(size: 13.5, weight: .medium))
        .lineLimit(1)
    }
}
