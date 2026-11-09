// `FormattedText` (`formatted-text.tsx`): a render result's text with its spans — bold (700),
// italic, underline, links (iris, underlined), inline images at their stored width — and the iris
// caret where `{{cursor}}` lands. One native `Text` built by interpolation, so it wraps and selects
// like any text.

import AppKit
import KudzuCore
import SwiftUI

/// Kudzu’s FormattedText design vocabulary.
public struct FormattedText: View {
    var text: String
    var spans: [FormatSpan]
    var cursorOffset: Int?
    @State private var availableWidth: CGFloat = .greatestFiniteMagnitude
    @Environment(\.colorScheme) private var scheme

    /// Creates FormattedText with the supplied content and styling.
    public init(_ result: RenderResult) {
        self.text = result.text
        self.spans = result.formatting ?? []
        self.cursorOffset = result.cursorOffset
    }

    /// Creates FormattedText with the supplied content and styling.
    public init(text: String, spans: [FormatSpan] = [], cursorOffset: Int? = nil) {
        self.text = text
        self.spans = spans
        self.cursorOffset = cursorOffset
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        let parts = FormattingHelpers.formattedParts(text, spans: spans, cursor: cursorOffset)
        var out = Text(verbatim: "")
        for part in parts {
            if cursorOffset == part.start { out = Text("\(out)\(Self.caret(t))") }
            if let image = part.image, case .image(let url, let width) = image.style {
                if part.start == image.start, let ns = Self.image(dataURL: url, width: CGFloat(width), maximumWidth: availableWidth) {
                    out = Text("\(out)\(Text(Image(nsImage: ns)).baselineOffset(-2))")
                }
                continue
            }
            out = Text("\(out)\(Self.styled(part, t: t))")
        }
        if cursorOffset == text.utf16.count { out = Text("\(out)\(Self.caret(t))") }
        return out
            .frame(maxWidth: spans.contains { if case .image = $0.style { true } else { false } } ? .infinity : nil, alignment: .leading)
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { availableWidth = $0 }
    }

    static func styled(_ part: FormattingHelpers.Part, t: KZTheme) -> Text {
        var s = Text(verbatim: part.text)
        let link = part.styles.contains { if case .link = $0 { true } else { false } }
        if part.styles.contains(.bold) { s = s.fontWeight(.bold) }
        if part.styles.contains(.italic) { s = s.italic() }
        if part.styles.contains(.underline) || link { s = s.underline() }
        if link { s = s.foregroundColor(t[.iris]) }
        return s
    }

    /// `mx-px inline-block h-[1.1em] w-[2px] translate-y-[3px] rounded-full bg-iris`.
    static func caret(_ t: KZTheme) -> Text {
        let h = (13 * 1.1).rounded()
        let image = NSImage(size: NSSize(width: 4, height: h), flipped: false) { _ in
            NSColor.oklch(t.value(.iris)).setFill()
            NSBezierPath(roundedRect: NSRect(x: 1, y: 0, width: 2, height: h), xRadius: 1, yRadius: 1).fill()
            return true
        }
        return Text(Image(nsImage: image)).baselineOffset(-3)
    }

    static func image(dataURL: String, width: CGFloat, maximumWidth: CGFloat = .greatestFiniteMagnitude) -> NSImage? {
        guard let comma = dataURL.firstIndex(of: ","), let data = Data(base64Encoded: String(dataURL[dataURL.index(after: comma)...])),
              let image = NSImage(data: data), image.size.width > 0 else { return nil }
        let fitted = min(width, max(0, maximumWidth))
        let scale = fitted / image.size.width
        image.size = NSSize(width: fitted, height: image.size.height * scale)
        return image
    }
}
