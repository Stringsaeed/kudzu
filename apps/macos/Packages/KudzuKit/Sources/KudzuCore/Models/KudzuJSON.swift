import Foundation

// MARK: - Encoder / decoder

/// The JSON coders every Kudzu file and payload goes through.
///
/// Integer widths follow Rust: `i64`/`usize` are `Int` (64-bit on every Mac); fixed widths
/// (`u32`, `u64`, `i32`) keep their width (`UInt32`, `UInt64`, `Int32`).
///
/// The Rust/TypeScript types are the contract (see `apps/macos/docs/MIGRATION.md` §1), so the
/// models implement their own `Codable` where Foundation's defaults differ from serde:
///
/// - **UUIDs** are written lowercase (`Foundation` writes uppercase). Models encode them with
///   ``Swift/KeyedEncodingContainer/encodeUUID(_:forKey:)``; decoding accepts either case.
/// - **Dates** default to chrono's serde format (RFC 3339 UTC, `Z`, fraction only when non-zero:
///   3 or 6 digits). Library records (`Snippet`, `UsageRecord`, `BackupInfo`) use
///   `kudzu_core::types::iso8601` instead: whole seconds. Both parse any fraction length, none,
///   or a numeric offset. See ``KudzuDate``. Models code their dates explicitly, so they produce
///   the contract through any `JSONEncoder`; these coders' date strategies cover other `Date`s.
/// - **Optionals** are omitted when serde has `skip_serializing_if = "Option::is_none"` and written
///   as `null` when it has not (``Swift/KeyedEncodingContainer/encodeNullable(_:forKey:)``).
public enum KudzuJSON {
    /// Pretty-printed, sorted keys (as `kudzu_json::encode` writes libraries), unescaped slashes.
    public static func encoder(pretty: Bool = true) -> JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = pretty ? [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes] : [.sortedKeys, .withoutEscapingSlashes]
        encoder.dateEncodingStrategy = .custom { date, encoder in
            var container = encoder.singleValueContainer()
            try container.encode(KudzuDate.format(date))
        }
        return encoder
    }

    /// Accepts every RFC 3339 timestamp chrono accepts.
    public static func decoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let raw = try container.decode(String.self)
            guard let date = KudzuDate.parse(raw) else {
                throw DecodingError.dataCorruptedError(in: container, debugDescription: "invalid ISO-8601 date: \(raw)")
            }
            return date
        }
        return decoder
    }

    /// A decoder whose `Settings` fill missing keys from `defaults` instead of `Settings.default`
    /// (`Store::load_dev` merges a file over the dev variant's defaults).
    public static func decoder(settingsDefaults defaults: Settings) -> JSONDecoder {
        let decoder = decoder()
        decoder.userInfo[.kudzuSettingsDefaults] = defaults
        return decoder
    }

    /// Encodes `value` with ``encoder(pretty:)``.
    public static func encode<T: Encodable>(_ value: T, pretty: Bool = true) throws -> Data {
        try encoder(pretty: pretty).encode(value)
    }

    /// Decodes `type` with ``decoder()``.
    public static func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        try decoder().decode(type, from: data)
    }
}

extension CodingUserInfoKey {
    /// `Settings` used for keys missing from a decoded `Settings` object.
    public static let kudzuSettingsDefaults = CodingUserInfoKey(rawValue: "kudzu.settingsDefaults")!
}

// MARK: - UUID

extension UUID {
    /// Rust's `Uuid` display form: lowercase, hyphenated.
    public var lowercased: String { uuidString.lowercased() }
}

// MARK: - Coding helpers

extension KeyedEncodingContainer {
    /// Writes a UUID lowercase.
    public mutating func encodeUUID(_ value: UUID, forKey key: Key) throws {
        try encode(value.lowercased, forKey: key)
    }

    /// Writes a UUID lowercase, omitting the key when `nil` (serde `skip_serializing_if`).
    public mutating func encodeUUIDIfPresent(_ value: UUID?, forKey key: Key) throws {
        if let value { try encodeUUID(value, forKey: key) }
    }

    /// Writes a UUID lowercase, or `null` when `nil` (a serde `Option` without `skip`).
    public mutating func encodeNullableUUID(_ value: UUID?, forKey key: Key) throws {
        if let value { try encodeUUID(value, forKey: key) } else { try encodeNil(forKey: key) }
    }

    /// Writes UUIDs lowercase.
    public mutating func encodeUUIDs(_ values: [UUID], forKey key: Key) throws {
        try encode(values.map(\.lowercased), forKey: key)
    }

    /// Writes `value`, or `null` when `nil` (a serde `Option` without `skip_serializing_if`).
    public mutating func encodeNullable<T: Encodable>(_ value: T?, forKey key: Key) throws {
        if let value { try encode(value, forKey: key) } else { try encodeNil(forKey: key) }
    }

    /// Writes a library date with whole seconds (`kudzu_core::types::iso8601`).
    public mutating func encodeWholeSeconds(_ value: Date, forKey key: Key) throws {
        try encode(KudzuDate.formatWholeSeconds(value), forKey: key)
    }

    /// Optional variant of ``encodeWholeSeconds(_:forKey:)`` that omits the key when `nil`.
    public mutating func encodeWholeSecondsIfPresent(_ value: Date?, forKey key: Key) throws {
        if let value { try encodeWholeSeconds(value, forKey: key) }
    }

    /// Writes a date in chrono's serde form (``KudzuDate/format(_:)``), whatever the encoder's
    /// date strategy.
    public mutating func encodeDate(_ value: Date, forKey key: Key) throws {
        try encode(KudzuDate.format(value), forKey: key)
    }

    /// Optional variant of ``encodeDate(_:forKey:)`` that omits the key when `nil`.
    public mutating func encodeDateIfPresent(_ value: Date?, forKey key: Key) throws {
        if let value { try encodeDate(value, forKey: key) }
    }

    /// Writes dates in chrono's serde form.
    public mutating func encodeDates(_ values: [Date], forKey key: Key) throws {
        try encode(values.map(KudzuDate.format), forKey: key)
    }
}

extension KeyedDecodingContainer {
    /// Decodes `key`, or returns `fallback` when it is missing or `null` (serde `#[serde(default)]`).
    public func decode<T: Decodable>(_ type: T.Type, forKey key: Key, default fallback: @autoclosure () -> T) throws -> T {
        try decodeIfPresent(type, forKey: key) ?? fallback()
    }

    /// Decodes an RFC 3339 string, whatever the decoder's date strategy.
    public func decodeDate(forKey key: Key) throws -> Date {
        try parseDate(decode(String.self, forKey: key), key)
    }

    /// Decodes an optional RFC 3339 string (`nil` when missing or `null`).
    public func decodeDateIfPresent(forKey key: Key) throws -> Date? {
        try decodeIfPresent(String.self, forKey: key).map { try parseDate($0, key) }
    }

    /// Decodes an array of RFC 3339 strings (empty when missing).
    public func decodeDates(forKey key: Key) throws -> [Date] {
        try decode([String].self, forKey: key, default: []).map { try parseDate($0, key) }
    }

    private func parseDate(_ raw: String, _ key: Key) throws -> Date {
        guard let date = KudzuDate.parse(raw) else {
            throw DecodingError.dataCorruptedError(forKey: key, in: self, debugDescription: "invalid ISO-8601 date: \(raw)")
        }
        return date
    }
}

// MARK: - Dates

/// RFC 3339 formatting and parsing matching chrono.
public enum KudzuDate {
    /// chrono's serde form for `DateTime<Utc>`: `Z`, no fraction when whole, else 3 or 6 digits
    /// (chrono would use 9 for nanoseconds; `Date` holds microseconds at best).
    public static func format(_ date: Date) -> String {
        let (seconds, micros) = split(date)
        let base = formatSeconds(seconds)
        if micros == 0 { return base + "Z" }
        if micros % 1000 == 0 { return base + "." + pad(micros / 1000, 3) + "Z" }
        return base + "." + pad(micros, 6) + "Z"
    }

    /// `kudzu_core::types::iso8601::format`: `yyyy-MM-ddTHH:mm:ssZ`, fraction truncated.
    public static func formatWholeSeconds(_ date: Date) -> String {
        formatSeconds(split(date).seconds) + "Z"
    }

    /// Always three fraction digits (JavaScript's `toISOString`).
    public static func formatMilliseconds(_ date: Date) -> String {
        let (seconds, micros) = split(date)
        return formatSeconds(seconds) + "." + pad(micros / 1000, 3) + "Z"
    }

    /// Parses an RFC 3339 timestamp: `T`/`t`/space separator, any fraction length or none,
    /// `Z`/`z` or `±HH:MM`. Returns `nil` for anything else.
    public static func parse(_ string: String) -> Date? {
        let b = Array(string.utf8)
        var i = 0
        func digits(_ n: Int) -> Int? {
            guard i + n <= b.count else { return nil }
            var v = 0
            for k in i..<(i + n) {
                guard b[k] >= 48, b[k] <= 57 else { return nil }
                v = v * 10 + Int(b[k] - 48)
            }
            i += n
            return v
        }
        func expect(_ c: UInt8) -> Bool {
            guard i < b.count, b[i] == c else { return false }
            i += 1
            return true
        }
        guard let year = digits(4), expect(45), let month = digits(2), expect(45), let day = digits(2) else { return nil }
        guard i < b.count, b[i] == 84 || b[i] == 116 || b[i] == 32 else { return nil }
        i += 1
        guard let hour = digits(2), expect(58), let minute = digits(2), expect(58), let second = digits(2) else { return nil }
        guard (1...12).contains(month), (1...daysIn(month: month, year: year)).contains(day),
              hour < 24, minute < 60, second <= 60 else { return nil }
        // Nanoseconds, like chrono: digits past the ninth are ignored.
        var nanos = 0
        if i < b.count, b[i] == 46 {
            i += 1
            let start = i
            while i < b.count, b[i] >= 48, b[i] <= 57 {
                if i - start < 9 { nanos = nanos * 10 + Int(b[i] - 48) }
                i += 1
            }
            guard i > start else { return nil }
            for _ in min(i - start, 9)..<9 { nanos *= 10 }
        }
        var offset = 0
        guard i < b.count else { return nil }
        if b[i] == 90 || b[i] == 122 {
            i += 1
        } else if b[i] == 43 || b[i] == 45 {
            let sign = b[i] == 45 ? -1 : 1
            i += 1
            guard let oh = digits(2), expect(58), let om = digits(2), oh < 24, om < 60 else { return nil }
            offset = sign * (oh * 3600 + om * 60)
        } else {
            return nil
        }
        guard i == b.count else { return nil }
        let days = daysFromCivil(year: year, month: month, day: day)
        let whole = days * 86_400 + hour * 3600 + minute * 60 + min(second, 59) - offset
        return Date(timeIntervalSince1970: Double(whole) + Double(nanos) / 1_000_000_000)
    }

    // MARK: Private

    private static func split(_ date: Date) -> (seconds: Int64, micros: Int64) {
        let totalMicros = Int64((date.timeIntervalSince1970 * 1_000_000).rounded())
        var seconds = totalMicros / 1_000_000
        var micros = totalMicros % 1_000_000
        if micros < 0 { micros += 1_000_000; seconds -= 1 }
        return (seconds, micros)
    }

    private static func formatSeconds(_ seconds: Int64) -> String {
        var days = seconds / 86_400
        var rem = seconds % 86_400
        if rem < 0 { rem += 86_400; days -= 1 }
        let (y, m, d) = civilFromDays(Int(days))
        let h = Int(rem / 3600), mi = Int(rem % 3600 / 60), s = Int(rem % 60)
        return "\(pad(Int64(y), 4))-\(pad(Int64(m), 2))-\(pad(Int64(d), 2))T\(pad(Int64(h), 2)):\(pad(Int64(mi), 2)):\(pad(Int64(s), 2))"
    }

    private static func pad(_ value: Int64, _ width: Int) -> String {
        let s = String(value)
        return s.count >= width ? s : String(repeating: "0", count: width - s.count) + s
    }

    private static func daysIn(month: Int, year: Int) -> Int {
        switch month {
        case 2: (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 ? 29 : 28
        case 4, 6, 9, 11: 30
        default: 31
        }
    }

    /// Howard Hinnant's `days_from_civil`.
    private static func daysFromCivil(year: Int, month: Int, day: Int) -> Int {
        let y = month <= 2 ? year - 1 : year
        let era = (y >= 0 ? y : y - 399) / 400
        let yoe = y - era * 400
        let mp = (month + 9) % 12
        let doy = (153 * mp + 2) / 5 + day - 1
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        return era * 146_097 + doe - 719_468
    }

    /// Howard Hinnant's `civil_from_days`.
    private static func civilFromDays(_ z0: Int) -> (Int, Int, Int) {
        let z = z0 + 719_468
        let era = (z >= 0 ? z : z - 146_096) / 146_097
        let doe = z - era * 146_097
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100)
        let mp = (5 * doy + 2) / 153
        let d = doy - (153 * mp + 2) / 5 + 1
        let m = mp < 10 ? mp + 3 : mp - 9
        return (yoe + era * 400 + (m <= 2 ? 1 : 0), m, d)
    }
}
