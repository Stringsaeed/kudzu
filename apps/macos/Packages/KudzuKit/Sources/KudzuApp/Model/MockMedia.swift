import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Artwork for the mock's image entries: `mockImage` of `lib/mock.ts` (a soft gradient "photo" or
/// a fake screenshot), drawn with Core Graphics instead of an SVG data URL.
enum MockMedia {
    struct Artwork: Hashable, Sendable {
        var width: Int
        var height: Int
        var hue: Double
        var screenshot: Bool
    }

    /// PNG bytes of `art`, scaled so the longest side is at most `maxSide` points.
    static func png(_ art: Artwork, maxSide: Int = 960) -> Data? {
        let scale = min(1, Double(maxSide) / Double(max(art.width, art.height)))
        let w = max(1, Int((Double(art.width) * scale).rounded())), h = max(1, Int((Double(art.height) * scale).rounded()))
        guard let ctx = CGContext(
            data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }
        // SVG coordinates are top-left based.
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        let W = CGFloat(w), H = CGFloat(h), hue = art.hue
        if art.screenshot {
            fill(ctx, CGRect(x: 0, y: 0, width: W, height: H), hsl(hue, 0.14, 0.12))
            fill(ctx, CGRect(x: W * 0.04, y: H * 0.06, width: W * 0.22, height: H * 0.88), hsl(hue, 0.14, 0.17), radius: 18 * scale)
            for i in 0..<6 {
                let rect = CGRect(x: W * 0.07, y: H * (0.14 + Double(i) * 0.1), width: W * (0.12 + Double(i % 3) * 0.03), height: H * 0.03)
                fill(ctx, rect, hsl(hue, 0.10, i == 1 ? 0.55 : 0.30), radius: 6 * scale)
            }
            fill(ctx, CGRect(x: W * 0.3, y: H * 0.06, width: W * 0.66, height: H * 0.4), hsl(hue, 0.6, 0.55), radius: 22 * scale)
            fill(ctx, CGRect(x: W * 0.3, y: H * 0.52, width: W * 0.32, height: H * 0.42), hsl(hue + 40, 0.3, 0.22), radius: 22 * scale)
            fill(ctx, CGRect(x: W * 0.64, y: H * 0.52, width: W * 0.32, height: H * 0.42), hsl(hue - 30, 0.3, 0.22), radius: 22 * scale)
        } else {
            let colors = [hsl(hue, 0.7, 0.62), hsl(hue + 70, 0.6, 0.35)] as CFArray
            if let gradient = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB), colors: colors, locations: [0, 1]) {
                ctx.drawLinearGradient(gradient, start: .zero, end: CGPoint(x: W, y: H), options: [])
            }
            ctx.setFillColor(hsl(hue + 20, 0.9, 0.85, alpha: 0.8))
            ctx.fillEllipse(in: CGRect(x: W * 0.72 - H * 0.12, y: H * 0.3 - H * 0.12, width: H * 0.24, height: H * 0.24))
            let path = CGMutablePath()
            path.move(to: CGPoint(x: 0, y: H * 0.78))
            path.addQuadCurve(to: CGPoint(x: W * 0.55, y: H * 0.72), control: CGPoint(x: W * 0.3, y: H * 0.55))
            path.addQuadCurve(to: CGPoint(x: W, y: H * 0.62), control: CGPoint(x: W * 0.8, y: H * 0.89))
            path.addLine(to: CGPoint(x: W, y: H))
            path.addLine(to: CGPoint(x: 0, y: H))
            path.closeSubpath()
            ctx.addPath(path)
            ctx.setFillColor(hsl(hue + 120, 0.35, 0.18, alpha: 0.75))
            ctx.fillPath()
        }
        guard let image = ctx.makeImage() else { return nil }
        let data = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, image, nil)
        return CGImageDestinationFinalize(dest) ? data as Data : nil
    }

    private static func fill(_ ctx: CGContext, _ rect: CGRect, _ color: CGColor, radius: Double = 0) {
        ctx.addPath(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil))
        ctx.setFillColor(color)
        ctx.fillPath()
    }

    /// CSS `hsl()` → sRGB.
    private static func hsl(_ h: Double, _ s: Double, _ l: Double, alpha: Double = 1) -> CGColor {
        let hue = (h.truncatingRemainder(dividingBy: 360) + 360).truncatingRemainder(dividingBy: 360) / 360
        func channel(_ n: Double) -> Double {
            let k = (n + hue * 12).truncatingRemainder(dividingBy: 12)
            return l - s * min(l, 1 - l) * max(-1, min(k - 3, 9 - k, 1))
        }
        return CGColor(srgbRed: channel(0), green: channel(8), blue: channel(4), alpha: alpha)
    }
}
