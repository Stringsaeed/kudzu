import Foundation
import SwiftUI
import Testing
@testable import KudzuDesign

@Suite struct WebSourceParityTests {
    static var repo: URL {
        var url = URL(fileURLWithPath: #filePath).resolvingSymlinksInPath().deletingLastPathComponent()
        for _ in 0..<6 { url.deleteLastPathComponent() }
        return url
    }
    /// Frozen design sources under `apps/macos` (the Tauri copies are not read).
    static func source(_ path: String) throws -> String {
        try String(contentsOf: repo.appendingPathComponent("apps/macos/" + path), encoding: .utf8)
    }
    static func matches(_ pattern: String, _ text: String) throws -> [[String]] {
        let regex = try NSRegularExpression(pattern: pattern)
        let ns = text as NSString
        return regex.matches(in: text, range: NSRange(location: 0, length: ns.length)).map { match in
            (1..<match.numberOfRanges).map { ns.substring(with: match.range(at: $0)) }
        }
    }
    static func properties(_ block: String) throws -> [String: String] {
        Dictionary(try matches("--([a-z0-9-]+): ([^;]+);", block).map { ($0[0], $0[1]) }, uniquingKeysWith: { _, last in last })
    }
    static func color(_ name: String, in values: [String: String]) throws -> OKLCH {
        let raw = try #require(values[name], "Missing CSS token \(name)")
        if raw.hasPrefix("var(--") { return try color(String(raw.dropFirst(6).dropLast()), in: values) }
        let body = String(raw.dropFirst(6).dropLast()).replacingOccurrences(of: "/", with: " ")
        let numbers = try body.split(whereSeparator: \.isWhitespace).map { part in
            try #require(Double(part.replacingOccurrences(of: "%", with: ""))) / (part.hasSuffix("%") ? 100 : 1)
        }
        return OKLCH(numbers[0], numbers[1], numbers[2], alpha: numbers.count > 3 ? numbers[3] : 1)
    }

    @Test func everyTokenStillMatchesActualCSS() throws {
        let css = try Self.source("design/reference/globals.css")
        let lightBlock = try #require(Self.matches("\n:root \\{([\\s\\S]*?)\n\\}", css).first?[0])
        let light = try Self.properties(lightBlock)
        let darkBlock = try #require(Self.matches("\n\\.dark \\{([\\s\\S]*?)\n\\}", css).first?[0])
        let dark = try Self.properties(darkBlock)
        for isDark in [false, true] {
            let values = isDark ? dark : light
            for token in KZToken.allCases where token != .sidebarVibrant && token != .scrollbarThumb {
                #expect(try Palette.value(token, dark: isDark) == Self.color(token.rawValue, in: values), "\(token) dark=\(isDark)")
            }
        }
        #expect(Palette.value(.sidebarVibrant, dark: false) == OKLCH(0.985, 0.001, 106.423, alpha: 0.72))
        #expect(Palette.value(.sidebarVibrant, dark: true) == OKLCH(0.216, 0.006, 56.043, alpha: 0.55))
    }

    @Test func wordmarkGeometryMatchesBothEchoSVGs() throws {
        for (suffix, art) in [("", WordmarkArt.full), ("-small", WordmarkArt.small)] {
            let svg = try Self.source("design/wordmark/kudzu-wordmark-echo\(suffix).svg")
            let paths = try Self.matches("(<path\\b[^>]*>)", svg)
            try #require(paths.count == art.layers.count)
            for (i, path) in paths.enumerated() {
                let data = try #require(Self.matches(" d=\"([^\"]+)\"", path[0]).first?[0])
                let opacity = try Self.matches(" fill-opacity=\"([^\"]+)\"", path[0]).first?[0]
                #expect(WordmarkArt.path(data) == art.layers[i].path)
                #expect(opacity.flatMap(Double.init) ?? 1 == art.layers[i].opacity)
            }
        }
    }
}
