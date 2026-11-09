import Foundation

// Types of `kudzu-core/src/formatting.rs`. Formatting annotates the unchanged plain-text template
// using UTF-16 ranges. The functions (`safeLink`, `insertionText`, …) live in `Formatting/`.

/// One style applied to a span. JSON is tagged by `kind`.
public enum FormatStyle: Codable, Hashable, Sendable {
    case bold
    case italic
    case underline
    case link(url: String)
    case image(dataUrl: String, width: UInt32)

    private enum CodingKeys: String, CodingKey { case kind, url, dataUrl, width }

    /// The `kind` discriminator.
    public var kind: String {
        switch self {
        case .bold: "bold"
        case .italic: "italic"
        case .underline: "underline"
        case .link: "link"
        case .image: "image"
        }
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "bold": self = .bold
        case "italic": self = .italic
        case "underline": self = .underline
        case "link": self = .link(url: try c.decode(String.self, forKey: .url))
        case "image": self = .image(dataUrl: try c.decode(String.self, forKey: .dataUrl), width: try c.decode(UInt32.self, forKey: .width))
        case let other:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: c, debugDescription: "unknown format style \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(kind, forKey: .kind)
        switch self {
        case .bold, .italic, .underline: break
        case let .link(url): try c.encode(url, forKey: .url)
        case let .image(dataUrl, width):
            try c.encode(dataUrl, forKey: .dataUrl)
            try c.encode(width, forKey: .width)
        }
    }
}

/// A style over the UTF-16 range `start..<end` of the source.
public struct FormatSpan: Codable, Hashable, Sendable {
    public var start: Int
    public var end: Int
    public var style: FormatStyle

    public init(start: Int, end: Int, style: FormatStyle) {
        self.start = start
        self.end = end
        self.style = style
    }
}

/// Rich formatting for a snippet's content.
public struct Formatting: Codable, Hashable, Sendable {
    /// Detects stale ranges when an older client edits only `content`.
    public var source: String
    public var spans: [FormatSpan]

    public init(source: String, spans: [FormatSpan]) {
        self.source = source
        self.spans = spans
    }
}
