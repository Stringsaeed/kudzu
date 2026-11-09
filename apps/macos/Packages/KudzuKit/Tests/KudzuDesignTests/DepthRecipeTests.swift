import Foundation
import Testing
@testable import KudzuDesign

@Suite struct DepthRecipeTests {
    // Reviewed native approximations, not a CSS layer renderer. Entries read radius/y/ink@alpha;
    // drops, inner shadows, and edge (colour/width) are separated by |. See design-v3a.md.
    static let expected: [(String, String, String)] = [
        ("hairline", "||line@0.08/1", "||white@0.08/1"),
        ("card", "1/1/ink@0.05 6/4/ink@0.06||line@0.07/1", "1/1/black@0.25 10/6/black@0.3|0/1/white@0.045|white@0.06/1"),
        ("raised", "1/1/ink@0.08|0/1/white@0.7|line@0.1/1", "1/1/black@0.35 2.5/2/black@0.2|0/1/white@0.08|white@0.07/1"),
        ("raised-hover", "2.5/2/ink@0.12|0/1/white@0.7|line@0.13/1", "1/1/black@0.4 4/4/black@0.3|0/1/white@0.1|white@0.1/1"),
        ("pressed", "|1/1/ink@0.12|line@0.1/1", "|1.5/1/black@0.35|white@0.06/1"),
        ("well", "|1/1/ink@0.06|line@0.09/1", "0/1/white@0.03|1/1/black@0.3|white@0.07/1"),
        ("knob", "1.5/1/ink@0.2||ink@0.12/0.5", "1.5/1/black@0.45||black@0.3/0.5"),
        ("key", "|0/-1/line@0.16|line@0.08/0.5", "|0/1/white@0.06 0/-1/black@0.45|black@0.3/0.5"),
        ("selected", "1/1/ink@0.06||line@0.05/1", "1/1/black@0.2|0/1/white@0.05|"),
        ("accent", "1/1/blueDeep@0.25 5/4/blue@0.3|0/1/white@0.22|blueDeep@0.6/1", "1/1/black@0.4 6/4/blueBright@0.3|0/1/white@0.35|blueBright@0.5/1"),
        ("soft", "1/1/ink@0.07 3/2/ink@0.06|0/1/white@0.7|line@0.06/1", "1/1/black@0.3 5/4/black@0.2|0/1/white@0.07|white@0.05/1"),
        ("float", "3/2/ink@0.06 14/12/ink@0.12||line@0.08/1", "3/2/black@0.2 14/12/black@0.35||white@0.08/1"),
        ("panel", "7/4/ink@0.08 28/24/ink@0.2||line@0.08/1", "8/4/black@0.25 30/24/black@0.45||white@0.08/1"),
    ]

    static func number(_ value: Double) -> String { String(format: "%.6g", value) }
    static func color(_ value: OKLCH) -> String {
        let names: [OKLCH: String] = [
            OKLCH(0.147, 0.01, 50): "ink", OKLCH(0.147, 0.004, 49.25): "line",
            OKLCH(1, 0, 0): "white", OKLCH(0, 0, 0): "black",
            OKLCH(0.424, 0.199, 265.638): "blueDeep", OKLCH(0.488, 0.243, 264.376): "blue",
            OKLCH(0.623, 0.214, 259.815): "blueBright",
        ]
        return (names[value.withAlpha(1)] ?? value.withAlpha(1).description) + "@" + number(value.alpha)
    }
    static func signature(_ r: Depth.Recipe) -> String {
        let drops = r.drops.map { "\(number($0.radius))/\(number($0.y))/\(color($0.color))" }.joined(separator: " ")
        let inner = r.inner.map { "\(number($0.radius))/\(number($0.y))/\(color($0.color))" }.joined(separator: " ")
        let line = r.hairline.map { "\(color($0))/\(number(r.hairlineWidth))" } ?? ""
        return [drops, inner, line].joined(separator: "|")
    }

    @Test func everyReviewedRecipeIsPinned() throws {
        #expect(Depth.all.map(\.name) == Self.expected.map(\.0))
        for (name, light, dark) in Self.expected {
            let depth = try #require(Depth.all.first { $0.name == name })
            #expect(Self.signature(depth.light) == light, "\(name) light")
            #expect(Self.signature(depth.dark) == dark, "\(name) dark")
        }
    }
}
