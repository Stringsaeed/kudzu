import Foundation
import Testing
@testable import KudzuCore

@Suite("TriggerIndex – performance")
struct TriggerIndexPerformanceTests {
    @Test func matchingStaysFastWithFiveThousandSnippets() throws {
        var snippets: [Snippet] = []
        // Many triggers share their last character (digits) – the worst case for a last-char bucket index.
        for i in 0..<5_000 {
            snippets.append(snip([";s\(i)", "tag\(i)x"], fire: i.isMultiple(of: 2) ? .immediate : .wholeWord))
        }
        let index = makeIndex(snippets)
        #expect(index.triggerCount == 10_000)

        let typed = Array("The quick brown fox jumps over the lazy dog ;s42 and ;s4999 tag12x 0123456789 ".map(String.init))
        var buffer = KeyBuffer()
        var matches = 0
        let rounds = 40
        let clock = ContinuousClock()
        let elapsed = clock.measure {
            for _ in 0..<rounds {
                for piece in typed {
                    let input = KeyInput.character(piece)
                    buffer.apply(input)
                    if index.match(buffer: buffer, lastInput: input, bundleID: "com.apple.Notes") != nil {
                        matches += 1
                        buffer.apply(.reset)
                    }
                }
            }
        }
        let keystrokes = rounds * typed.count
        let perKeystroke = elapsed / keystrokes
        #expect(matches > 0)
        // Loose bound (unoptimised debug build, shared CI machines); real cost is a few microseconds.
        #expect(perKeystroke < .milliseconds(1), "avg \(perKeystroke) per keystroke")
    }

    @Test func buildingLargeIndexIsReasonable() {
        let snippets = (0..<5_000).map { snip([";s\($0)"]) }
        let clock = ContinuousClock()
        let elapsed = clock.measure { _ = makeIndex(snippets) }
        #expect(elapsed < .seconds(5))
    }
}
