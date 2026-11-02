import CoreGraphics
import Foundation
@testable import KudzuUI
import Testing

@Suite("Icons")
struct IconTests {
    @Test("SVG numbers in compact syntax")
    func compactNumbers() throws {
        let path = try KzSVGPath.parse("M8 1.9l1.85 3.76 4.15.6-3 2.93.71 4.13z")
        let box = path.boundingBoxOfPath
        #expect(box.minX >= 7.9 && box.maxX <= 14.1)
        #expect(path.currentPoint == CGPoint(x: 8, y: 1.9))
    }

    @Test("Relative commands, implicit linetos, H and V")
    func relative() throws {
        let path = try KzSVGPath.parse("m2 2 3 0v4h-3z")
        #expect(path.boundingBoxOfPath == CGRect(x: 2, y: 2, width: 3, height: 4))
    }

    @Test("Arcs match a circle")
    func arcs() throws {
        let path = try KzSVGPath.parse("M2 8a6 6 0 1 0 12 0a6 6 0 1 0-12 0")
        let box = path.boundingBoxOfPath
        #expect(abs(box.minX - 2) < 0.01 && abs(box.maxX - 14) < 0.01)
        #expect(abs(box.minY - 2) < 0.01 && abs(box.maxY - 14) < 0.01)
    }

    @Test("Bad data throws")
    func badData() {
        #expect(throws: KzSVGPath.ParseError.self) { try KzSVGPath.parse("8 8") }
        #expect(throws: KzSVGPath.ParseError.self) { try KzSVGPath.parse("M8") }
    }

    @Test("Z takes no coordinates: a number after it throws instead of looping forever")
    func closeTakesNoCoordinates() {
        // Run on a separate thread so a regression fails the test instead of hanging the run.
        final class Outcome: @unchecked Sendable { var threw: [Bool] = [] }
        let outcome = Outcome()
        let finished = DispatchSemaphore(value: 0)
        Thread {
            for d in ["M0 0Z10 10", "M0 0 z 1", "M0 0z,5"] {
                outcome.threw.append((try? KzSVGPath.parse(d)) == nil)
            }
            finished.signal()
        }.start()
        guard finished.wait(timeout: .now() + 5) == .success else {
            Issue.record("KzSVGPath.parse did not return")
            return
        }
        #expect(outcome.threw == [true, true, true])
        // A command after Z is still fine.
        #expect(throws: Never.self) { try KzSVGPath.parse("M0 0h4v4zm6 0h2v2Z M1 1 L2 2z") }
    }

    @Test("Every glyph parses and stays on the 16 grid", arguments: KzIconName.allCases)
    func glyphParses(_ name: KzIconName) throws {
        for part in name.parts {
            switch part {
            case .stroke(let d), .fill(let d):
                let path = try KzSVGPath.parse(d)
                let box = path.boundingBoxOfPath
                #expect(box.minX >= 0.5 && box.minY >= 0.5 && box.maxX <= 15.5 && box.maxY <= 15.5, "\(name) leaves the grid: \(box)")
            case .circle(let x, let y, let r):
                #expect(x - r >= 0.5 && x + r <= 15.5 && y - r >= 0.5 && y + r <= 15.5)
            }
        }
    }

    @Test("The design system's 33 glyphs are ported verbatim from bundle.js")
    func portedVerbatim() throws {
        let js = try String(contentsOf: repoRoot.appendingPathComponent("design/kudzu/project/components/bundle.js"), encoding: .utf8)
        let start = try #require(js.range(of: "var ICONS = {"))
        let end = try #require(js[start.upperBound...].range(of: "\n  };"))
        let body = js[start.upperBound..<end.lowerBound]
        let entry = try Regex(#"^\s*'?([a-z-]+)'?:\s*\[(.*)\],?\s*$"#).anchorsMatchLineEndings()
        var source: [String: [String]] = [:]
        for line in body.split(separator: "\n") {
            guard let match = try entry.firstMatch(in: String(line)) else { continue }
            let name = String(match.output[1].substring!)
            let parts = String(match.output[2].substring!)
                .matches(of: /'([^']*)'/).map { String($0.output.1) }
            source[name] = parts
        }
        #expect(source.count == 33)
        let ported = KzIconName.allCases.filter { !$0.isAddition }
        #expect(Set(ported.map(\.rawValue)) == Set(source.keys))
        for name in ported {
            let ours = name.parts.map { part -> String in
                switch part {
                case .stroke(let d): d
                case .fill(let d): "F:" + d
                case .circle(let x, let y, let r): "c:\(fmt(x)):\(fmt(y)):\(fmt(r))"
                }
            }
            #expect(ours == source[name.rawValue], "\(name.rawValue) differs from bundle.js")
        }
    }

    @Test("Addition SVGs in design/kudzu/additions/icons match the Swift glyphs")
    func additionSVGsMatch() throws {
        for name in KzIconName.allCases where name.isAddition {
            let url = repoRoot.appendingPathComponent("design/kudzu/additions/icons/\(name.rawValue).svg")
            let svg = try String(contentsOf: url, encoding: .utf8)
            for part in name.parts {
                switch part {
                case .stroke(let d), .fill(let d):
                    #expect(svg.contains("d=\"\(d)\""), "\(name.rawValue).svg is missing \(d)")
                case .circle(let x, let y, let r):
                    #expect(svg.contains("cx=\"\(fmt(x))\" cy=\"\(fmt(y))\" r=\"\(fmt(r))\""))
                }
            }
        }
    }

    @Test("Menu bar glyphs load as 18pt templates")
    func menuBar() throws {
        for icon in KzMenuBarIcon.allCases {
            let image = icon.image()
            #expect(image.isTemplate)
            #expect(image.size == CGSize(width: 18, height: 18))
            let document = try #require(KzSVGDocument.load(icon.resourceName), "\(icon) SVG missing from bundle")
            #expect(!document.elements.isEmpty)
        }
    }

    @Test("Bundled menu bar SVGs are identical to the design sources")
    func menuBarSourcesMatch() throws {
        let sources = [
            "kudzu-menubar": "design/kudzu/project/assets/Logos/kudzu-menubar.svg",
            "kudzu-menubar-paused": "design/kudzu/project/assets/Logos/kudzu-menubar-paused.svg",
            "kudzu-menubar-permissions": "design/kudzu/additions/kudzu-menubar-permissions.svg",
            "kudzu-menubar-secure-input": "design/kudzu/additions/kudzu-menubar-secure-input.svg",
        ]
        for (name, path) in sources {
            let bundled = try #require(KzResources.bundle.url(forResource: name, withExtension: "svg", subdirectory: "MenuBar"))
            #expect(try Data(contentsOf: bundled) == Data(contentsOf: repoRoot.appendingPathComponent(path)), "\(name).svg drifted from \(path)")
        }
    }
}

private func fmt(_ v: Double) -> String {
    v.rounded() == v ? String(Int(v)) : String(v)
}
