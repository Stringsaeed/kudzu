import AppKit
import CryptoKit
import Foundation
import KudzuUI
import Testing

@Suite("Tokens")
struct TokenTests {
    @Test("Tokens.generated.swift is up to date with tokens.json and the generator")
    func generatedFileIsFresh() throws {
        var hasher = SHA256()
        hasher.update(data: try Data(contentsOf: repoRoot.appendingPathComponent("design/kudzu/project/tokens.json")))
        hasher.update(data: Data([0]))
        hasher.update(data: try Data(contentsOf: repoRoot.appendingPathComponent("scripts/design/generate-tokens.py")))
        let fingerprint = hasher.finalize().map { String(format: "%02x", $0) }.joined()
        #expect(
            fingerprint == KzTokens.sourceFingerprint,
            "Stale tokens: run `python3 scripts/design/generate-tokens.py` from the repo root."
        )
    }

    @Test("Every colour token in tokens.json is generated")
    func everyColourIsGenerated() throws {
        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: repoRoot.appendingPathComponent("design/kudzu/project/tokens.json"))) as! [String: Any]
        let names = ((json["color"] as! [String: Any])["tokens"] as! [[String: Any]]).map { $0["name"] as! String }
        #expect(names == KzColorToken.allCases.map(\.rawValue))
    }

    @Test("Aliases resolve: focus is vine-ink in both themes")
    func aliasesResolve() {
        #expect(KzColorToken.focus.aliasOf == .vineInk)
        #expect(KzColorToken.focus.light == KzColorToken.vineInk.light)
        #expect(KzColorToken.focus.dark == KzColorToken.vineInk.dark)
    }

    @Test("Eight-digit hex keeps its alpha")
    func eightDigitHex() {
        #expect(KzColorToken.glass.light.hex == "#f3f4f0d6")
        #expect(abs(KzColorToken.glass.light.alpha - 0xD6 / 255.0) < 0.0001)
        #expect(KzColorToken.ink.light.hex == "#151a14")
    }

    @Test("Dynamic colours resolve per appearance, including Increase Contrast")
    func dynamicColours() throws {
        func resolved(_ token: KzColorToken, _ name: NSAppearance.Name) throws -> NSColor {
            let appearance = try #require(NSAppearance(named: name))
            var color = NSColor.clear
            appearance.performAsCurrentDrawingAppearance {
                color = token.nsColor.usingColorSpace(.sRGB) ?? .clear
            }
            return color
        }
        let light = try resolved(.ink, .aqua)
        let dark = try resolved(.ink, .darkAqua)
        #expect(abs(light.redComponent - KzColorToken.ink.light.red) < 0.002)
        #expect(abs(dark.redComponent - KzColorToken.ink.dark.red) < 0.002)
    }

    @Test("Increase Contrast substitutions")
    func highContrast() {
        // macOS can't instantiate high-contrast appearances by name, so test the value logic directly.
        #expect(KzColorToken.line.value(dark: false, highContrast: true) == KzColorToken.lineStrong.light)
        #expect(KzColorToken.inkFaint.value(dark: true, highContrast: true) == KzColorToken.inkMuted.dark)
        #expect(KzColorToken.glass.value(dark: true, highContrast: true).alpha == 1)
        #expect(KzColorToken.ink.value(dark: true, highContrast: true) == KzColorToken.ink.dark)
    }

    @Test("Type styles match tokens.json")
    func typeStyles() {
        #expect(KzTextStyle.allCases.count == 12)
        #expect(KzTextStyle.body.spec.size == 13 && KzTextStyle.body.spec.lineHeight == 18)
        #expect(KzTextStyle.stat.spec.tabularFigures)
        #expect(!KzTextStyle.title.spec.tabularFigures)
        #expect(abs(KzTextStyle.stat.spec.tracking - (-0.56)) < 0.0001)
        #expect(KzTextStyle.trigger.spec.family == .mono)
    }

    @Test("Metrics and shadows")
    func metricsAndShadows() {
        #expect(KzSpace.s4 == 16 && KzSpace.all.count == 8)
        #expect(KzRadius.md == 8 && KzRadius.full == 999)
        #expect(KzSize.titlebar == 52 && KzSize.controlSm == 24)
        #expect(KzBlur.glass == 28)
        let glass = KzShadow.glass.lightLayers
        #expect(glass.count == 3)
        #expect(glass[0].inset && glass[0].y == 1 && abs(glass[0].color.alpha - 0.75) < 0.0001)
        #expect(glass[2].spread == -10 && glass[2].blur == 24)
        #expect(KzShadow.window.darkLayers[0].spread == 0.5)
    }
}
