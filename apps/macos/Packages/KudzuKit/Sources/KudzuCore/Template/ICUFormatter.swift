import Foundation

extension Template {
    /// Formats a fixed-offset instant with ICU patterns. Unsupported letters are emitted literally.
    /// Names, Latin digits, ISO weeks and GMT zones match the Rust formatter, including its fallbacks.
    public static func formatICU(_ date: Date, pattern: String, locale: String, timeZone: TimeZone = .gmt, nanosecond: Int? = nil) -> String {
        let formatter = ICUFormatter(date: date, locale: locale, timeZone: timeZone, nanosecond: nanosecond)
        let chars = Array(pattern.unicodeScalars); var i = 0, out = ""
        while i < chars.count {
            let c = chars[i]
            if c == "'" {
                if i + 1 < chars.count, chars[i + 1] == "'" { out += "'"; i += 2; continue }
                i += 1
                while i < chars.count {
                    if chars[i] == "'" {
                        if i + 1 < chars.count, chars[i + 1] == "'" { out += "'"; i += 2; continue }
                        i += 1; break
                    }
                    out.unicodeScalars.append(chars[i]); i += 1
                }
            } else if c.isASCII, CharacterSet.letters.contains(c) {
                var count = 1
                while i + count < chars.count, chars[i + count] == c { count += 1 }
                out += formatter.field(Character(c), count); i += count
            } else { out.unicodeScalars.append(c); i += 1 }
        }
        return out
    }
}

private struct ICUFormatter {
    var date: Date
    var calendar: Calendar
    var iso: Calendar
    var names: ICUNames
    var sundayFirst: Bool
    var timeZone: TimeZone
    var civil: GregorianDate
    var nanos: Int
    init(date: Date, locale: String, timeZone: TimeZone, nanosecond: Int?) {
        // Split the instant into whole seconds and a fraction that agree with each other. A `Date` holds
        // `x.999999999` as `x + 1`, so with exact nanoseconds the whole second is what remains after them.
        // Without them the fraction is rounded to microseconds but stays in its second (`x.9999996` is
        // `x.999999`, as Chrono prints it, not `x.000000`).
        let t = date.timeIntervalSince1970
        var whole = floor(t)
        let nanos: Int
        if let nanosecond {
            nanos = nanosecond
            whole = (t - Double(nanosecond) / 1_000_000_000).rounded()
        } else {
            nanos = min(Int(((t - whole) * 1_000_000).rounded()), 999_999) * 1000
        }
        let instant = Date(timeIntervalSince1970: whole)
        self.date = instant; self.timeZone = timeZone; self.nanos = nanos
        self.civil = GregorianDate(instant, offset: timeZone.secondsFromGMT(for: instant))
        calendar = Calendar(identifier: .gregorian); calendar.timeZone = timeZone
        iso = Calendar(identifier: .iso8601); iso.timeZone = timeZone
        let parts = locale.components(separatedBy: CharacterSet(charactersIn: "_-@."))
        let language = (parts.first ?? "").lowercased()
        let region = parts.dropFirst().first { $0.count == 2 || $0.count == 3 }?.uppercased()
        sundayFirst = region.map { ["US", "CA", "JP", "BR", "MX", "IL", "KR", "TW", "PH", "IN", "ZA"].contains($0) } ?? (language == "en" || language.isEmpty)
        switch language {
        case "fr": names = ICUNameTables.fr
        case "de": names = ICUNameTables.de
        case "es": names = ICUNameTables.es
        case "it": names = ICUNameTables.it
        case "pt": names = ICUNameTables.pt
        case "nl": names = ICUNameTables.nl
        case "ar": names = ICUNameTables.ar
        default: names = ICUNameTables.en
        }
    }
    func value(_ component: Calendar.Component) -> Int {
        switch component {
        case .year: return civil.year > 0 ? civil.year : 1 - civil.year
        case .era: return civil.year > 0 ? 1 : 0
        case .month: return civil.month
        case .day: return civil.day
        case .hour: return civil.hour
        case .minute: return civil.minute
        case .second: return civil.second
        case .weekday: return civil.weekday
        default: return calendar.component(component,from:date)
        }
    }
    func pad(_ value: Int, _ width: Int) -> String {
        let text = String(value.magnitude)
        return (value < 0 ? "-" : "") + String(repeating: "0", count: max(0, width - text.count)) + text
    }
    func first(_ text: String) -> String { text.unicodeScalars.first.map { String($0).uppercased() } ?? "" }
    func weekday(_ count: Int) -> String {
        let i = value(.weekday) - 1
        switch count {
        case 1...3: return names.weekdays_abbr[i]
        case 4: return names.weekdays[i]
        case 5: return first(names.weekdays[i])
        default: return String(names.weekdays_abbr[i].unicodeScalars.filter { $0 != "." }.prefix(2))
        }
    }
    func zone(_ colon: Bool, _ short: Bool) -> String {
        let seconds = timeZone.secondsFromGMT(for: date), minutes = abs(seconds) / 60
        let h = minutes / 60, m = minutes % 60
        return (seconds < 0 ? "-" : "+") + pad(h, 2) + (short && m == 0 ? "" : (colon ? ":" : "") + pad(m, 2))
    }
    func gmt(_ long: Bool) -> String {
        let seconds = timeZone.secondsFromGMT(for: date)
        if seconds == 0 { return "GMT" }
        if long { return "GMT" + zone(true, false) }
        let minutes = abs(seconds) / 60
        return "GMT" + (seconds < 0 ? "-" : "+") + String(minutes / 60) + (minutes % 60 == 0 ? "" : ":" + pad(minutes % 60, 2))
    }
    func field(_ c: Character, _ count: Int) -> String {
        let hour = value(.hour), year = value(.year), month = value(.month), day = value(.day)
        switch c {
        case "G":
            let index = value(.era) == 0 ? 0 : 1
            return count <= 3 ? names.eras[index] : count == 4 ? names.eras_full[index] : first(names.eras[index])
        case "y": return pad(count == 2 ? year % 100 : year, count)
        case "u": return pad(value(.era) == 0 ? 1 - year : year, count)
        case "Y":
            let y = civil.iso.year
            return pad(count == 2 ? (y % 100 + 100) % 100 : y, count)
        case "M", "L":
            switch count {
            case 1, 2: return pad(month, count)
            case 3: return names.months_abbr[month - 1]
            case 5: return first(names.months[month - 1])
            default: return names.months[month - 1]
            }
        case "d", "H", "m", "s":
            // DateFormatter handles these identically under a Gregorian, Latin-digit locale.
            let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.calendar = calendar; f.timeZone = timeZone
            f.dateFormat = String(repeating: String(c), count: count)
            let v = c == "d" ? day : c == "H" ? hour : c == "m" ? value(.minute) : value(.second)
            return count <= 2 && civil.year >= 1583 ? f.string(from: date) : pad(v, count)
        case "D": return pad(civil.ordinal, count)
        case "F": return pad((day - 1) / 7 + 1, count)
        case "E": return weekday(count)
        case "e", "c": return count <= 2 ? pad(sundayFirst ? value(.weekday) : (value(.weekday) + 5) % 7 + 1, c == "e" ? count : 1) : weekday(count)
        case "a", "b", "B": return hour < 12 ? names.am : names.pm
        case "h": return pad(hour % 12 == 0 ? 12 : hour % 12, count)
        case "k": return pad(hour == 0 ? 24 : hour, count)
        case "K": return pad(hour % 12, count)
        case "S": return String((pad(nanos, 9) + String(repeating: "0", count: max(0, count - 9))).prefix(count))
        case "A": return pad((hour * 3600 + value(.minute) * 60 + value(.second)) * 1000 + nanos / 1_000_000, count)
        case "Q", "q":
            let q = (month - 1) / 3 + 1
            switch count {
            case 1, 5: return String(q)
            case 2: return pad(q, 2)
            case 3: return "Q\(q)"
            default: return ["1st quarter", "2nd quarter", "3rd quarter", "4th quarter"][q - 1]
            }
        case "w": return pad(civil.iso.week, count)
        case "W":
            let first = (value(.weekday) - 1 - (day - 1) % 7 + 7) % 7
            let monday = (first + 6) % 7
            return pad((day - 1 + monday) / 7 + 1, count)
        case "Z": return count <= 3 ? zone(false, false) : count == 4 ? gmt(true) : timeZone.secondsFromGMT(for: date) == 0 ? "Z" : zone(true, false)
        case "O", "z", "v", "V": return gmt(count >= 4 || (c == "V" && count >= 2))
        case "x", "X":
            if c == "X", timeZone.secondsFromGMT(for: date) == 0 { return "Z" }
            return zone(count != 1 && count != 2 && count != 4, count == 1)
        default: return String(repeating: String(c), count: count)
        }
    }
}
