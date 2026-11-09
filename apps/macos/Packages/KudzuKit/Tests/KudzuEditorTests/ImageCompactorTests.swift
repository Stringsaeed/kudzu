import CoreGraphics
import Foundation
import ImageIO
import KudzuCore
import Testing
import UniformTypeIdentifiers
@testable import KudzuEditor

@Suite struct ImageCompactorTests {
    /// Encodes a `width`×`height` image; `noise` makes it incompressible.
    static func image(width: Int, height: Int, noise: Bool = false, type: UTType = .png) -> Data {
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        if noise, let pixels = context.data?.assumingMemoryBound(to: UInt32.self) {
            var seed: UInt64 = 0x9E37_79B9_7F4A_7C15
            for i in 0..<(context.bytesPerRow / 4 * height) {
                seed = seed &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
                pixels[i] = UInt32(truncatingIfNeeded: seed >> 32) | 0xFF00_0000
            }
        } else {
            context.setFillColor(CGColor(red: 0.2, green: 0.4, blue: 0.9, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        }
        let data = NSMutableData()
        let destination = CGImageDestinationCreateWithData(data, type.identifier as CFString, 1, nil)!
        CGImageDestinationAddImage(destination, context.makeImage()!, nil)
        CGImageDestinationFinalize(destination)
        return data as Data
    }

    @Test func largeImagesAreScaledTo512OnTheLongSide() throws {
        let result = try ImageCompactor.compact(data: Self.image(width: 1000, height: 500))
        #expect(result.pixelWidth == 512)
        #expect(result.pixelHeight == 256)
        #expect(result.width == 240)
        #expect(result.dataUrl.hasPrefix("data:image/png;base64,iVBORw0KGgo"))
        #expect(Formatting.safeImage(result.dataUrl))
    }

    @Test func smallImagesKeepTheirSize() throws {
        let result = try ImageCompactor.compact(data: Self.image(width: 10, height: 30, type: .jpeg))
        #expect(result.pixelWidth == 10)
        #expect(result.pixelHeight == 30)
        // Display width: at least 16, at most 240.
        #expect(result.width == 16)
    }

    @Test func detailedImagesShrinkUntilTheyFit() throws {
        let result = try ImageCompactor.compact(data: Self.image(width: 512, height: 512, noise: true))
        #expect(result.dataUrl.utf8.count <= ImageCompactor.maxDataUrlLength)
        #expect(result.pixelWidth < 512)
        #expect(Formatting.safeImage(result.dataUrl))
    }

    @Test func otherFormatsAreRejected() {
        #expect(throws: ImageCompactor.Failure.unsupported) { try ImageCompactor.compact(data: Self.image(width: 4, height: 4, type: .gif)) }
        #expect(throws: ImageCompactor.Failure.unsupported) { try ImageCompactor.compact(data: Data("not an image".utf8)) }
        #expect(ImageCompactor.Failure.unsupported.localizedDescription == "Choose a PNG, JPEG, or WebP image.")
    }
}
