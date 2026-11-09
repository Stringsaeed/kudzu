// Colour conversion: every token's OKLCH → sRGB against a reference table computed independently
// (Python, CSS Color 4 sample-code path OKLab → LMS → XYZ D65 → linear sRGB, parsing globals.css
// directly; the script is in the T4 report). Within 1/255 per channel, alpha exact.

import AppKit
import Testing
@testable import KudzuDesign

typealias RGBA = (Double, Double, Double, Double)

@Suite struct ColorConversionTests {
    static let expected: [(name: String, light: RGBA, dark: RGBA)] = [
        ("background", (1.000000, 1.000000, 1.000000, 1.000000), (0.046959, 0.039356, 0.035544, 1.000000)),
        ("foreground", (0.046959, 0.039356, 0.035544, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("card", (1.000000, 1.000000, 1.000000, 1.000000), (0.109512, 0.097988, 0.090307, 1.000000)),
        ("cardForeground", (0.046959, 0.039356, 0.035544, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("popover", (1.000000, 1.000000, 1.000000, 1.000000), (0.109512, 0.097988, 0.090307, 1.000000)),
        ("popoverForeground", (0.046959, 0.039356, 0.035544, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("primary", (0.077908, 0.279135, 0.901774, 1.000000), (0.099781, 0.233770, 0.722922, 1.000000)),
        ("primaryForeground", (0.936940, 0.964035, 0.998872, 1.000000), (0.936940, 0.964035, 0.998872, 1.000000)),
        ("secondary", (0.956385, 0.956385, 0.959343, 1.000000), (0.152897, 0.152887, 0.165770, 1.000000)),
        ("secondaryForeground", (0.093796, 0.093793, 0.105832, 1.000000), (0.980256, 0.980256, 0.980256, 1.000000)),
        ("muted", (0.960863, 0.960863, 0.957902, 1.000000), (0.161624, 0.144195, 0.139838, 1.000000)),
        ("mutedForeground", (0.472714, 0.442002, 0.420037, 1.000000), (0.652131, 0.626223, 0.608938, 1.000000)),
        ("accent", (0.960863, 0.960863, 0.957902, 1.000000), (0.161624, 0.144195, 0.139838, 1.000000)),
        ("accentForeground", (0.109512, 0.097988, 0.090307, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("destructive", (0.906458, 0.000000, 0.042215, 1.000000), (1.000000, 0.391153, 0.403857, 1.000000)),
        ("border", (0.906710, 0.897532, 0.892941, 1.000000), (1.000000, 1.000000, 1.000000, 0.100000)),
        ("input", (0.906710, 0.897532, 0.892941, 1.000000), (1.000000, 1.000000, 1.000000, 0.150000)),
        ("ring", (0.652131, 0.626223, 0.608938, 1.000000), (0.472714, 0.442002, 0.420037, 1.000000)),
        ("chart1", (0.556592, 0.773164, 1.000000, 1.000000), (0.556592, 0.773164, 1.000000, 1.000000)),
        ("chart2", (0.169333, 0.498049, 1.000000, 1.000000), (0.169333, 0.498049, 1.000000, 1.000000)),
        ("chart3", (0.083619, 0.364417, 0.986343, 1.000000), (0.083619, 0.364417, 0.986343, 1.000000)),
        ("chart4", (0.077908, 0.279135, 0.901774, 1.000000), (0.077908, 0.279135, 0.901774, 1.000000)),
        ("chart5", (0.099781, 0.233770, 0.722922, 1.000000), (0.099781, 0.233770, 0.722922, 1.000000)),
        ("sidebar", (0.980533, 0.980534, 0.977561, 1.000000), (0.109512, 0.097988, 0.090307, 1.000000)),
        ("sidebarForeground", (0.046959, 0.039356, 0.035544, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("sidebarPrimary", (0.083619, 0.364417, 0.986343, 1.000000), (0.169333, 0.498049, 1.000000, 1.000000)),
        ("sidebarPrimaryForeground", (0.936940, 0.964035, 0.998872, 1.000000), (0.936940, 0.964035, 0.998872, 1.000000)),
        ("sidebarAccent", (0.960863, 0.960863, 0.957902, 1.000000), (0.161624, 0.144195, 0.139838, 1.000000)),
        ("sidebarAccentForeground", (0.109512, 0.097988, 0.090307, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("sidebarBorder", (0.906710, 0.897532, 0.892941, 1.000000), (1.000000, 1.000000, 1.000000, 0.100000)),
        ("sidebarRing", (0.652131, 0.626223, 0.608938, 1.000000), (0.472714, 0.442002, 0.420037, 1.000000)),
        ("surface", (1.000000, 1.000000, 1.000000, 1.000000), (0.109512, 0.097988, 0.090307, 1.000000)),
        ("surfaceRaised", (1.000000, 1.000000, 1.000000, 1.000000), (0.136334, 0.124419, 0.116504, 1.000000)),
        ("hairline", (0.046959, 0.039356, 0.035544, 0.080000), (1.000000, 1.000000, 1.000000, 0.080000)),
        ("faint", (0.581083, 0.555651, 0.538831, 1.000000), (0.502608, 0.473009, 0.453381, 1.000000)),
        ("iris", (0.077908, 0.279135, 0.901774, 1.000000), (0.169333, 0.498049, 1.000000, 1.000000)),
        ("irisSoft", (0.077908, 0.279135, 0.901774, 0.100000), (0.169333, 0.498049, 1.000000, 0.180000)),
        ("irisForeground", (0.077908, 0.279135, 0.901774, 1.000000), (0.556592, 0.773164, 1.000000, 1.000000)),
        ("irisContrast", (0.936940, 0.964035, 0.998872, 1.000000), (0.936940, 0.964035, 0.998872, 1.000000)),
        ("chipTime", (0.000000, 0.389688, 0.597185, 1.000000), (0.424188, 0.803714, 0.918545, 1.000000)),
        ("chipClipboard", (0.000000, 0.431619, 0.249423, 1.000000), (0.457893, 0.836107, 0.627617, 1.000000)),
        ("chipCursor", (0.077908, 0.279135, 0.901774, 1.000000), (0.556592, 0.773164, 1.000000, 1.000000)),
        ("chipForm", (0.612308, 0.328525, 0.000000, 1.000000), (0.959484, 0.753956, 0.462519, 1.000000)),
        ("chipError", (0.744992, 0.131982, 0.175588, 1.000000), (1.000000, 0.503768, 0.483134, 1.000000)),
        ("chipNeutral", (0.046959, 0.039356, 0.035544, 1.000000), (0.980533, 0.980534, 0.977561, 1.000000)),
        ("success", (0.000000, 0.601098, 0.338923, 1.000000), (0.330402, 0.798249, 0.558627, 1.000000)),
        ("warning", (0.822841, 0.520532, 0.000000, 1.000000), (0.934784, 0.693483, 0.331136, 1.000000)),
        ("favorite", (0.907365, 0.548432, 0.031934, 1.000000), (0.975191, 0.713297, 0.311286, 1.000000)),
        ("sidebarVibrant", (0.980533, 0.980534, 0.977561, 0.720000), (0.109512, 0.097988, 0.090307, 0.550000)),
        ("scrollbarThumb", (0.388573, 0.388573, 0.388573, 0.250000), (0.388573, 0.388573, 0.388573, 0.250000)),
    ]

    @Test func everyTokenIsInTheReferenceTable() {
        let names = Set(Self.expected.map(\.name))
        for token in KZToken.allCases {
            #expect(names.contains(Self.swiftName(token)), "missing \(token)")
        }
        #expect(names.count == KZToken.allCases.count)
    }

    @Test(arguments: [false, true])
    func tokensMatchReference(dark: Bool) {
        for token in KZToken.allCases {
            guard let row = Self.expected.first(where: { $0.name == Self.swiftName(token) }) else { continue }
            let want = dark ? row.dark : row.light
            let got = Palette.value(token, dark: dark).srgb
            let tol = 1.0 / 255
            #expect(abs(got.r - want.0) <= tol && abs(got.g - want.1) <= tol && abs(got.b - want.2) <= tol,
                    "\(token) \(dark ? "dark" : "light"): got \(got) want \(want)")
            #expect(abs(got.a - want.3) < 1e-9, "\(token) alpha")
        }
    }

    @Test func dynamicColorsResolveByAppearance() throws {
        let color = NSColor.kz.background
        var light: NSColor?
        var dark: NSColor?
        try #require(NSAppearance(named: .aqua)).performAsCurrentDrawingAppearance { light = color.usingColorSpace(.sRGB) }
        try #require(NSAppearance(named: .darkAqua)).performAsCurrentDrawingAppearance { dark = color.usingColorSpace(.sRGB) }
        let l = try #require(light), d = try #require(dark)
        #expect(abs(l.redComponent - 1) < 1e-6)
        #expect(abs(d.redComponent - 0.046959) < 1.0 / 255)
    }

    @Test func colorMixInOKLCH() {
        // color-mix(in oklch, X 13%, transparent) keeps X's channels at alpha 0.13.
        let chip = Palette.value(.chipTime, dark: false)
        #expect(chip.scaledAlpha(0.13).alpha == 0.13)
        // White's hue is powerless, so mixing takes the other colour's hue.
        let mixed = Palette.value(.surfaceRaised, dark: true).mixed(with: .white, amount: 0.04)
        #expect(abs(mixed.h - 56) < 1e-6)
        #expect(abs(mixed.l - (0.245 * 0.96 + 1 * 0.04)) < 1e-4)
    }

    @Test func roundTripThroughSRGB() {
        // An in-gamut colour (`--faint`); out-of-gamut ones clip, so they do not round-trip.
        let v = OKLCH(0.65, 0.01, 56)
        let back = OKLCH(srgb: v.srgb)
        #expect(abs(back.l - v.l) < 1e-3 && abs(back.c - v.c) < 1e-3 && abs(back.h - v.h) < 0.1)
    }

    static func swiftName(_ token: KZToken) -> String {
        switch token {
        case .sidebarVibrant: return "sidebarVibrant"
        case .scrollbarThumb: return "scrollbarThumb"
        default:
            let parts = token.rawValue.split(separator: "-")
            return String(parts[0]) + parts.dropFirst().map { $0.prefix(1).uppercased() + $0.dropFirst() }.joined()
        }
    }
}
