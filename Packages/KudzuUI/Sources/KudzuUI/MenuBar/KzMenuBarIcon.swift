import AppKit
import SwiftUI

/// The menu bar glyph for each engine state, as 18pt template images (macOS tints them).
///
/// `active` and `paused` come from the design system's Logos. `permissionsMissing` and
/// `secureInput` were drawn for the app in the same style; their SVGs live in
/// `design/kudzu/additions/` so they can go back into the design system.
public enum KzMenuBarIcon: String, CaseIterable, Sendable {
    /// Expanding: a sprouting colon, matching the app icon's printed legend.
    case active
    /// Expansion paused by the user: pause bars replace the colon's lower dot.
    case paused
    /// Accessibility or Input Monitoring is missing: a warning replaces the lower dot.
    case permissionsMissing
    /// Secure Input is on, so macOS blocks expansion: a lock replaces the lower dot.
    case secureInput

    /// The bundled SVG this glyph is drawn from.
    public var resourceName: String {
        switch self {
        case .active: "kudzu-menubar"
        case .paused: "kudzu-menubar-paused"
        case .permissionsMissing: "kudzu-menubar-permissions"
        case .secureInput: "kudzu-menubar-secure-input"
        }
    }

    /// The VoiceOver description of the status item.
    public var accessibilityDescription: String {
        switch self {
        case .active: "Kudzu"
        case .paused: "Kudzu, paused"
        case .permissionsMissing: "Kudzu, needs permissions"
        case .secureInput: "Kudzu, blocked by Secure Input"
        }
    }

    /// A fresh 18×18pt template image of the glyph.
    public func image() -> NSImage {
        let drawing = KzSVGDocument.load(resourceName)
        let size = drawing?.size ?? CGSize(width: 18, height: 18)
        let image = NSImage(size: size, flipped: true) { _ in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            drawing?.draw(in: context)
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = accessibilityDescription
        return image
    }
}

/// A minimal SVG renderer for the menu bar glyphs: `<g>` and `<path>` with fill, stroke and fill-rule.
struct KzSVGDocument: Sendable {
    struct Element: Sendable {
        var path: Path
        var fill: Bool
        var evenOdd: Bool
        var strokeWidth: CGFloat?
        var lineCap: CGLineCap
        var lineJoin: CGLineJoin
    }

    var size: CGSize
    var elements: [Element]

    static func load(_ resourceName: String) -> KzSVGDocument? {
        guard let url = Bundle.module.url(forResource: resourceName, withExtension: "svg", subdirectory: "MenuBar"),
              let data = try? Data(contentsOf: url) else { return nil }
        return parse(data)
    }

    static func parse(_ data: Data) -> KzSVGDocument? {
        let delegate = Delegate()
        let parser = XMLParser(data: data)
        parser.delegate = delegate
        guard parser.parse() else { return nil }
        return KzSVGDocument(size: delegate.size, elements: delegate.elements)
    }

    /// Draws every element in black; template rendering recolours it.
    func draw(in context: CGContext) {
        context.setFillColor(NSColor.black.cgColor)
        context.setStrokeColor(NSColor.black.cgColor)
        for element in elements {
            if element.fill {
                context.addPath(element.path.cgPath)
                context.fillPath(using: element.evenOdd ? .evenOdd : .winding)
            }
            if let width = element.strokeWidth {
                context.setLineWidth(width)
                context.setLineCap(element.lineCap)
                context.setLineJoin(element.lineJoin)
                context.addPath(element.path.cgPath)
                context.strokePath()
            }
        }
    }

    private final class Delegate: NSObject, XMLParserDelegate {
        var size = CGSize(width: 18, height: 18)
        var elements: [Element] = []
        private var stack: [[String: String]] = [[:]]

        func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes: [String: String]) {
            var inherited = stack.last ?? [:]
            for key in ["fill", "stroke", "stroke-width", "fill-rule", "stroke-linecap", "stroke-linejoin"] {
                if let value = attributes[key] { inherited[key] = value }
            }
            stack.append(inherited)
            switch name {
            case "svg":
                let box = (attributes["viewBox"] ?? "").split(whereSeparator: { $0 == " " || $0 == "," }).compactMap { Double($0) }
                if box.count == 4 { size = CGSize(width: box[2], height: box[3]) }
            case "path":
                guard let d = attributes["d"], let path = try? KzSVGPath.parse(d) else { return }
                let fill = (inherited["fill"] ?? "#000") != "none"
                let strokes = (inherited["stroke"] ?? "none") != "none"
                elements.append(Element(
                    path: Path(path),
                    fill: fill,
                    evenOdd: inherited["fill-rule"] == "evenodd",
                    strokeWidth: strokes ? CGFloat(Double(inherited["stroke-width"] ?? "1") ?? 1) : nil,
                    lineCap: inherited["stroke-linecap"] == "round" ? .round : inherited["stroke-linecap"] == "square" ? .square : .butt,
                    lineJoin: inherited["stroke-linejoin"] == "round" ? .round : inherited["stroke-linejoin"] == "bevel" ? .bevel : .miter
                ))
            default:
                break
            }
        }

        func parser(_ parser: XMLParser, didEndElement name: String, namespaceURI: String?, qualifiedName: String?) {
            stack.removeLast()
        }
    }
}
