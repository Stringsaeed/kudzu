import CoreFoundation
import Foundation

extension Agent {
    /// Agent dictionaries contain only JSON values. Encode through JSONEncoder so pretty text
    /// uses serde's `"key": value` layout, including nested arrays and empty containers.
    struct JSONValue: Encodable {
        let value: Any
        init(_ value: Any) { self.value = value }

        /// Apple's JSONEncoder pretty writer also inserts a space before colons and expands
        /// empty arrays. Emit container layout directly; JSONEncoder still escapes every scalar.
        /// This builds the JSON, rather than post-processing serialized user text.
        func pretty(depth: Int = 0) throws -> String {
            let indent = String(repeating: "  ", count: depth)
            let child = indent + "  "
            if let object = value as? [String: Any] {
                if object.isEmpty { return "{}" }
                let keys = object.keys.sorted { $0.utf8.lexicographicallyPrecedes($1.utf8) }
                let rows = try keys.map { key in
                    let name = String(decoding: try KudzuJSON.encode(key, pretty: false), as: UTF8.self)
                    return child + name + ": " + (try JSONValue(object[key] ?? NSNull()).pretty(depth: depth + 1))
                }
                return "{\n" + rows.joined(separator: ",\n") + "\n" + indent + "}"
            }
            if let array = value as? [Any] {
                if array.isEmpty { return "[]" }
                let rows = try array.map { child + (try JSONValue($0).pretty(depth: depth + 1)) }
                return "[\n" + rows.joined(separator: ",\n") + "\n" + indent + "]"
            }
            return String(decoding: try KudzuJSON.encode(self, pretty: false), as: UTF8.self)
        }

        func encode(to encoder: any Encoder) throws {
            var container = encoder.singleValueContainer()
            switch value {
            case is NSNull: try container.encodeNil()
            case let object as [String: Any]: try container.encode(object.mapValues(JSONValue.init))
            case let array as [Any]: try container.encode(array.map(JSONValue.init))
            case let string as String: try container.encode(string)
            case let number as NSNumber:
                if CFGetTypeID(number) == CFBooleanGetTypeID() { try container.encode(number.boolValue) }
                else if ["f", "d"].contains(String(cString: number.objCType)) { try container.encode(number.doubleValue) }
                else if number.stringValue.hasPrefix("-") { try container.encode(number.int64Value) }
                else { try container.encode(number.uint64Value) }
            default:
                throw EncodingError.invalidValue(value, .init(codingPath: encoder.codingPath, debugDescription: "Expected a JSON value"))
            }
        }
    }

    static func json(_ value: Any) -> String {
        // Values are built here from JSON dictionaries, never arbitrary model instances.
        guard let data = try? KudzuJSON.encode(JSONValue(value), pretty: false) else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }
}
