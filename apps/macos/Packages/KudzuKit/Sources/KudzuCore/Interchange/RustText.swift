import Foundation

// Rust `str`/`char` semantics for the T2 ports (interchange, secrets, clipboard, sync, agent).
//
// Swift `String` compares by canonical equivalence and walks `Character`s (grapheme clusters), so
// `"$\u{301}".hasPrefix("$")` is false and `",\u{301}"` is not the `Character` `","`. Rust walks bytes
// and `char`s (Unicode scalars). Code ported from `kudzu-core` uses these helpers wherever Rust's answer
// depends on bytes or scalars, so a combining mark after a delimiter can't change the result.

enum RustText {
    // MARK: char classes

    /// `char::is_whitespace`: the Unicode `White_Space` property.
    static func isWhitespace(_ c: Unicode.Scalar) -> Bool { c.properties.isWhitespace }

    /// `char::is_numeric`: general categories Nd, Nl and No (not every scalar with a numeric value).
    static func isNumeric(_ c: Unicode.Scalar) -> Bool {
        switch c.properties.generalCategory {
        case .decimalNumber, .letterNumber, .otherNumber: true
        default: false
        }
    }

    /// `char::is_alphanumeric`: `Alphabetic` or numeric.
    static func isAlphanumeric(_ c: Unicode.Scalar) -> Bool { c.properties.isAlphabetic || isNumeric(c) }

    /// `u8::is_ascii_alphanumeric`.
    static func isASCIIAlphanumeric(_ b: UInt8) -> Bool { (48...57).contains(b) || (65...90).contains(b) || (97...122).contains(b) }

    // MARK: str methods

    /// One `Character` per Unicode scalar, so a combining mark does not fuse with the scalar before it.
    static func chars(_ s: String) -> [Character] { s.unicodeScalars.map { Character($0) } }

    /// `str::trim` (Unicode `White_Space`, by scalar).
    static func trim(_ s: String) -> String { trimEnd(trimStart(s)) }

    /// `str::trim_start`.
    static func trimStart(_ s: String) -> String {
        let scalars = s.unicodeScalars
        return String(scalars[(scalars.firstIndex { !isWhitespace($0) } ?? scalars.endIndex)...])
    }

    /// `str::trim_end`.
    static func trimEnd(_ s: String) -> String {
        let scalars = s.unicodeScalars
        guard let last = scalars.lastIndex(where: { !isWhitespace($0) }) else { return "" }
        return String(scalars[...last])
    }

    /// `str::split_whitespace`.
    static func splitWhitespace(_ s: String) -> [String] {
        s.unicodeScalars.split(whereSeparator: isWhitespace).map { String(String.UnicodeScalarView($0)) }
    }

    /// `str::lines`: split at `\n`, dropping one `\r` before each `\n`; a final unterminated line keeps
    /// a bare `\r`, and a trailing `\n` adds no empty line.
    static func lines(_ s: String) -> [String] {
        var out: [String] = [], line: [UInt8] = []
        for b in s.utf8 {
            if b == UInt8(ascii: "\n") {
                if line.last == UInt8(ascii: "\r") { line.removeLast() }
                out.append(String(decoding: line, as: UTF8.self)); line = []
            } else {
                line.append(b)
            }
        }
        if !line.isEmpty { out.append(String(decoding: line, as: UTF8.self)) }
        return out
    }

    /// `String::from_utf8`: `nil` on invalid UTF-8, and a leading BOM is kept (Foundation's
    /// `String(data:encoding: .utf8)` drops it).
    static func utf8String<Bytes: Collection>(_ bytes: Bytes) -> String? where Bytes.Element == UInt8 {
        let s = String(decoding: bytes, as: UTF8.self)
        return s.utf8.elementsEqual(bytes) ? s : nil
    }

    /// `str::to_ascii_lowercase`.
    static func asciiLowercased(_ s: String) -> String {
        String(decoding: s.utf8.map { (65...90).contains($0) ? $0 + 32 : $0 }, as: UTF8.self)
    }

    /// `str::starts_with` on bytes.
    static func hasPrefix(_ s: String, _ prefix: String) -> Bool { s.utf8.starts(with: prefix.utf8) }

    /// `str::ends_with` on bytes.
    static func hasSuffix(_ s: String, _ suffix: String) -> Bool { s.utf8.reversed().starts(with: suffix.utf8.reversed()) }

    /// Drop `n` Unicode scalars. `String.dropFirst` drops grapheme clusters and can swallow a combining mark.
    static func dropFirst(_ s: String, _ n: Int) -> String { String(s.unicodeScalars.dropFirst(n)) }

    /// `str::split` on one scalar. `maxSplits` is the number of splits, matching `splitn(maxSplits + 1)`.
    static func split(_ s: String, separator: Unicode.Scalar, maxSplits: Int = Int.max, omittingEmptySubsequences: Bool = true) -> [String] {
        Array(s.unicodeScalars).split(separator: separator, maxSplits: maxSplits, omittingEmptySubsequences: omittingEmptySubsequences).map { String(String.UnicodeScalarView($0)) }
    }

    /// `str::split` on a scalar predicate.
    static func split(_ s: String, where isSeparator: (Unicode.Scalar) -> Bool, omittingEmptySubsequences: Bool = true) -> [String] {
        s.unicodeScalars.split(omittingEmptySubsequences: omittingEmptySubsequences, whereSeparator: isSeparator).map { String(String.UnicodeScalarView($0)) }
    }

    /// `str::contains(&str)` on bytes.
    static func contains(_ s: String, _ needle: String) -> Bool { find(Array(needle.utf8), in: Array(s.utf8)) != nil }

    /// The first byte offset of `needle` in `bytes[from..<to]`, like `str::find` on a slice.
    static func find(_ needle: [UInt8], in bytes: [UInt8], from: Int = 0, to: Int? = nil) -> Int? {
        let to = to ?? bytes.count
        guard let first = needle.first else { return from <= to ? from : nil }
        let last = to - needle.count
        guard from <= last else { return nil }
        var i = from
        while i <= last {
            if bytes[i] == first {
                var k = 1
                while k < needle.count && bytes[i + k] == needle[k] { k += 1 }
                if k == needle.count { return i }
            }
            i += 1
        }
        return nil
    }

    /// The scalar that ends at byte `end` of valid UTF-8, with its start offset.
    static func scalar(before end: Int, in bytes: [UInt8]) -> (Unicode.Scalar, Int)? {
        guard end > 0 else { return nil }
        var start = end - 1
        while start > 0 && bytes[start] & 0xC0 == 0x80 { start -= 1 }
        return String(decoding: bytes[start..<end], as: UTF8.self).unicodeScalars.first.map { ($0, start) }
    }

    /// The scalar that starts at byte `start` of valid UTF-8 (a char boundary), with its width in bytes.
    static func scalar(at start: Int, in bytes: [UInt8]) -> (Unicode.Scalar, Int)? {
        guard start < bytes.count else { return nil }
        let lead = bytes[start]
        let width = lead < 0x80 ? 1 : lead < 0xE0 ? 2 : lead < 0xF0 ? 3 : 4
        guard start + width <= bytes.count else { return nil }
        return String(decoding: bytes[start..<(start + width)], as: UTF8.self).unicodeScalars.first.map { ($0, width) }
    }

    /// The byte offset of the first scalar in `bytes[from...]` (from a char boundary) matching `stop`,
    /// or `bytes.count`: `str::find(|c| …)` on a slice, without decoding past the match.
    static func firstScalar(in bytes: [UInt8], from: Int, where stop: (Unicode.Scalar) -> Bool) -> Int {
        var i = from
        while let (c, width) = scalar(at: i, in: bytes) {
            if stop(c) { return i }
            i += width
        }
        return bytes.count
    }
}
