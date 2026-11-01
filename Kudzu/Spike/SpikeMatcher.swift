import Foundation

@MainActor
final class SpikeMatcher {
    struct Match { let trigger: String; let text: String; let method: Inserter.Method }
    private var buffer = ""

    func reset() { buffer = "" }

    func consume(_ signal: EventTap.Signal) -> Match? {
        switch signal {
        case .reset: reset(); return nil
        case .backspace:
            if !buffer.isEmpty { buffer.removeLast() }
            return nil
        case .character(let text): buffer = String((buffer + text).suffix(32))
        }
        let snippets = [
            Match(trigger: ";kz", text: "Kudzu works! 🌱", method: .type),
            Match(trigger: ";long", text: Self.longText, method: .paste),
            Match(trigger: ";now", text: Date.now.formatted(date: .complete, time: .standard), method: .automatic)
        ]
        guard let match = snippets.first(where: { buffer.hasSuffix($0.trigger) }) else { return nil }
        reset()
        return match
    }

    private static let longText = """
    Kudzu expands a short trigger into a longer message while you keep writing in your favorite app.
    This paragraph tests the paste insertion path, including line breaks, punctuation, and Unicode: café, 日本語, and 🌱.
    Your previous clipboard should return after the target app receives the paste command. Copy something else during that delay to check that Kudzu preserves your newer clipboard instead.
    End of the Kudzu paste test.
    """
}
