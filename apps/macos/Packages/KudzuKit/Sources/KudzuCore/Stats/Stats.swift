import Foundation

public struct SnippetUsage: Hashable, Sendable {
    public var count: Int
    public var chars: Int
    /// Milliseconds since the Unix epoch.
    public var last: Double
    public init(count: Int = 0, chars: Int = 0, last: Double = 0) { self.count = count; self.chars = chars; self.last = last }
}
/// Usage records are per device; these helpers sum across devices. Typed text is never stored.
public enum Stats {
    /// Average word length used by typing-speed conventions.
    public static let CHARS_PER_WORD = 5
    public static let charsPerWord = CHARS_PER_WORD
    public struct Totals: Hashable, Sendable {
        public var expansions: Int
        public var chars: Int
        public var minutesSaved: Double
        public var activeSnippets: Int
        public var usedSnippets: Int
    }
    public static func usageById(_ library: Library) -> [UUID: SnippetUsage] {
        var result: [UUID: SnippetUsage] = [:]
        for u in library.usage {
            var cur = result[u.snippetID] ?? SnippetUsage()
            cur.count += u.count; cur.chars += max(0, u.charsSaved)
            cur.last = max(cur.last, (u.lastUsedAt?.timeIntervalSince1970 ?? 0) * 1000)
            result[u.snippetID] = cur
        }
        return result
    }
    public static func totals(_ library: Library, usage: [UUID: SnippetUsage], wpm: Double) -> Totals {
        let expansions = usage.values.reduce(0) { $0 + $1.count }, chars = usage.values.reduce(0) { $0 + $1.chars }
        return Totals(expansions: expansions, chars: chars, minutesSaved: Double(chars) / 5 / max(1, wpm), activeSnippets: library.snippets.filter(\.isEnabled).count, usedSnippets: usage.values.filter { $0.count > 0 }.count)
    }
    /// ECMAScript toFixed: ties round upward on the exact binary value (printf uses ties-to-even).
    private static func fixed(_ n: Double, _ places: Int) -> String {
        guard n.isFinite, abs(n) < 1e21 else { return String(n) }
        let bits = abs(n).bitPattern
        let exponentBits = Int((bits >> 52) & 0x7ff)
        let mantissa = (bits & ((1 << 52) - 1)) | (exponentBits == 0 ? 0 : 1 << 52)
        let exponent = (exponentBits == 0 ? 1 : exponentBits) - 1023 - 52
        let scaled = mantissa * (places == 0 ? 1 : 10)
        let rounded: UInt64
        if exponent >= 0 {
            guard exponent < 64, scaled <= UInt64.max >> exponent else { return String(format:"%.*f",locale:Locale(identifier:"en_US_POSIX"),places,n) }
            rounded = scaled << exponent
        } else {
            let shift = -exponent
            if shift >= 64 { rounded = 0 }
            else { rounded = (scaled >> shift) + ((scaled & ((1 << shift) - 1)) >= (1 << (shift - 1)) ? 1 : 0) }
        }
        let digits = String(rounded), sign = n < 0 ? "-" : ""
        if places == 0 { return sign + digits }
        let padded = String(repeating:"0",count:max(0,places + 1 - digits.count)) + digits
        return sign + padded.dropLast(places) + "." + padded.suffix(places)
    }
    public static func formatDuration(_ minutes: Double) -> String {
        if minutes < 1 { return "\(Int(floor(minutes * 60 + 0.5)))s" }
        if minutes < 60 { return "\(Int(floor(minutes + 0.5))) min" }
        let h = minutes / 60
        return h < 24 ? "\(fixed(h, h < 10 ? 1 : 0)) h" : "\(fixed(h / 24, 1)) days"
    }
    public static func formatCount(_ n: Int) -> String {
        if n < 1000 { return String(n) }
        if n < 1_000_000 { return fixed(Double(n) / 1000, n < 10_000 ? 1 : 0) + "k" }
        return fixed(Double(n) / 1_000_000, 1) + "M"
    }
    public static func relativeTime(_ ms: Double, now: Date = Date(), locale: Locale = .current) -> String {
        if ms == 0 { return "Never" }
        let diff = (ms - now.timeIntervalSince1970 * 1000) / 1000, magnitude = abs(diff)
        if magnitude < 60 { return "Just now" }
        if magnitude < 86_400 * 30 {
            var components = DateComponents()
            let value = Int(floor(diff / (magnitude < 3600 ? 60 : magnitude < 86_400 ? 3600 : 86_400) + 0.5))
            if magnitude < 3600 { components.minute = value } else if magnitude < 86_400 { components.hour = value } else { components.day = value }
            let formatter = RelativeDateTimeFormatter(); formatter.locale = locale; formatter.dateTimeStyle = .named; formatter.unitsStyle = .full
            return formatter.localizedString(from: components)
        }
        let formatter = DateFormatter(); formatter.locale = locale; formatter.setLocalizedDateFormatFromTemplate("MMMdyyyy")
        return formatter.string(from: Date(timeIntervalSince1970: ms / 1000))
    }
}
