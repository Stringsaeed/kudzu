// The two CSS springs (`--spring-soft`, `--spring-pop` in globals.css) are `linear()` sample tables
// over the CSS duration. Natively they are `Spring(duration:bounce:)`; this suite fits the bounce
// (search over [0, 0.6] in 0.005 steps) at the pinned perceptual duration and checks the pinned
// constants. The CSS forces its last sample to 1 (the curve is cut at 240 ms with ≤1.5% left to
// settle), so the shape fit uses the interior samples and the whole table is checked within 0.02.

import SwiftUI
import Foundation
import Testing
@testable import KudzuDesign

@Suite struct SpringFitTests {
    static let soft: [Double] = [0, 0.053, 0.175, 0.324, 0.473, 0.609, 0.723, 0.814, 0.883, 0.934, 0.968, 0.991, 1.005, 1.012, 1.015, 1.015, 1]
    static let pop: [Double] = [0, 0.067, 0.225, 0.422, 0.618, 0.789, 0.923, 1.018, 1.075, 1.102, 1.106, 1.096, 1.077, 1.056, 1.035, 1.018, 1.005, 0.996, 0.991, 0.989, 1]

    /// Max deviation between a spring and a table sampled evenly over 240 ms.
    static func deviation(_ spring: Spring, _ table: [Double], interiorOnly: Bool) -> Double {
        let n = table.count - (interiorOnly ? 1 : 0)
        return (0..<n).map { i in
            abs(spring.value(target: 1.0, time: Motion.slow * Double(i) / Double(table.count - 1)) - table[i])
        }.max() ?? .infinity
    }

    static func bestBounce(_ table: [Double], duration: Double) -> (bounce: Double, deviation: Double) {
        var best = (0.0, Double.infinity)
        for k in 0...120 {
            let b = Double(k) * 0.005
            let d = deviation(Spring(duration: duration, bounce: b), table, interiorOnly: true)
            if d < best.1 { best = (b, d) }
        }
        return best
    }

    @Test func softSpringFitsTable() {
        let duration = Motion.slow * Motion.softDurationRatio
        let fit = Self.bestBounce(Self.soft, duration: duration)
        #expect(abs(fit.bounce - Motion.softBounce) < 1e-9, "fit gives \(fit.bounce)")
        #expect(fit.deviation < 0.002)
        #expect(Self.deviation(Motion.softSpring(), Self.soft, interiorOnly: false) < 0.02)
    }

    @Test func popSpringFitsTable() {
        let duration = Motion.slow * Motion.popDurationRatio
        let fit = Self.bestBounce(Self.pop, duration: duration)
        #expect(abs(fit.bounce - Motion.popBounce) < 1e-9, "fit gives \(fit.bounce)")
        #expect(fit.deviation < 0.002)
        #expect(Self.deviation(Motion.popSpring(), Self.pop, interiorOnly: false) < 0.02)
    }

    @Test func springsScaleWithCSSDuration() {
        // bar-grow is 480 ms of --spring-soft: the same curve, twice as long.
        let long = Motion.softSpring(duration: 0.48)
        for i in 0..<(Self.soft.count - 1) {
            let t = 0.48 * Double(i) / Double(Self.soft.count - 1)
            #expect(abs(long.value(target: 1.0, time: t) - Self.soft[i]) < 0.002)
        }
    }

    @Test func overshootMatchesIntent() {
        let peak: (Spring) -> Double = { s in (0...200).map { s.value(target: 1.0, time: 0.4 * Double($0) / 200) }.max() ?? 0 }
        #expect(abs(peak(Motion.softSpring()) - 1.015) < 0.005)
        #expect(abs(peak(Motion.popSpring()) - 1.106) < 0.01)
    }

    @Test func fittedParametersArePinned() {
        #expect(Motion.softBounce == 0.2)
        #expect(Motion.softDurationRatio == 0.262 / 0.240)
        #expect(Motion.popBounce == 0.42)
        #expect(Motion.popDurationRatio == 0.190 / 0.240)
    }

    @Test func springsFollowActualCSSBetweenKnots() throws {
        let cssURL = try #require(
            Bundle.module.url(forResource: "springs", withExtension: "css", subdirectory: "Resources"),
            "Missing KudzuDesignTests resource Resources/springs.css"
        )
        let css = try String(contentsOf: cssURL, encoding: .utf8)
        for (name, table, spring) in [
            ("soft", Self.soft, Motion.softSpring()), ("pop", Self.pop, Motion.popSpring()),
        ] {
            let raw = try #require(WebSourceParityTests.matches("--spring-\(name): linear\\(([^)]+)\\);", css).first?[0])
            let knots = try raw.split(separator: ",").map {
                try #require(Double($0.trimmingCharacters(in: .whitespaces)))
            }
            #expect(knots == table, "CSS reference table changed")
            var worst = 0.0
            for i in 0...10_000 {
                let progress = Double(i) / 10_000
                let position = progress * Double(knots.count - 1)
                let lower = min(Int(position), knots.count - 2)
                let cssValue = knots[lower] + (knots[lower + 1] - knots[lower]) * (position - Double(lower))
                worst = max(worst, abs(spring.value(target: 1.0, time: Motion.slow * progress) - cssValue))
            }
            #expect(worst < 0.02, "\(name): maximum full-curve deviation \(worst)")
            print("Full-curve \(name) deviation: \(worst)")
        }
    }
}

extension SpringFitTests {
    @Test func independentJointDurationAndBounceFit() {
        // Search both axes independently of Motion's ratios. Include all 17 / 21 CSS knots;
        // the forced endpoint is reported separately because native springs settle naturally.
        for (name, table, expectedBounce, expectedDuration) in [
            ("soft", Self.soft, 0.2, 0.262), ("pop", Self.pop, 0.42, 0.190),
        ] {
            var best = (duration: 0.0, bounce: 0.0, error: Double.infinity)
            for ms in 150...350 {
                for b in 0...120 {
                    let bounce = Double(b) * 0.005
                    let duration = Double(ms) / 1000
                    let error = Self.deviation(Spring(duration: duration, bounce: bounce), table, interiorOnly: true)
                    if error < best.error { best = (duration, bounce, error) }
                }
            }
            #expect(best.error < 0.002)
            #expect(abs(best.duration - expectedDuration) < 1e-9)
            #expect(abs(best.bounce - expectedBounce) < 1e-9)
            let spring = Spring(duration: best.duration, bounce: best.bounce)
            print("Joint fit \(name): duration=\(best.duration), bounce=\(best.bounce), maxInterior=\(best.error), maxAll=\(Self.deviation(spring, table, interiorOnly: false))")
        }
    }
}
