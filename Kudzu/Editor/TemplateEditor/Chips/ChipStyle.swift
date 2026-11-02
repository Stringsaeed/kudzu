import AppKit
import KudzuCore
import KudzuUI

/// How serious a problem on a chip or in the text is. It decides the ink and fill of the chip and the squiggle colour.
enum ChipSeverity: Int, Comparable {
    case warning, error

    static func < (lhs: ChipSeverity, rhs: ChipSeverity) -> Bool { lhs.rawValue < rhs.rawValue }

    init(_ severity: TemplateDiagnostic.Severity) {
        self = severity == .error ? .error : .warning
    }

    /// Squiggle colour: `danger` for errors, `warning` for warnings.
    var squiggle: NSColor {
        switch self {
        case .warning: KzColorToken.warning.nsColor
        case .error: KzColorToken.danger.nsColor
        }
    }
}

/// The colours of one chip in one state, all from `KzColor` tokens (dynamic, so Light and Dark follow the view).
struct ChipPalette {
    var fill: NSColor
    var ink: NSColor
    /// Fill and ink when the chip is selected in the focused editor: the ink colour becomes the fill.
    var selectedFill: NSColor
    var selectedInk: NSColor

    /// The design system's tag: `vine-ink` on `vine-soft`; `danger`/`warning` on their soft fills for problems.
    static func palette(for severity: ChipSeverity?) -> ChipPalette {
        switch severity {
        case nil:
            ChipPalette(fill: KzEditorTag.fill, ink: KzEditorTag.ink,
                        selectedFill: KzEditorTag.selectedFill, selectedInk: KzEditorTag.selectedInk)
        case .warning:
            ChipPalette(fill: KzColorToken.warningSoft.nsColor, ink: KzColorToken.warning.nsColor,
                        selectedFill: KzColorToken.warning.nsColor, selectedInk: KzColorToken.surfaceRaised.nsColor)
        case .error:
            ChipPalette(fill: KzColorToken.dangerSoft.nsColor, ink: KzColorToken.danger.nsColor,
                        selectedFill: KzColorToken.danger.nsColor, selectedInk: KzColorToken.surfaceRaised.nsColor)
        }
    }
}

/// Icons for chip kinds and insert items, drawn from `KzIcon` (the editor chips themselves are text-only, as in the design).
enum ChipStyle {
    /// The icon for a chip family.
    static func icon(for kind: ChipLabel.Kind) -> KzIconName {
        switch kind {
        case .date: .clock
        case .time: .timer
        case .clipboard: .copy
        case .cursor: .cursor
        case .snippet: .textPlus
        case .field: .type
        case .choice: .list
        case .unknown: .warning
        }
    }

    /// The icon for an insert-menu item, matching the chip it creates.
    static func icon(for item: TemplateInsertItem) -> KzIconName {
        switch item {
        case .date: icon(for: ChipLabel.Kind.date)
        case .time: icon(for: ChipLabel.Kind.time)
        case .clipboard: icon(for: ChipLabel.Kind.clipboard)
        case .cursor: icon(for: ChipLabel.Kind.cursor)
        case .field: icon(for: ChipLabel.Kind.field)
        case .choice: icon(for: ChipLabel.Kind.choice)
        case .snippet: icon(for: ChipLabel.Kind.snippet)
        }
    }
}
