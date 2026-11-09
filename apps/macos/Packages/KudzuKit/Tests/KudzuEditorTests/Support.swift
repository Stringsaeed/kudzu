import AppKit
import KudzuCore
@testable import KudzuEditor

/// The 24×24 PNG from `scripts/tests/formatting.test.mjs`.
let PNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg=="

/// Records what the editor publishes.
@MainActor
final class Recorder {
    var value: String
    var spans: [FormatSpan]?
    var changes = 0
    var slashes: [(position: Int, rect: CGRect)] = []
    var chips: [(tag: TagToken, rect: CGRect)] = []
    var selections: [Range<Int>] = []

    init(value: String, spans: [FormatSpan]?) {
        self.value = value
        self.spans = spans
    }
}

/// A headless editor (no window) wired to a recorder.
@MainActor
func makeEditor(_ source: String, spans: [FormatSpan]? = nil, selection: Range<Int>? = nil) -> (TemplateTextController, Recorder) {
    let editor = TemplateTextController(state: EditorState(source: source, spans: spans, selection: selection))
    let recorder = Recorder(value: source, spans: spans)
    editor.onChange = { value, spans in
        recorder.value = value
        recorder.spans = spans
        recorder.changes += 1
    }
    editor.onSlashOpen = { recorder.slashes.append(($0, $1)) }
    editor.onChipClick = { recorder.chips.append(($0, $1)) }
    editor.onSelectionChange = { recorder.selections.append($0) }
    editor.scrollView.frame = NSRect(x: 0, y: 0, width: 480, height: 240)
    editor.textView.frame = NSRect(x: 0, y: 0, width: 480, height: 240)
    return (editor, recorder)
}

extension TemplateTextController {
    /// Puts the storage caret/selection at a *source* range (as a click or arrow keys would).
    func setSourceSelection(_ range: Range<Int>) { select(range) }

    var sourceSelectionNow: Range<Int> { currentState.selection }

    /// What the storage serializes to.
    var serialized: String { scan().source }

    var storageString: String { storage.string }

    /// Types text the way the keyboard does, one grapheme per `insertText`.
    func type(_ text: String) {
        for character in text { textView.insertText(String(character), replacementRange: NSRange(location: NSNotFound, length: 0)) }
    }
}

func utf16Equal(_ a: String, _ b: String) -> Bool { a.utf16.elementsEqual(b.utf16) }
