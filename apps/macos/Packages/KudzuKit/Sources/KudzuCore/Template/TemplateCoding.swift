import Foundation

extension AttributeValue: Codable {
    private enum CodingKeys: String, CodingKey { case type, value }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "string": self = .string(try c.decode(String.self, forKey: .value))
        case "bool": self = .bool(try c.decode(Bool.self, forKey: .value))
        case "number": self = .number(try c.decode(Double.self, forKey: .value))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "unknown attribute type")
        }
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .string(let value): try c.encode("string", forKey: .type); try c.encode(value, forKey: .value)
        case .bool(let value): try c.encode("bool", forKey: .type); try c.encode(value, forKey: .value)
        case .number(let value): try c.encode("number", forKey: .type); try c.encode(value, forKey: .value)
        }
    }
}
private struct SourceRange: Codable {
    var start: Int
    var end: Int
    init(_ range: Range<Int>) { start = range.lowerBound; end = range.upperBound }
    var range: Range<Int> { start..<max(start,end) }
}
extension Tag: Codable {
    private enum CodingKeys: String, CodingKey { case name, attributes, range, verbatim }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.init(name: try c.decode(String.self, forKey: .name), attributes: try c.decode([Attribute].self, forKey: .attributes), range: try c.decode(SourceRange.self, forKey: .range).range, verbatim: try c.decodeIfPresent(String.self, forKey: .verbatim))
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(name,forKey:.name); try c.encode(attributes,forKey:.attributes); try c.encode(SourceRange(range),forKey:.range); try c.encode(verbatim,forKey:.verbatim)
    }
}
extension TemplateNode: Codable {
    private enum CodingKeys: String, CodingKey { case kind, text, range }
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "text": self = .text(try c.decode(String.self, forKey: .text), range: try c.decode(SourceRange.self, forKey: .range).range)
        case "tag": self = .tag(try Tag(from: decoder))
        default: throw DecodingError.dataCorruptedError(forKey: .kind, in: c, debugDescription: "unknown node kind")
        }
    }
    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .text(let text, let range): try c.encode("text",forKey:.kind); try c.encode(text,forKey:.text); try c.encode(SourceRange(range),forKey:.range)
        case .tag(let tag): try c.encode("tag",forKey:.kind); try tag.encode(to:encoder)
        }
    }
}
public typealias Node = TemplateNode
