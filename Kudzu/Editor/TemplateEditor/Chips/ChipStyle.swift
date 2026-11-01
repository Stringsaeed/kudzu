import AppKit
import KudzuCore

/// Colors and metrics shared by chip drawing and the insert menu.
enum ChipStyle {
    /// Tint for a chip family; warnings override the family color.
    static func tint(for kind: ChipLabel.Kind, warning: Bool = false) -> NSColor {
        if warning { return .systemOrange }
        switch kind {
        case .date, .time: return .systemBlue
        case .clipboard: return .systemTeal
        case .cursor: return .systemPink
        case .snippet: return .systemPurple
        case .field: return .systemGreen
        case .choice: return .systemIndigo
        case .unknown: return .systemRed
        }
    }

    /// Tint for an insert-menu item, matching the chip it creates.
    static func tint(forItem item: TemplateInsertItem) -> NSColor {
        switch item {
        case .date: tint(for: ChipLabel.Kind.date)
        case .time: tint(for: ChipLabel.Kind.time)
        case .clipboard: tint(for: ChipLabel.Kind.clipboard)
        case .cursor: tint(for: ChipLabel.Kind.cursor)
        case .field: tint(for: ChipLabel.Kind.field)
        case .choice: tint(for: ChipLabel.Kind.choice)
        case .snippet: tint(for: ChipLabel.Kind.snippet)
        }
    }

    /// Label font derived from the editor body font, a touch smaller so chips sit inside the line box.
    static func labelFont(for bodyFont: NSFont) -> NSFont {
        .systemFont(ofSize: (bodyFont.pointSize - 1.5).rounded(), weight: .medium)
    }

    static let outerMargin: CGFloat = 1.5
    static let horizontalPadding: CGFloat = 6
    static let iconGap: CGFloat = 3.5
}
