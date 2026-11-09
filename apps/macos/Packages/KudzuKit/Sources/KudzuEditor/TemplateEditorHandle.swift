// The imperative side of the editor (`TemplateEditorHandle` in template-editor.tsx), plus the
// observable state the formatting toolbar reads. Create one with `@State` and pass it to
// `TemplateEditor`; it attaches to the text view when the editor appears.

import AppKit
import KudzuCore
import Observation

@MainActor
@Observable
public final class TemplateEditorHandle {
    @ObservationIgnored public private(set) weak var controller: TemplateTextController?

    /// The source as the editor shows it.
    public private(set) var source = ""
    /// Formatting spans, `nil` in plain mode.
    public private(set) var spans: [FormatSpan]?
    /// The raw selection (source UTF-16 offsets).
    public private(set) var selection: Range<Int> = 0..<0
    /// Whether the text view has keyboard focus (`focus-within` on the editor card).
    public private(set) var isFocused = false
    /// Height of the formatting toolbar above the text, measured by `TemplateEditor`.
    @ObservationIgnored var toolbarHeight: CGFloat = 0

    public init() {}

    func attach(_ controller: TemplateTextController) {
        self.controller = controller
        sync()
    }

    /// Copies the controller's state; only changed values are written, so SwiftUI updates rarely.
    func sync() {
        guard let state = controller?.state else { return }
        if !source.utf16.elementsEqual(state.source.utf16) { source = state.source }
        if !FormattingHelpers.sameSpans(spans, state.spans) { spans = state.spans }
        if selection != state.selection { selection = state.selection }
        if let focused = controller?.isFocused, focused != isFocused { isFocused = focused }
    }

    var model: EditorState { EditorState(source: source, spans: spans, selection: selection) }

    // MARK: State for toolbars

    public var isFormatted: Bool { spans != nil }
    /// The selection widened over the chips and images it touches.
    public var selectedRange: Range<Int> { model.selectedRange }
    public var hasSelection: Bool { !selectedRange.isEmpty }
    /// Whether `kind` ("bold", "italic", "underline", "link") covers the selection.
    public func isActive(_ kind: String) -> Bool { model.isActive(kind) }
    public var linkAtSelection: String? { model.linkAtSelection }
    public var isComposing: Bool { controller?.isComposing ?? false }

    /// The tag token that starts at `position`, if any (to open a chip's popover after inserting it).
    public func tag(at position: Int) -> TagToken? {
        for token in Template.tokenize(source) {
            if case .tag(let tag) = token, tag.from == position { return tag }
        }
        return nil
    }

    // MARK: Commands

    public func focus() { controller?.focus() }

    /// Publishes an in-progress composition into the bound source. Save and delete call this so
    /// the draft includes what the user sees, not only what has already been committed.
    public func commitComposition() { controller?.commitComposition() }

    /// Inserts text at the selection (replacing it) and returns the inserted range.
    @discardableResult
    public func insert(_ text: String) -> Range<Int> {
        controller?.insert(text) ?? selection.lowerBound..<selection.lowerBound
    }

    /// Replaces `from..<to` with `text`; the caret goes after it.
    @discardableResult
    public func replace(_ from: Int, _ to: Int, _ text: String, focus: Bool = true) -> Range<Int> {
        controller?.replace(from..<to, with: text, focus: focus) ?? from..<from
    }

    /// The caret rect at a source position in screen coordinates.
    public func coordsAt(_ position: Int) -> CGRect? { controller?.coordsAt(position) }

    /// The caret rect at a source position in the editor view's own coordinate space (top-left
    /// origin), for `.popover(attachmentAnchor: .rect(.rect(…)))`.
    public func localRect(at position: Int) -> CGRect? { controller?.localRect(at: position) }

    /// The caret rect at a source position in the whole `TemplateEditor`'s coordinate space
    /// (formatting toolbar included, however many rows it wraps to), for popovers attached to it.
    public func anchorRect(at position: Int) -> CGRect? {
        localRect(at: position)?.offsetBy(dx: 0, dy: toolbarHeight)
    }

    public func select(_ range: Range<Int>) { controller?.select(range) }

    /// Toggles a style on the selection; `nil` removes all formatting there.
    @discardableResult
    public func changeFormat(_ style: KudzuCore.FormatStyle?) -> Bool { controller?.changeFormat(style) ?? false }

    /// Submitting the link popover keeps an unchanged link; unlike changeFormat it never toggles
    /// that link off. The toolbar validates the URL before applying it.
    func applyLink(_ url: String) {
        if linkAtSelection.map({ $0.utf16.elementsEqual(url.utf16) }) != true { changeFormat(.link(url: url)) }
    }

    public func setFormatted(_ formatted: Bool) { controller?.setFormatted(formatted) }

    public func toggleList(ordered: Bool) { controller?.toggleList(ordered: ordered) }

    public func insertImage(dataUrl: String, width: Int, label: String) throws(EditorState.ImageError) {
        try controller?.insertImage(dataUrl: dataUrl, width: width, label: label)
    }

    public var canUndo: Bool { controller?.undoManager.canUndo ?? false }
    public func undo() { controller?.undoManager.undo() }
    public func redo() { controller?.undoManager.redo() }
}
