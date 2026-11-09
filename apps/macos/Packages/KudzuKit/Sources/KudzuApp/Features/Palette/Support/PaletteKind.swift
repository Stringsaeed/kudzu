import Foundation
import KudzuCore

/// The glyph beside a palette row (`KindIcon` in `palette.tsx`).
enum PaletteKind: Equatable, Sendable {
    case form, clipboard, date, plain

    /// Form wins over clipboard, clipboard over a date or time tag, otherwise a document.
    static func classify(_ content: String) -> PaletteKind {
        let facts = Template.analyze(content)
        if facts.isForm { return .form }
        if facts.usesClipboard { return .clipboard }
        if content.range(of: #"\{\{(date|time)"#, options: .regularExpression) != nil { return .date }
        return .plain
    }
}
