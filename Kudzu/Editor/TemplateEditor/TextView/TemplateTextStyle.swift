import AppKit
import KudzuUI

/// Monospace typography shared by loaded, typed, pasted and placeholder editor text.
enum TemplateTextStyle {
    static var font: NSFont { .monospacedSystemFont(ofSize: 14, weight: .regular) }
    static let lineHeight: CGFloat = 22

    static func attributes(color: NSColor = KzColorToken.ink.nsColor) -> [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        paragraph.minimumLineHeight = lineHeight
        paragraph.maximumLineHeight = lineHeight
        paragraph.tabStops = []
        paragraph.defaultTabInterval = (" " as NSString).size(withAttributes: [.font: font]).width * 4
        return [.font: font, .foregroundColor: color, .paragraphStyle: paragraph]
    }
}
