import Foundation

/// Chrono uses a proleptic Gregorian calendar, including year zero. Foundation's Gregorian
/// cutover differs before 1582, so those fields and calendar offsets use integer civil arithmetic.
struct GregorianDate {
    var year: Int
    var month: Int
    var day: Int
    var hour: Int
    var minute: Int
    var second: Int
    var days: Int
    init(_ date: Date, offset: Int) {
        let seconds = Int(floor(date.timeIntervalSince1970)) + offset
        days = Self.floorDiv(seconds,86400)
        let daytime = seconds - days * 86400
        hour = daytime / 3600; minute = daytime / 60 % 60; second = daytime % 60
        let civil = Self.civil(days)
        year = civil.0; month = civil.1; day = civil.2
    }
    static func floorDiv(_ a: Int, _ b: Int) -> Int { let q = a / b; return a % b < 0 ? q - 1 : q }
    static func civil(_ days: Int) -> (Int,Int,Int) {
        let z = days + 719468, era = floorDiv(z,146097), doe = z - era * 146097
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365
        var y = yoe + era * 400
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100), mp = (5 * doy + 2) / 153
        let d = doy - (153 * mp + 2) / 5 + 1, m = mp + (mp < 10 ? 3 : -9)
        y += m <= 2 ? 1 : 0
        return (y,m,d)
    }
    static func days(_ year: Int, _ month: Int, _ day: Int) -> Int {
        let y = year - (month <= 2 ? 1 : 0), era = floorDiv(y,400), yoe = y - era * 400
        let mp = month + (month > 2 ? -3 : 9), doy = (153 * mp + 2) / 5 + day - 1
        return era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
    }
    static func leap(_ year: Int) -> Bool { year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) }
    static func monthDays(_ y: Int, _ m: Int) -> Int { m == 2 ? (leap(y) ? 29 : 28) : [4,6,9,11].contains(m) ? 30 : 31 }
    /// Sunday = 1; Monday = 2.
    var weekday: Int { ((days + 4) % 7 + 7) % 7 + 1 }
    var ordinal: Int { days - Self.days(year,1,1) + 1 }
    static func weeks(_ y: Int) -> Int {
        let jan1 = ((Self.days(y,1,1) + 3) % 7 + 7) % 7 + 1
        return jan1 == 4 || (jan1 == 3 && leap(y)) ? 53 : 52
    }
    var iso: (year: Int, week: Int) {
        let weekdayMonday = (weekday + 5) % 7 + 1
        let week = (ordinal - weekdayMonday + 10) / 7
        if week < 1 { return (year - 1, Self.weeks(year - 1)) }
        if week > Self.weeks(year) { return (year + 1,1) }
        return (year,week)
    }
}

extension DateOffset {
    public static func parse(_ source: String) -> DateOffset? { DateOffset(source) }
    /// Fixed-offset arithmetic, matching Chrono: parts apply in written order, months clamp.
    public func apply(to date: Date, timeZone: TimeZone = .gmt) -> Date? {
        let offset = timeZone.secondsFromGMT(for: date)
        var result = date
        for part in parts {
            let value = GregorianDate(result,offset:offset)
            if part.unit == "m" || part.unit == "y" {
                let months = part.amount.multipliedReportingOverflow(by: part.unit == "y" ? 12 : 1)
                guard !months.overflow, months.partialValue.magnitude <= UInt32.max else { return nil }
                let target = (value.year * 12 + value.month - 1).addingReportingOverflow(months.partialValue)
                guard !target.overflow else { return nil }
                let year = GregorianDate.floorDiv(target.partialValue,12), month = target.partialValue - year * 12 + 1
                guard (-262143...262142).contains(year) else { return nil }
                let day = min(value.day,GregorianDate.monthDays(year,month))
                let shift = GregorianDate.days(year,month,day) - value.days
                result = result.addingTimeInterval(Double(shift) * 86400)
            } else {
                guard let scale = ["d":86400,"w":604800,"h":3600,"min":60][part.unit] else { return nil }
                let seconds = part.amount.multipliedReportingOverflow(by:scale)
                guard !seconds.overflow, seconds.partialValue.magnitude <= UInt64(Int64.max / 1000) else { return nil }
                // No two Chrono dates are farther apart than this; avoid overflowing Foundation's date components.
                guard seconds.partialValue.magnitude <= 17_000_000_000_000 else { return nil }
                result = result.addingTimeInterval(Double(seconds.partialValue))
                guard (-262143...262142).contains(GregorianDate(result,offset:offset).year) else { return nil }
            }
        }
        return result
    }
}
