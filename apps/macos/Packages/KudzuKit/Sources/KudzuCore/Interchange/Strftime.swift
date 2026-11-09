// Ported from kudzu-core/src/interchange/strftime.rs
// Conversion between Espanso `strftime` patterns / second offsets and Kudzu ICU patterns / offsets.

import Foundation

/// Conversion between Espanso strftime/second offsets and Kudzu ICU/calendar offsets.
public enum Strftime {
    public struct ConversionResult: Equatable, Sendable { public var icuFormat: String; public var unsupportedSpecifiers: [String] }
    static let specifiers: [Character:String] = ["Y":"yyyy","y":"yy","m":"MM","d":"dd","e":"d","H":"HH","I":"hh","M":"mm","S":"ss","p":"a","A":"EEEE","a":"EEE","B":"MMMM","b":"MMM","j":"DDD","Z":"zzz","z":"xx"]
    public static func toICU(_ strftime: String) -> ConversionResult {
        let chars = strftime.unicodeScalars.map { Character(String($0)) }; var out = "", literal = "", unsupported: [String] = [], i = 0
        func flush() { out += escapeICULiteral(literal); literal = "" }
        while i < chars.count {
            let ch = chars[i]; i += 1
            if ch != "%" { literal.append(ch); continue }
            if i == chars.count { literal += "%"; break }
            let spec = chars[i]; i += 1
            if spec == "%" { literal += "%" }
            else if let token = specifiers[spec] { flush(); out += token }
            else { let token = "%\(spec)"; literal += token; unsupported.append(token) }
        }
        flush(); return ConversionResult(icuFormat:out,unsupportedSpecifiers:unsupported)
    }
    static func escapeICULiteral(_ text: String) -> String {
        var out = "", quoted = false
        for ch in text {
            if ch == "'" { out += "''" }
            else if ch.isASCII && ch.isLetter { if !quoted { out += "'"; quoted = true }; out.append(ch) }
            else { if quoted { out += "'"; quoted = false }; out.append(ch) }
        }
        if quoted { out += "'" }; return out
    }
    /// Converts an ICU date pattern back into an Espanso `strftime` format string.
    public static func toStrftime(_ icu: String) -> String {
        let chars = icu.unicodeScalars.map { Character(String($0)) }; var i = 0, out = ""
        while i < chars.count {
            let ch = chars[i]
            if ch == "'" {
                if i+1 < chars.count && chars[i+1] == "'" { out += "'"; i += 2; continue }
                i += 1
                while i < chars.count {
                    if chars[i] == "'" {
                        if i+1 < chars.count && chars[i+1] == "'" { out += "'"; i += 2 } else { i += 1; break }
                    } else { out += chars[i] == "%" ? "%%" : String(chars[i]); i += 1 }
                }; continue
            }
            if ch == "%" { out += "%%"; i += 1; continue }
            if ch.isASCII && ch.isLetter {
                let start = i; while i < chars.count && chars[i] == ch { i += 1 }; let n = i-start
                let token: String?
                switch ch {
                case "y": token = n == 2 ? "%y" : "%Y"
                case "M": token = n >= 4 ? "%B" : n == 3 ? "%b" : "%m"
                case "d": token = n >= 2 ? "%d" : "%e"
                case "H": token = "%H"
                case "h": token = "%I"
                case "m": token = "%M"
                case "s": token = "%S"
                case "a": token = "%p"
                case "E": token = n >= 4 ? "%A" : "%a"
                case "D": token = "%j"
                case "z": token = "%Z"
                case "x","X","Z": token = "%z"
                default: token = nil
                }
                out += token ?? String(repeating:String(ch),count:n)
            } else { out.append(ch); i += 1 }
        }; return out
    }
    /// Converts an Espanso offset in seconds into a Kudzu offset (`+1d`, `-4h`, `+1w2d`); `None` for zero.
    public static func secondsToKudzuOffset(_ seconds: Int64) -> String? {
        if seconds == 0 { return nil }
        var rem = seconds.magnitude, parts: [String] = []
        for (unit,size): (String,UInt64) in [("w",604800),("d",86400),("h",3600)] {
            let n = rem/size; if n > 0 { parts.append("\(n)\(unit)"); rem %= size }
        }
        let minutes = max(rem/60,rem > 0 && parts.isEmpty ? 1 : 0)
        if minutes > 0 { parts.append("\(minutes)min") }
        return parts.isEmpty ? nil : (seconds > 0 ? "+" : "-") + parts.joined()
    }
    /// Converts a Kudzu offset (`+3d`, `-1w2d`, `+30min`) into seconds for Espanso export.
    ///
    /// Months and years are approximated as 30 and 365 days, since Espanso offsets are plain seconds.
    public static func kudzuOffsetToSeconds(_ offset: String) -> Int64? {
        var scalars = Array(RustText.trim(offset).unicodeScalars), sign: Int64 = 1
        if let first = scalars.first, first == "+" || first == "-" { sign = first == "-" ? -1 : 1; scalars.removeFirst() }
        let chars = scalars.map { Character($0) }; var i = 0, total: Int64 = 0, any = false
        while i < chars.count {
            let start = i; while i < chars.count && chars[i].isASCII && chars[i].isNumber { i += 1 }
            guard let n = Int64(String(chars[start..<i])) else { return nil }
            let u = i; while i < chars.count && chars[i].isLetter { i += 1 }
            guard let size = ["w":604800,"d":86400,"h":3600,"min":60,"m":2592000,"y":31536000][String(chars[u..<i])] else { return nil }
            let mul = n.multipliedReportingOverflow(by:Int64(size)), sum = total.addingReportingOverflow(mul.partialValue)
            if mul.overflow || sum.overflow { return nil }; total = sum.partialValue; any = true
        }
        return any ? sign*total : nil
    }
}
