// Renders Kudzu's derived design exports with AppKit (no Tauri CLI, no third-party rasterizer).
// Run through scripts/design/update-design-assets.sh, from apps/macos:
//   swift scripts/design/design-assets.swift
//
// Writes:
//   design/wordmark/kudzu-wordmark-echo-small.png   741×741, the small wordmark centred (input below)
//   design/icon.png                                 1024×1024, the exported icon with macOS margins
//   design/dmg/background.png                       660×440 pt at 2× (1320×880 px, 144 dpi)
//   Kudzu/Assets.xcassets/MenuBar*.imageset/*.png   18 pt menu-bar glyphs at 1×, 2× and 3×
//   ../website/public/assets/kudzu-icon.png         256×256 copy of design/icon.png for the site
import AppKit

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
struct Brand: Decodable { let tagline: String; let description: String }
let brand = try JSONDecoder().decode(Brand.self, from: Data(contentsOf: root.appendingPathComponent("design/brand.json")))
func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 255) / 255, green: CGFloat((hex >> 8) & 255) / 255, blue: CGFloat(hex & 255) / 255, alpha: alpha)
}
func render(_ path: String, width: Int, height: Int, scale: Int = 1, draw: () -> Void) throws {
    guard let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width * scale, pixelsHigh: height * scale, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0), let context = NSGraphicsContext(bitmapImageRep: bitmap) else { fatalError("Cannot create bitmap") }
    // The point size sets the PNG's dpi: 72 × scale. Finder shows the 2× DMG background at 660×440.
    bitmap.size = NSSize(width: width, height: height)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
    draw()
    NSGraphicsContext.restoreGraphicsState()
    guard let data = bitmap.representation(using: .png, properties: [:]) else { fatalError("Cannot encode PNG") }
    try data.write(to: root.appendingPathComponent(path))
}
/// An SVG as an image, through AppKit's SVG renderer with two rewrites for what it gets wrong:
/// - it ignores `fill-opacity`; Kudzu's SVGs only use it on stroke-less paths, where it means
///   the same as `opacity`;
/// - it blurs the edge of a mask; the menu-bar badges (brand-assets.py) mask with a white rect
///   minus a black circle, which is the same as an even-odd clip path.
func svg(_ path: String) -> NSImage {
    guard var text = try? String(contentsOf: root.appendingPathComponent(path), encoding: .utf8) else { fatalError("Cannot read \(path)") }
    text = text.replacingOccurrences(of: "fill-opacity=", with: "opacity=")
    let mask = ##"<mask id="(\w+)"><rect width="([\d.]+)" height="([\d.]+)" fill="#fff"/><circle cx="([\d.]+)" cy="([\d.]+)" r="([\d.]+)" fill="#000"/></mask>"##
    if let match = text.range(of: mask, options: .regularExpression) {
        let parts = try! NSRegularExpression(pattern: mask).firstMatch(in: text, range: NSRange(match, in: text))!
        let v = (1 ... 6).map { String(text[Range(parts.range(at: $0), in: text)!]) }
        let (id, w, h, cx, cy, r) = (v[0], v[1], v[2], Double(v[3])!, Double(v[4])!, v[5])
        let top = cy - Double(r)!, bottom = cy + Double(r)!
        let clip = #"<clipPath id="\#(id)"><path clip-rule="evenodd" d="M0 0H\#(w)V\#(h)H0Z M\#(cx) \#(top)A\#(r) \#(r) 0 1 0 \#(cx) \#(bottom)A\#(r) \#(r) 0 1 0 \#(cx) \#(top)Z"/></clipPath>"#
        text.replaceSubrange(match, with: clip)
        text = text.replacingOccurrences(of: ##"mask="url(#\##(id))""##, with: ##"clip-path="url(#\##(id))""##)
    }
    guard let image = NSImage(data: Data(text.utf8)) else { fatalError("Cannot render \(path)") }
    return image
}
/// Draws `image` scaled to fit a `side`-point square, centred (`tauri icon --fit contain`).
func drawContained(_ image: NSImage, side: CGFloat) {
    let fit = min(side / image.size.width, side / image.size.height)
    let size = NSSize(width: image.size.width * fit, height: image.size.height * fit)
    image.draw(in: NSRect(x: (side - size.width) / 2, y: (side - size.height) / 2, width: size.width, height: size.height))
}
func text(_ value: String, y: CGFloat, size: CGFloat, weight: NSFont.Weight = .regular, ink: NSColor, tracking: CGFloat = 0) {
    let style = NSMutableParagraphStyle()
    style.alignment = .center
    let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: ink, .paragraphStyle: style, .kern: tracking]
    (value as NSString).draw(in: NSRect(x: 30, y: 440 - y - size * 1.6, width: 600, height: size * 1.6), withAttributes: attributes)
}

// Wordmark raster for the DMG background.
let smallWordmark = svg("design/wordmark/kudzu-wordmark-echo-small.svg")
try render("design/wordmark/kudzu-wordmark-echo-small.png", width: 741, height: 741) { drawContained(smallWordmark, side: 741) }

guard let icon = NSImage(contentsOf: root.appendingPathComponent("design/app-icon/Kudzu-1024.png")) else { fatalError("Export the icon from Icon Composer first") }
try render("design/icon.png", width: 1024, height: 1024) {
    icon.draw(in: NSRect(x: 96, y: 96, width: 832, height: 832))
}
try render("design/dmg/background.png", width: 660, height: 440, scale: 2) {
    color(0xF8F9F7).setFill()
    NSRect(x: 0, y: 0, width: 660, height: 440).fill()
    NSGradient(starting: color(0x2B7FFF, 0.075), ending: color(0x2B7FFF, 0))!.draw(fromCenter: NSPoint(x: 330, y: 220), radius: 0, toCenter: NSPoint(x: 330, y: 220), radius: 290, options: [])
    guard let wordmark = NSImage(contentsOf: root.appendingPathComponent("design/wordmark/kudzu-wordmark-echo-small.png")) else { fatalError("Render the wordmark first") }
    let artworkHeight = wordmark.size.width * 207 / 741
    wordmark.draw(in: NSRect(x: 250, y: 363, width: 160, height: 160 * 207 / 741), from: NSRect(x: 0, y: (wordmark.size.height - artworkHeight) / 2, width: wordmark.size.width, height: artworkHeight), operation: .sourceOver, fraction: 1)
    text(brand.tagline, y: 91, size: 24, weight: .semibold, ink: color(0x202521))
    text("Drag Kudzu to Applications to get started.", y: 132, size: 12, ink: color(0x606861))
    let guide = NSBezierPath()
    guide.move(to: NSPoint(x: 255, y: 222))
    guide.curve(to: NSPoint(x: 402, y: 222), controlPoint1: NSPoint(x: 305, y: 246), controlPoint2: NSPoint(x: 353, y: 246))
    guide.lineWidth = 1.6
    color(0x2B7FFF, 0.5).setStroke()
    guide.stroke()
    let arrow = NSBezierPath()
    arrow.move(to: NSPoint(x: 393, y: 233))
    arrow.line(to: NSPoint(x: 403, y: 222))
    arrow.line(to: NSPoint(x: 389, y: 219))
    arrow.lineWidth = 1.6
    arrow.lineJoinStyle = .round
    arrow.stroke()
    text(brand.description, y: 324, size: 12, ink: color(0x737C75))
    text("After copying, open Kudzu from Applications.", y: 350, size: 10, ink: color(0x858D87))
}

// Menu-bar glyphs (template images) straight into the asset catalog.
for (suffix, imageset) in [("", "MenuBar"), ("-paused", "MenuBarPaused"), ("-permissions", "MenuBarPermissions"), ("-secure-input", "MenuBarSecureInput")] {
    let glyph = svg("design/menu-bar/kudzu-menubar\(suffix).svg")
    for (scale, name) in [(1, ""), (2, "@2x"), (3, "@3x")] {
        let side = 18 * scale
        try render("Kudzu/Assets.xcassets/\(imageset).imageset/kudzu-menubar\(suffix)\(name).png", width: side, height: side) { drawContained(glyph, side: CGFloat(side)) }
    }
}

// The website's copy of the app icon.
guard let padded = NSImage(contentsOf: root.appendingPathComponent("design/icon.png")) else { fatalError("Cannot read design/icon.png") }
try render("../website/public/assets/kudzu-icon.png", width: 256, height: 256) {
    padded.draw(in: NSRect(x: 0, y: 0, width: 256, height: 256))
}
