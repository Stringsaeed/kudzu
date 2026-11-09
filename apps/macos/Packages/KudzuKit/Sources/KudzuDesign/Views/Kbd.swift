// Keycaps (`kbd.tsx`) and `Shortcut` (`components/app/shortcut.tsx`). No native control draws an
// inline keycap, so these are custom: `h-5 min-w-5 rounded-[5px] px-1 bg-foreground/[0.06]`, 11 pt
// medium muted tabular text, key depth. Inside tooltips and filled buttons keycaps go translucent so
// they read on any fill.

import KudzuCore
import SwiftUI

/// Where a keycap sits, which restyles it like the web's `in-data-[slot=…]` rules.
public enum KbdContext: Sendable {
    case standard
    /// Inside a tooltip (inverted surface): `bg-background/20 text-background`, no shadow.
    case tooltip
    /// Inside a default (filled) button: `bg-black/15 text-current`, inset shadow.
    case onFilledButton
}

extension EnvironmentValues {
    /// The keycap treatment inherited from its enclosing control.
    @Entry public var kzKbdContext: KbdContext = .standard
}

/// `Shortcut` tones.
public enum ShortcutTone: Sendable {
    /// Keycap on a regular surface. Tooltips and filled buttons restyle it automatically.
    case `default`
    /// Hint that should recede, e.g. revealed on hover: transparent, faint, no shadow.
    case ghost
    /// On an accent fill that isn't a Kudzu button (custom iris buttons).
    case onAccent
    /// Highlighted, e.g. the global quick-search shortcut: iris-soft, iris-foreground.
    case accent
}

/// An inline keyboard keycap using Kudzu’s key depth.
public struct Kbd: View {
    var label: String
    var tone: ShortcutTone
    /// `md` = 20 pt / 11 pt text; `sm` = 18 pt / 10.5 pt (Shortcut's default).
    var small: Bool
    @Environment(\.kzKbdContext) private var context
    @Environment(\.colorScheme) private var scheme

    /// Creates Kbd with the supplied content and styling.
    public init(_ label: String, tone: ShortcutTone = .default, small: Bool = false) {
        self.label = label
        self.tone = tone
        self.small = small
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        let h: CGFloat = small ? 18 : 20
        let shape = RoundedRectangle(cornerRadius: Radius.key)
        let style = resolved(t)
        keycapLabel(style.text)
            .font(.system(size: small ? 10.5 : 11, weight: .medium, design: .monospaced))
            .lineLimit(1)
            .iconSize(12)
            .padding(.horizontal, small ? 3 : 4)
            .frame(minWidth: h, minHeight: h, maxHeight: h)
            .modifier(KbdSurface(fill: style.fill, depth: style.depth, shape: shape))
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }

    /// Filled-button keycaps use a quiet inset independent of theme, without a border.
    static let filledDepth = Depth(name: "filled-key",
        light: .init(inner: [.init(radius: 0.5, y: 1, color: OKLCH(0, 0, 0, alpha: 0.15))]),
        dark: .init(inner: [.init(radius: 0.5, y: 1, color: OKLCH(0, 0, 0, alpha: 0.15))]))

    @ViewBuilder private func keycapLabel(_ style: AnyShapeStyle?) -> some View {
        if let style { Text(label).foregroundStyle(style) }
        else { Text(label) }
    }

    private func resolved(_ t: KZTheme) -> (fill: Color, text: AnyShapeStyle?, depth: Depth) {
        switch tone {
        case .ghost: return (.clear, AnyShapeStyle(t[.faint]), .none)
        case .onAccent: return (KZTheme.black(0.15), nil, Self.filledDepth)
        case .accent: return (t[.irisSoft], AnyShapeStyle(t[.irisForeground]), .none)
        case .default:
            switch context {
            case .standard: return (t[.foreground, alpha: 0.06], AnyShapeStyle(t[.mutedForeground]), .key)
            case .tooltip: return (t[.background, alpha: t.dark ? 0.1 : 0.2], AnyShapeStyle(t[.background]), .none)
            case .onFilledButton: return (KZTheme.black(0.15), nil, Self.filledDepth)
            }
        }
    }
}

private struct KbdSurface<S: InsettableShape>: ViewModifier {
    var fill: Color
    var depth: Depth
    var shape: S
    func body(content: Content) -> some View {
        content.depth(depth, fill: fill, in: shape)
    }
}

/// A keyboard shortcut drawn with keycaps. Pass a combo (`"mod+shift+k"`) or pre-made labels (e.g.
/// from a recorded accelerator). Hidden from VoiceOver; give the control `.keyboardShortcut` or an
/// accessibility label instead.
public struct Shortcut: View {
    var labels: [String]
    var tone: ShortcutTone
    var small: Bool

    /// Creates Shortcut with the supplied content and styling.
    public init(_ combo: String, tone: ShortcutTone = .default, small: Bool = true) {
        self.labels = Keys.keyLabels(combo)
        self.tone = tone
        self.small = small
    }

    /// Creates Shortcut with the supplied content and styling.
    public init(keys: [String], tone: ShortcutTone = .default, small: Bool = true) {
        self.labels = keys
        self.tone = tone
        self.small = small
    }

    /// The composed native view.
    public var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(labels.enumerated()), id: \.offset) { _, key in
                Kbd(key, tone: tone, small: small)
            }
        }
        .accessibilityHidden(true)
    }

    /// Words for a combo, e.g. "mod+enter" → "Command Return".
    public static func spoken(_ combo: String) -> String {
        let words = ["mod": "Command", "ctrl": "Control", "shift": "Shift", "alt": "Option", "enter": "Return", "escape": "Escape", "esc": "Escape",
                     "tab": "Tab", "space": "Space", "backspace": "Delete", "up": "Up Arrow", "down": "Down Arrow", "left": "Left Arrow", "right": "Right Arrow"]
        return combo.split(separator: "+").map { words[$0.lowercased()] ?? $0.uppercased() }.joined(separator: " ")
    }
}

/// A sentence with keycaps inside it (`Opens the tray. <Shortcut combo="enter" spoken /> inserts.`).
/// `Text` can't host a view, so the words and the keycaps flow in a wrapping layout and read as one
/// element whose spoken label names each shortcut ("Return inserts").
/// `ShortcutText([.text("Hold"), .shortcut("shift"), .text("to do the opposite.")])`; parts are
/// joined by spaces. Font and colour come from the environment, like a `Text`.
public struct ShortcutText: View {
    /// One run of the sentence.
    public enum Part: Hashable, Sendable {
        /// Plain words.
        case text(String)
        /// A combo drawn as keycaps (`"mod+enter"`).
        case shortcut(String)
    }

    var parts: [Part]

    /// Creates ShortcutText from its runs.
    public init(_ parts: [Part]) {
        self.parts = parts
    }

    /// The spoken sentence: shortcuts as words.
    public var spoken: String {
        parts.map {
            switch $0 {
            case let .text(words): words
            case let .shortcut(combo): Shortcut.spoken(combo)
            }
        }.joined(separator: " ")
    }

    private enum Token: Hashable {
        case word(String)
        case keys(String)
    }

    private var tokens: [Token] {
        parts.flatMap { part -> [Token] in
            switch part {
            case let .text(words): words.split(separator: " ").map { .word(String($0)) }
            case let .shortcut(combo): [.keys(combo)]
            }
        }
    }

    /// The composed native view.
    public var body: some View {
        WordFlow(spacing: 3) {
            ForEach(Array(tokens.enumerated()), id: \.offset) { _, token in
                switch token {
                case let .word(word): Text(word)
                case let .keys(combo): Shortcut(combo)
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(spoken)
    }
}

/// Lays words out left to right, wrapping to a new line when the width runs out; each line's items
/// are centred vertically (keycaps sit on the text's centre line).
struct WordFlow: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat = 0

    private struct Line {
        var indices: [Int] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func lines(for subviews: Subviews, width: CGFloat) -> [Line] {
        var result: [Line] = [Line()]
        for (i, view) in subviews.enumerated() {
            let size = view.sizeThatFits(.unspecified)
            let needed = result[result.count - 1].indices.isEmpty ? size.width : result[result.count - 1].width + spacing + size.width
            if needed > width, !result[result.count - 1].indices.isEmpty {
                result.append(Line())
            }
            var line = result[result.count - 1]
            line.width = line.indices.isEmpty ? size.width : line.width + spacing + size.width
            line.height = max(line.height, size.height)
            line.indices.append(i)
            result[result.count - 1] = line
        }
        return result
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let all = lines(for: subviews, width: width)
        let height = all.reduce(0) { $0 + $1.height } + lineSpacing * CGFloat(max(0, all.count - 1))
        let used = all.map(\.width).max() ?? 0
        return CGSize(width: proposal.width.map { min($0, used) } ?? used, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for line in lines(for: subviews, width: bounds.width) {
            var x = bounds.minX
            for i in line.indices {
                let size = subviews[i].sizeThatFits(.unspecified)
                subviews[i].place(at: CGPoint(x: x, y: y + line.height / 2), anchor: .leading, proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += line.height + lineSpacing
        }
    }
}
