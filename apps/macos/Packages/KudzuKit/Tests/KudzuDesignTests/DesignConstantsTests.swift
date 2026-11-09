// Radius, durations, stagger, keyframe values and depth levels.

import AppKit
import Testing
@testable import KudzuDesign

@Suite struct DesignConstantsTests {
    @Test func radiusScale() {
        #expect(Radius.base == 10)
        #expect([Radius.sm, Radius.md, Radius.lg, Radius.xl, Radius.xl2, Radius.xl3, Radius.xl4] == [6, 8, 10, 14, 18, 22, 26])
        #expect(Radius.mdMax10 == 8 && Radius.mdMax12 == 8)
        #expect(Radius.palettePanel == 18 && Radius.formPanel == 16)
    }

    @Test func durations() {
        #expect(Motion.fast == 0.080 && Motion.moderate == 0.160 && Motion.slow == 0.240)
        #expect(Motion.duration(0.24, reduce: true) == 0.001)
        #expect(Motion.duration(0.24, reduce: false) == 0.24)
    }

    @Test func staggerDelays() {
        #expect((0..<8).map { Stagger.delay(for: $0) }.map { ($0 * 1000).rounded() } == [0, 30, 60, 90, 120, 150, 150, 150])
    }

    @Test func keyframeValues() {
        #expect(KeyframeSpec.riseOffset == 4 && KeyframeSpec.popScale == 0.94 && KeyframeSpec.checkScale == 0.4 && KeyframeSpec.tickScale == 1.3)
        #expect(KeyframeSpec.starDuration == 0.42 && KeyframeSpec.starMidpoint == 0.55)
        #expect(KeyframeSpec.starFrom == (0.5, -24) && KeyframeSpec.starMid == (1.25, 6))
        #expect(KeyframeSpec.barDuration == 0.48 && KeyframeSpec.confirmDuration == 0.24 && KeyframeSpec.confirmScale == 0.97)
        #expect(KeyframeSpec.shimmerPeriod == 1.6)
    }

    @Test func depthLevelsExistForBothAppearances() {
        let names = Set(Depth.all.map(\.name))
        for name in ["card", "raised", "raised-hover", "pressed", "well", "knob", "key", "selected", "accent", "soft", "float", "panel", "hairline"] {
            #expect(names.contains(name), "missing depth \(name)")
        }
        for depth in Depth.all {
            for dark in [false, true] {
                let r = depth.recipe(dark: dark)
                #expect(r.drops.count <= 2, "\(depth.name): at most two native shadows")
                #expect(!r.drops.isEmpty || !r.inner.isEmpty || r.hairline != nil, "\(depth.name) \(dark) draws nothing")
            }
        }
        #expect(Depth.shadowXs == .raised && Depth.shadowSm == .soft && Depth.shadowMd == .float && Depth.shadowXl == .panel)
    }

    @Test func buttonSizes() {
        #expect(ButtonSize.default.height == 32 && ButtonSize.sm.height == 28 && ButtonSize.xs.height == 24 && ButtonSize.lg.height == 36)
        #expect(ButtonSize.icon.height == 32 && ButtonSize.icon.isIcon)
        #expect(ButtonSize.sm.fontSize == 12.8 && ButtonSize.default.fontSize == 14)
    }

    @Test func commandScore() {
        #expect(CommandScore.score("New snippet", "") == 1)
        #expect(CommandScore.score("New snippet", "xyz") == 0)
        #expect(CommandScore.score("New snippet", "ns") > 0)
        #expect(CommandScore.score("New snippet", "new") > CommandScore.score("New snippet", "nwe".filter { _ in true }) || CommandScore.score("New snippet", "nwe") == 0)
        #expect(CommandScore.score("Settings", "settings") == 1)
    }

    @Test func wordmarkPathsParse() {
        #expect(WordmarkArt.full.layers.count == 5 && WordmarkArt.small.layers.count == 3)
        #expect(WordmarkArt.full.layers.map(\.opacity) == [0.32, 0.47, 0.65, 0.82, 1])
        for layer in WordmarkArt.full.layers + WordmarkArt.small.layers {
            #expect(!layer.path.isEmpty)
            #expect(layer.path.boundingRect.width > 100)
        }
    }
}
