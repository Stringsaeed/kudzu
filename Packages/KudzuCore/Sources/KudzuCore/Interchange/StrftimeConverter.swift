import Foundation

/// Converts between Espanso `strftime` patterns / offsets and Kudzu ICU date patterns / offsets.
public enum StrftimeConverter {
    public struct ConversionResult: Sendable, Equatable {
        public var icuFormat: String
        public var unsupportedSpecifiers: [String]

        public init(icuFormat: String, unsupportedSpecifiers: [String] = []) {
            self.icuFormat = icuFormat
            self.unsupportedSpecifiers = unsupportedSpecifiers
        }
    }

    /// Converts an Espanso `strftime` format string into an ICU date format string.
    public static func toICU(_ strftime: String) -> ConversionResult {
        var result = ""
        var literalBuffer = ""
        var unsupported: [String] = []

        func flushLiterals() {
            guard !literalBuffer.isEmpty else { return }
            result += escapeICULiteral(literalBuffer)
            literalBuffer = ""
        }

        var index = strftime.startIndex
        while index < strftime.endIndex {
            let ch = strftime[index]
            if ch == "%" {
                let nextIndex = strftime.index(after: index)
                guard nextIndex < strftime.endIndex else {
                    literalBuffer.append("%")
                    break
                }
                let spec = strftime[nextIndex]
                if spec == "%" {
                    literalBuffer.append("%")
                    index = strftime.index(after: nextIndex)
                    continue
                }
                if let icuToken = mapSpecifier(spec) {
                    flushLiterals()
                    result += icuToken
                } else {
                    let token = "%\(spec)"
                    unsupported.append(token)
                    literalBuffer += token
                }
                index = strftime.index(after: nextIndex)
            } else {
                literalBuffer.append(ch)
                index = strftime.index(after: index)
            }
        }
        flushLiterals()

        return ConversionResult(icuFormat: result, unsupportedSpecifiers: unsupported)
    }

    private static func mapSpecifier(_ spec: Character) -> String? {
        switch spec {
        case "Y": return "yyyy"
        case "y": return "yy"
        case "m": return "MM"
        case "d": return "dd"
        case "e": return "d"
        case "H": return "HH"
        case "I": return "hh"
        case "M": return "mm"
        case "S": return "ss"
        case "p": return "a"
        case "A": return "EEEE"
        case "a": return "EEE"
        case "B": return "MMMM"
        case "b": return "MMM"
        case "j": return "DDD"
        case "Z": return "zzz"
        case "z": return "xx"
        default: return nil
        }
    }

    private static func escapeICULiteral(_ text: String) -> String {
        var out = ""
        var inQuotes = false
        for ch in text {
            if ch == "'" {
                if inQuotes {
                    out += "''"
                } else {
                    out += "''"
                }
            } else if ch.isASCII && ch.isLetter {
                if !inQuotes {
                    out.append("'")
                    inQuotes = true
                }
                out.append(ch)
            } else {
                if inQuotes {
                    out.append("'")
                    inQuotes = false
                }
                out.append(ch)
            }
        }
        if inQuotes {
            out.append("'")
        }
        return out
    }

    /// Converts an ICU date format string back into an Espanso `strftime` format string.
    public static func toStrftime(_ icu: String) -> String {
        var out = ""
        let chars = Array(icu)
        var i = 0

        while i < chars.count {
            let ch = chars[i]
            if ch == "'" {
                if i + 1 < chars.count && chars[i + 1] == "'" {
                    out.append("'")
                    i += 2
                    continue
                }
                i += 1
                while i < chars.count {
                    if chars[i] == "'" {
                        if i + 1 < chars.count && chars[i + 1] == "'" {
                            out.append("'")
                            i += 2
                        } else {
                            i += 1
                            break
                        }
                    } else {
                        if chars[i] == "%" {
                            out += "%%"
                        } else {
                            out.append(chars[i])
                        }
                        i += 1
                    }
                }
                continue
            }

            if ch == "%" {
                out += "%%"
                i += 1
                continue
            }

            if ch.isASCII && ch.isLetter {
                let start = i
                while i < chars.count && chars[i] == ch {
                    i += 1
                }
                let runLength = i - start
                let token = String(repeating: String(ch), count: runLength)
                out += mapICUTokenToStrftime(token, char: ch, count: runLength)
            } else {
                out.append(ch)
                i += 1
            }
        }

        return out
    }

    private static func mapICUTokenToStrftime(_ token: String, char: Character, count: Int) -> String {
        switch (char, count) {
        case ("y", 2): return "%y"
        case ("y", _): return "%Y"
        case ("M", 4...): return "%B"
        case ("M", 3): return "%b"
        case ("M", _): return "%m"
        case ("d", 2...): return "%d"
        case ("d", 1): return "%e"
        case ("H", _): return "%H"
        case ("h", _): return "%I"
        case ("m", _): return "%M"
        case ("s", _): return "%S"
        case ("a", _): return "%p"
        case ("E", 4...): return "%A"
        case ("E", _): return "%a"
        case ("D", _): return "%j"
        case ("z", _): return "%Z"
        case ("x", _), ("X", _), ("Z", _): return "%z"
        default: return token
        }
    }

    /// Converts an Espanso date offset in seconds into a Kudzu offset string (e.g. `+1d`, `-4h`, `+1w2d`).
    public static func secondsToKudzuOffset(_ seconds: Int) -> String? {
        guard seconds != 0 else { return nil }
        let sign = seconds > 0 ? "+" : "-"
        var rem = abs(seconds)

        // Decompose into weeks (604800), days (86400), hours (3600), minutes (60)
        var parts: [String] = []
        let weeks = rem / 604_800
        if weeks > 0 {
            // If it's a clean number of days (< 7) we don't enter here; if >= 7, use w and d
            parts.append("\(weeks)w")
            rem %= 604_800
        }
        let days = rem / 86_400
        if days > 0 {
            parts.append("\(days)d")
            rem %= 86_400
        }
        let hours = rem / 3_600
        if hours > 0 {
            parts.append("\(hours)h")
            rem %= 3_600
        }
        let minutes = max(rem > 0 && parts.isEmpty ? 1 : 0, rem / 60)
        if minutes > 0 {
            parts.append("\(minutes)min")
        }

        guard !parts.isEmpty else { return nil }
        return sign + parts.joined()
    }

    /// Converts a Kudzu offset string (e.g. `+3d`, `-1w2d`, `+30min`) into seconds for Espanso export.
    public static func kudzuOffsetToSeconds(_ offset: String) -> Int? {
        let trimmed = offset.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return nil }

        var chars = Array(trimmed)
        var sign = 1
        if chars.first == "+" {
            chars.removeFirst()
        } else if chars.first == "-" {
            sign = -1
            chars.removeFirst()
        }

        var totalSeconds = 0
        var i = 0
        var parsedAny = false
        while i < chars.count {
            var numStr = ""
            while i < chars.count && chars[i].isNumber {
                numStr.append(chars[i])
                i += 1
            }
            var unitStr = ""
            while i < chars.count && chars[i].isLetter {
                unitStr.append(chars[i])
                i += 1
            }
            guard let value = Int(numStr), !unitStr.isEmpty else {
                return nil
            }
            parsedAny = true
            switch unitStr {
            case "w": totalSeconds += value * 604_800
            case "d": totalSeconds += value * 86_400
            case "h": totalSeconds += value * 3_600
            case "min": totalSeconds += value * 60
            case "m": totalSeconds += value * 30 * 86_400
            case "y": totalSeconds += value * 365 * 86_400
            default: return nil
            }
        }

        return parsedAny ? sign * totalSeconds : nil
    }
}
