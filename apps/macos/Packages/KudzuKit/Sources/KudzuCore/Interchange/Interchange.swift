import Foundation

/// Errors reading, writing or decoding interchange files.
public enum InterchangeError: Error, LocalizedError, Equatable {
    case unsupportedFormatVersion(found: Int, maxSupported: Int)
    case invalidSource(String), decoding(String), encoding(String), io(String)
    public var errorDescription: String? {
        switch self {
        case .unsupportedFormatVersion(let found,let max): "Unsupported Kudzu library format version \(found). Maximum supported version is \(max)."
        case .invalidSource(let message), .io(let message): message
        case .decoding(let message): "Failed to decode Kudzu library: \(message)"
        case .encoding(let message): "Failed to encode Kudzu library: \(message)"
        }
    }
}
/// A library converted from an export, plus everything that did not convert cleanly.
public struct ImportResult: Codable, Equatable, Sendable {
    public var library: Library
    public var warnings: [ImportWarning]
    public init(library: Library = .empty, warnings: [ImportWarning] = []) { self.library = library; self.warnings = warnings }
}
extension KudzuJSON {
    /// Human-readable, sorted library JSON, compatible with the Tauri export.
    public static func encodeLibrary(_ library: Library) throws -> String {
        do { return String(decoding:try encode(library),as:UTF8.self) } catch { throw InterchangeError.encoding(error.localizedDescription) }
    }
    /// Rejects future versions before decoding; unknown keys remain forward compatible.
    public static func decodeLibrary(_ json: String) throws -> Library {
        let data = Data(json.utf8)
        do {
            if let v = try JSONSerialization.jsonObject(with:data) as? [String:Any], let n = v["formatVersion"] as? Int, n > Library.currentFormatVersion { throw InterchangeError.unsupportedFormatVersion(found:n,maxSupported:Library.currentFormatVersion) }
            return try decode(Library.self,from:data)
        } catch let e as InterchangeError { throw e } catch { throw InterchangeError.decoding(error.localizedDescription) }
    }
}
/// Buffers literal runs so adjacent braces and backslashes cannot form an accidental tag.
struct TemplateOut {
    var out = "", literal = ""
    mutating func text(_ text: String) { literal += text }
    mutating func pushChar(_ ch: Character) { literal.append(ch) }
    mutating func tag(_ source: String) { flush(); out += source }
    mutating func flush() { if !literal.isEmpty { out += Template.serialize([.text(literal,range:0..<0)]); literal = "" } }
    mutating func finish() -> String { flush(); return out }
}
enum ImportTags {
    static func tag(_ name: String, _ attrs: [Attribute] = []) -> String { Template.serialize([.tag(Tag(name:name,attributes:attrs))]) }
    static func string(_ key: String, _ value: String) -> Attribute { Attribute(name:key,value:.string(value)) }
    static func date(_ format: String? = nil, _ offset: String? = nil, _ locale: String? = nil) -> String {
        tag("date",zip(["format","offset","locale"],[format,offset,locale]).compactMap { key,value in value.flatMap { $0.isEmpty ? nil : string(key,$0) } })
    }
    static func clipboard(_ trim: Bool = false) -> String { tag("clipboard",trim ? [Attribute(name:"trim",value:.bool(true))] : []) }
    static func cursor() -> String { tag("cursor") }
    static func snippet(_ id: UUID) -> String { tag("snippet",[string("id",id.uuidString)]) }
    static func field(_ name: String, _ value: String? = nil, _ multiline: Bool = false) -> String {
        var attrs = [string("name",name)]; if let value, !value.isEmpty { attrs.append(string("default",value)) }; if multiline { attrs.append(Attribute(name:"multiline",value:.bool(true))) }; return tag("field",attrs)
    }
    static func choice(_ name: String, _ options: [String], _ value: String? = nil) -> String {
        var attrs = [string("name",name),string("options",options.map { $0.replacingOccurrences(of:"|",with:"\\|") }.joined(separator:"|"))]
        if let value, !value.isEmpty { attrs.append(string("default",value)) }; return tag("choice",attrs)
    }
}
