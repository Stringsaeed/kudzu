// Run from desktop: swift scripts/design-assets.swift
import AppKit
let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
struct Brand: Decodable { let tagline: String; let description: String }
let brand = try JSONDecoder().decode(Brand.self, from: Data(contentsOf: root.appendingPathComponent("design/brand.json")))
func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 255) / 255, green: CGFloat((hex >> 8) & 255) / 255, blue: CGFloat(hex & 255) / 255, alpha: alpha)
}
func render(_ path: String, width: Int, height: Int, scale: Int = 1, draw: () -> Void) throws {
    guard let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width * scale, pixelsHigh: height * scale, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0), let context = NSGraphicsContext(bitmapImageRep: bitmap) else { fatalError("Cannot create bitmap") }
    bitmap.size = NSSize(width: width, height: height)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
    draw()
    NSGraphicsContext.restoreGraphicsState()
    guard let data = bitmap.representation(using: .png, properties: [:]) else { fatalError("Cannot encode PNG") }
    try data.write(to: root.appendingPathComponent(path))
}
func text(_ value: String, y: CGFloat, size: CGFloat, weight: NSFont.Weight = .regular, ink: NSColor, tracking: CGFloat = 0) {
    let style = NSMutableParagraphStyle()
    style.alignment = .center
    let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: ink, .paragraphStyle: style, .kern: tracking]
    (value as NSString).draw(in: NSRect(x: 30, y: 440 - y - size * 1.6, width: 600, height: size * 1.6), withAttributes: attributes)
}
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
