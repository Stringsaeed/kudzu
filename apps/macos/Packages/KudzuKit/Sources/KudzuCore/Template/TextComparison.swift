import Foundation

// MARK: - Exact keys

extension String {
    /// The string's UTF-8 bytes, for comparisons that must match Rust `String` equality and `HashMap<String, _>`
    /// keys. Swift `String ==` and `Dictionary` keys use canonical equivalence, so `é` (U+00E9) and
    /// `e` + U+0301 would merge; form field names and option values are distinct when their bytes differ.
    /// (Trigger matching is different: `kudzu-core` compares triggers in NFC on purpose, which `Character`
    /// equality reproduces.)
    var exactKey: [UInt8] { Array(utf8) }

    /// Byte equality, as Rust's `==` on `str`.
    func isExactly(_ other: String) -> Bool { utf8.elementsEqual(other.utf8) }
}

extension Dictionary where Key == String {
    /// The value stored under a key with exactly these bytes. A `Dictionary` keeps the bytes of the key it
    /// stored, but a lookup would also find a canonically equivalent one.
    func exactValue(for key: String) -> Value? {
        guard let index = index(forKey: key), self[index].key.isExactly(key) else { return nil }
        return self[index].value
    }
}

// MARK: - ECMAScript whitespace

/// JavaScript's `\s` and `String.prototype.trim` set (WhiteSpace + LineTerminator), used by the ports of
/// `template.ts` and `search.ts`. It includes U+FEFF but, unlike Unicode `White_Space`, not U+0085.
enum ECMAScriptWhitespace {
    static func contains(_ scalar: Unicode.Scalar) -> Bool {
        switch scalar.value {
        case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000, 0xFEFF: true
        default: false
        }
    }

    /// `text.trim()`.
    static func trim(_ text: String) -> String {
        let scalars = text.unicodeScalars
        guard let first = scalars.firstIndex(where: { !contains($0) }), let last = scalars.lastIndex(where: { !contains($0) }) else { return "" }
        return String(scalars[first...last])
    }

    /// `text.split(/\s+/)` on already-trimmed text, without empty pieces.
    static func split(_ text: String) -> [String] {
        text.unicodeScalars.split(whereSeparator: contains).map { String(String.UnicodeScalarView($0)) }
    }

    /// `text.replace(/\s+/g, " ").trim()`.
    static func collapse(_ text: String) -> String { split(text).joined(separator: " ") }
}
