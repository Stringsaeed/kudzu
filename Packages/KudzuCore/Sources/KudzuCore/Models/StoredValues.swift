import Foundation

// CloudKit-safe encodings for values that aren't plain scalars.
//
// Why JSON strings rather than SwiftData's native `[String]` / Codable
// composite attributes:
// - The CloudKit record field is a plain STRING we fully control. SwiftData's
//   transformable encoding of arrays and composites is private and has changed
//   across OS releases; a schema that is additive-only forever shouldn't depend
//   on it.
// - Decoding is lenient: values written by a newer client (an unknown enum case,
//   an extra key) degrade to defaults instead of failing the whole fetch.
// - It's readable in the CloudKit dashboard, which helps debugging sync.
// Triggers and rules are tiny, far below CloudKit's per-record size limit.
enum StoredValues {
    private static let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        // Stable output, so re-encoding an unchanged value doesn't dirty the record.
        encoder.outputFormatting = [.sortedKeys]
        return encoder
    }()

    static func encode<T: Encodable>(_ value: T) -> String {
        // Encoding [String] / plain structs can't fail.
        (try? String(decoding: encoder.encode(value), as: UTF8.self)) ?? ""
    }

    static func decode<T: Decodable>(_ type: T.Type, from string: String?) -> T? {
        guard let string, let data = string.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(type, from: data)
    }

    static func encodeStrings(_ strings: [String]) -> String { encode(strings) }

    static func decodeStrings(_ string: String?) -> [String] {
        decode([String].self, from: string) ?? []
    }

    static func encodeUUIDs(_ ids: [UUID]) -> String { encode(ids.map(\.uuidString)) }

    static func decodeUUIDs(_ string: String?) -> [UUID] {
        decodeStrings(string).compactMap(UUID.init(uuidString:))
    }

    /// Mirror of `AppRule` with every field optional and enums as raw strings,
    /// so a rule written by a newer client still decodes.
    private struct StoredAppRule: Codable {
        var mode: String?
        var bundleIDs: [String]?
        var insertion: String?
    }

    static func encodeRule(_ rule: AppRule?) -> String? {
        guard let rule else { return nil }
        return encode(StoredAppRule(mode: rule.mode.rawValue, bundleIDs: rule.bundleIDs, insertion: rule.insertion?.rawValue))
    }

    /// An unknown rule mode yields nil (inherit from the parent scope) rather than guessing allow/deny.
    static func decodeRule(_ string: String?) -> AppRule? {
        guard let stored = decode(StoredAppRule.self, from: string),
              let mode = stored.mode.flatMap(AppRuleMode.init(rawValue:))
        else { return nil }
        return AppRule(
            mode: mode,
            bundleIDs: stored.bundleIDs ?? [],
            insertion: stored.insertion.flatMap(InsertionMethod.init(rawValue:))
        )
    }
}

extension Optional where Wrapped == String {
    /// Parses an optional raw enum value; unknown values become nil.
    func decodedEnum<E: RawRepresentable>(_: E.Type) -> E? where E.RawValue == String {
        flatMap(E.init(rawValue:))
    }
}
