import Foundation
import Testing
@testable import KudzuCore

/// `KudzuDate`: chrono's RFC 3339 output and parsing rules.
@Suite struct ModelsDateTests {
    private let base = 1_794_130_215.0 // 2026-11-08T09:30:15Z

    @Test(arguments: [
        ("2026-11-08T09:30:15Z", 0.0),
        ("2026-11-08T09:30:15.5Z", 0.5),
        ("2026-11-08T09:30:15.123Z", 0.123),
        ("2026-11-08T09:30:15.123456Z", 0.123_456),
        ("2026-11-08T09:30:15.123456789Z", 0.123_456_789),
        ("2026-11-08T09:30:15.1234567891234Z", 0.123_456_789),
        ("2026-11-08t09:30:15z", 0.0),
        ("2026-11-08 09:30:15Z", 0.0),
        ("2026-11-08T11:30:15+02:00", 0.0),
        ("2026-11-08T05:00:15-04:30", 0.0),
        ("2026-11-08T09:30:15.250+00:00", 0.25),
    ])
    func parsesRFC3339(_ input: String, _ fraction: Double) throws {
        let date = try #require(KudzuDate.parse(input))
        #expect(abs(date.timeIntervalSince1970 - (base + fraction)) < 0.000_001)
    }

    @Test(arguments: [
        "", "2026-11-08", "2026-11-08T09:30Z", "2026-11-08T09:30:15", "2026-11-08T09:30:15.Z",
        "2026-13-08T09:30:15Z", "2026-02-30T09:30:15Z", "2026-11-08T24:00:00Z", "2026-11-08T09:30:15+0200",
        "2026-11-08T09:30:15Zjunk", " 2026-11-08T09:30:15Z",
    ])
    func rejectsInvalid(_ input: String) {
        #expect(KudzuDate.parse(input) == nil)
    }

    @Test func formatsLikeChrono() {
        #expect(KudzuDate.format(Date(timeIntervalSince1970: base)) == "2026-11-08T09:30:15Z")
        #expect(KudzuDate.format(Date(timeIntervalSince1970: base + 0.5)) == "2026-11-08T09:30:15.500Z")
        #expect(KudzuDate.format(Date(timeIntervalSince1970: base + 0.123_456)) == "2026-11-08T09:30:15.123456Z")
        #expect(KudzuDate.formatWholeSeconds(Date(timeIntervalSince1970: base + 0.999)) == "2026-11-08T09:30:15Z")
        #expect(KudzuDate.formatMilliseconds(Date(timeIntervalSince1970: base)) == "2026-11-08T09:30:15.000Z")
        #expect(KudzuDate.format(Date(timeIntervalSince1970: 0)) == "1970-01-01T00:00:00Z")
        #expect(KudzuDate.format(Date(timeIntervalSince1970: 951_782_400)) == "2000-02-29T00:00:00Z")
        #expect(KudzuDate.format(Date(timeIntervalSince1970: -1)) == "1969-12-31T23:59:59Z")
    }

    @Test func formatParsesBack() throws {
        for offset in stride(from: -86_400.0 * 400, to: 86_400 * 400, by: 86_400 * 37.3) {
            let date = Date(timeIntervalSince1970: base + offset.rounded() + 0.042)
            let parsed = try #require(KudzuDate.parse(KudzuDate.format(date)))
            #expect(abs(parsed.timeIntervalSince(date)) < 0.000_001)
        }
    }

    @Test func coderStrategiesUseKudzuDate() throws {
        struct Box: Codable, Equatable { var at: Date }
        let box = Box(at: Date(timeIntervalSince1970: base + 0.5))
        let json = String(decoding: try KudzuJSON.encode(box, pretty: false), as: UTF8.self)
        #expect(json == #"{"at":"2026-11-08T09:30:15.500Z"}"#)
        #expect(try KudzuJSON.decode(Box.self, from: Data(#"{"at":"2026-11-08T11:30:15.5+02:00"}"#.utf8)) == box)
        #expect(throws: DecodingError.self) { try KudzuJSON.decode(Box.self, from: Data(#"{"at":"yesterday"}"#.utf8)) }
    }
}
