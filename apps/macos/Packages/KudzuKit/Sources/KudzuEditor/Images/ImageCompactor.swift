// `compactImage` from image-tools.tsx with ImageIO instead of a canvas: PNG, JPEG or WebP in,
// a compact PNG data URL out — at most 512 px on the long side and 196 608 data-URL characters
// (144 KiB of PNG), shrinking by 0.75 up to eight times. EXIF orientation is applied.

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// An image ready to embed: a PNG data URL and its default display width.
public struct CompactImage: Hashable, Sendable {
    public var dataUrl: String
    public var width: Int
    /// Pixel size of the encoded PNG.
    public var pixelWidth: Int
    public var pixelHeight: Int
}

public enum ImageCompactor {
    public enum Failure: Error, Equatable, LocalizedError {
        case unsupported, tooLarge, unreadable, unopenable, tooDetailed

        public var errorDescription: String? {
            switch self {
            case .unsupported: "Choose a PNG, JPEG, or WebP image."
            case .tooLarge: "Choose an image smaller than 20 MB."
            case .unreadable: "This image couldn't be read."
            case .unopenable: "This image couldn't be opened."
            case .tooDetailed: "This image is too detailed to embed. Try a smaller image."
            }
        }
    }

    public static let maxDimension = 512
    /// `safeImage`'s limit on one data URL.
    public static let maxDataUrlLength = 196_608
    public static let maxFileSize = 20 * 1024 * 1024
    public static let accepted: [UTType] = [.png, .jpeg, .webP]

    public static func compact(contentsOf url: URL) throws(Failure) -> CompactImage {
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        if size > maxFileSize { throw .tooLarge }
        guard let data = try? Data(contentsOf: url) else { throw .unreadable }
        return try compact(data: data)
    }

    public static func compact(data: Data) throws(Failure) -> CompactImage {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let type = CGImageSourceGetType(source) as String?, let uti = UTType(type) else { throw .unsupported }
        guard accepted.contains(where: { uti.conforms(to: $0) }) else { throw .unsupported }
        if data.count > maxFileSize { throw .tooLarge }
        guard let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              var width = properties[kCGImagePropertyPixelWidth] as? Int,
              var height = properties[kCGImagePropertyPixelHeight] as? Int, width > 0, height > 0 else { throw .unopenable }
        if let orientation = properties[kCGImagePropertyOrientation] as? Int, (5...8).contains(orientation) { swap(&width, &height) }

        var scale = min(1, Double(maxDimension) / Double(max(width, height)))
        for _ in 0..<8 {
            let target = max(1, Int((Double(max(width, height)) * scale).rounded()))
            let options: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: target,
            ]
            guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { throw .unopenable }
            guard let png = encodePNG(image) else { throw .unopenable }
            let dataUrl = "data:image/png;base64," + png.base64EncodedString()
            if dataUrl.utf8.count <= maxDataUrlLength {
                return CompactImage(dataUrl: dataUrl, width: max(16, min(image.width, 240)), pixelWidth: image.width, pixelHeight: image.height)
            }
            scale *= 0.75
        }
        throw .tooDetailed
    }

    static func encodePNG(_ image: CGImage) -> Data? {
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination) ? data as Data : nil
    }
}
