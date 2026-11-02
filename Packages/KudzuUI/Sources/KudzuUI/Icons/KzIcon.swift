import CoreGraphics
import SwiftUI

/// A parsed glyph: its stroked and filled geometry on the 16-unit grid.
public struct KzGlyph: Sendable {
    /// Everything that is stroked (paths and circles), combined, on the 16-unit grid.
    public let stroke: Path
    /// Everything that is filled, on the 16-unit grid.
    public let fill: Path

    /// The glyph for an icon name, parsed once and cached.
    public static func named(_ name: KzIconName) -> KzGlyph {
        cache[name]!
    }

    private static let cache: [KzIconName: KzGlyph] = {
        var out: [KzIconName: KzGlyph] = [:]
        for name in KzIconName.allCases {
            out[name] = KzGlyph(parts: name.parts)
        }
        return out
    }()

    /// Builds a glyph from parts. Invalid path data is a programming error caught by the tests.
    public init(parts: [KzGlyphPart]) {
        let stroke = CGMutablePath()
        let fill = CGMutablePath()
        for part in parts {
            switch part {
            case .stroke(let d):
                if let path = try? KzSVGPath.parse(d) { stroke.addPath(path) }
            case .fill(let d):
                if let path = try? KzSVGPath.parse(d) { fill.addPath(path) }
            case .circle(let x, let y, let r):
                stroke.addEllipse(in: CGRect(x: x - r, y: y - r, width: 2 * r, height: 2 * r))
            }
        }
        self.stroke = Path(stroke)
        self.fill = Path(fill)
    }
}

/// Kudzu's line icon. Takes its colour from the foreground style, like text.
///
/// Sizes: 16 in the sidebar, toolbar and settings; 14 in small buttons and menus; 12 in chips and
/// rows; 28 only for the empty-state mark. The 1.5 stroke scales with the size, as in the SVGs.
public struct KzIcon: View {
    /// The glyph to draw.
    public let name: KzIconName
    /// Edge length in points.
    public let size: CGFloat
    /// The VoiceOver label; `nil` hides a decorative icon from accessibility.
    public let label: String?

    /// Creates an icon. Pass a `label` (often `name.accessibilityLabel`) when the icon stands alone.
    public init(_ name: KzIconName, size: CGFloat = 16, label: String? = nil) {
        self.name = name
        self.size = size
        self.label = label
    }

    public var body: some View {
        let glyph = KzGlyph.named(name)
        let scale = size / 16
        ZStack {
            KzGlyphShape(path: glyph.stroke)
                .stroke(style: StrokeStyle(lineWidth: 1.5 * scale, lineCap: .round, lineJoin: .round))
            KzGlyphShape(path: glyph.fill)
                .fill()
        }
        .frame(width: size, height: size)
        .accessibilityElement()
        .modifier(KzIconAccessibility(label: label))
    }
}

private struct KzIconAccessibility: ViewModifier {
    let label: String?

    func body(content: Content) -> some View {
        if let label {
            content
                .accessibilityLabel(Text(label))
                .accessibilityAddTraits(.isImage)
        } else {
            content.accessibilityHidden(true)
        }
    }
}

/// A 16-unit glyph path scaled to fill its frame.
public struct KzGlyphShape: Shape {
    /// The path on the 16-unit grid.
    public let path: Path

    /// Creates a shape from a 16-grid path.
    public init(path: Path) {
        self.path = path
    }

    public func path(in rect: CGRect) -> Path {
        let transform = CGAffineTransform(translationX: rect.minX, y: rect.minY)
            .scaledBy(x: rect.width / 16, y: rect.height / 16)
        return path.applying(transform)
    }
}
