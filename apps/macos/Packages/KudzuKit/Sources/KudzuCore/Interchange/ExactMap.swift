import Foundation

/// A string map keyed by UTF-8 bytes, matching Rust `HashMap<String, _>` / `BTreeMap<String, _>`.
/// Swift `Dictionary` keys use canonical equivalence, so `é` and `e` + U+0301 would share one entry.
public struct ExactMap<Value: Equatable & Sendable>: Equatable, Sendable {
    private var storage: [(key: String, value: Value)] = []

    public init() {}

    public var count: Int { storage.count }
    public var isEmpty: Bool { storage.isEmpty }
    public var keys: [String] { storage.map(\.key) }
    public var elements: [(key: String, value: Value)] { storage }

    public subscript(key: String) -> Value? {
        get { storage.first { $0.key.isExactly(key) }?.value }
        set {
            if let index = storage.firstIndex(where: { $0.key.isExactly(key) }) {
                if let newValue { storage[index].value = newValue } else { storage.remove(at: index) }
            } else if let newValue {
                storage.append((key, newValue))
            }
        }
    }

    public mutating func removeValue(forKey key: String) {
        storage.removeAll { $0.key.isExactly(key) }
    }

    public mutating func merge(_ other: ExactMap, uniquingKeysWith combine: (Value, Value) -> Value) {
        for item in other.storage {
            if let current = self[item.key] { self[item.key] = combine(current, item.value) }
            else { self[item.key] = item.value }
        }
    }

    public static func == (lhs: Self, rhs: Self) -> Bool {
        guard lhs.count == rhs.count else { return false }
        return lhs.storage.allSatisfy { item in
            rhs.storage.contains { $0.key.isExactly(item.key) && $0.value == item.value }
        }
    }
}

extension ExactMap: Codable where Value: Codable {
    private struct DynamicKey: CodingKey {
        var stringValue: String
        init?(stringValue: String) { self.stringValue = stringValue }
        var intValue: Int? { nil }
        init?(intValue: Int) { nil }
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: DynamicKey.self)
        self.init()
        for key in container.allKeys {
            self[key.stringValue] = try container.decode(Value.self, forKey: key)
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: DynamicKey.self)
        for item in storage {
            try container.encode(item.value, forKey: DynamicKey(stringValue: item.key)!)
        }
    }
}
