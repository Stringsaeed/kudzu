// Owns the NSTextView and keeps its storage a presentation of an `EditorState`: plain text is the
// source verbatim, every chip and image is one attachment character carrying its source. User
// edits arrive as storage edits and are turned back into source edits (`FormattingEdit`), so the
// bound string is always the serialized storage, byte for byte.
//
// TextKit 1, chosen deliberately: attachment *cells* give per-chip drawing with a hover state,
// exact glyph rects for `coordsAt` and chip clicks, and headless layout for tests; TextKit 2's
// NSTextView falls back to TextKit 1 on attachment cells anyway.

import AppKit
import KudzuCore
import KudzuDesign

@MainActor
public final class TemplateTextController: NSObject {
    public let scrollView: NSScrollView
    public let textView: KZTextView
    /// The editor's own history (CodeMirror's `history()`): text, formatting and selection.
    public let undoManager: UndoManager

    /// The last reconciled state; what the bindings hold.
    public internal(set) var state: EditorState
    /// `state` plus the user edits not yet reconciled.
    var live: EditorState
    /// The atoms currently in the storage.
    var map: EditorMap
    var pending: [(edit: FormattingEdit, kind: EditorInputKind)] = []
    /// Native edits before NSTextStorage widens their dirty range for attribute fixing/batching.
    var nativeBatch: (before: EditorState, edits: [FormattingEdit], kind: EditorInputKind)?
    /// Nonzero while the controller itself writes to the storage.
    var applying = 0
    var lastUndo: (kind: EditorInputKind, edit: FormattingEdit)?

    // MARK: Callbacks

    public var onChange: ((String, [FormatSpan]?) -> Void)?
    public var onChipClick: ((TagToken, CGRect) -> Void)?
    public var onSlashOpen: ((Int, CGRect) -> Void)?
    public var onSelectionChange: ((Range<Int>) -> Void)?
    public var onEscape: (() -> Void)?
    /// A growing editor's content height may have changed.
    var onHeightChange: (() -> Void)?
    /// Mirrors state into a `TemplateEditorHandle` for SwiftUI.
    var onStateChange: (() -> Void)?
    public var snippetName: (String) -> String? = { _ in nil }

    // MARK: Appearance (`.kz-editor`)

    /// 13 pt at 1.7 line height.
    public static let fontSize: CGFloat = 13
    public let lineHeight: CGFloat = KZTextStyle.editor.lineHeight
    public static let inset = NSSize(width: 14, height: 12)

    var textFont: NSFont {
        state.isFormatted ? NSFont.systemFont(ofSize: Self.fontSize) : KZFonts.monoNSFont(size: Self.fontSize)
    }

    var baseAttributes: [NSAttributedString.Key: Any] {
        [.font: textFont, .foregroundColor: NSColor.kz.foreground]
    }

    public var placeholder: String {
        get { textView.placeholderString }
        set { textView.placeholderString = newValue }
    }

    /// Whether the text view is its window's first responder (the card's `focus-within` ring).
    public internal(set) var isFocused = false

    // MARK: Growth (`min-h-[168px]`)

    /// When set, the editor grows with its content from this height and never scrolls itself, so
    /// the pane around it scrolls instead; `nil` fills the frame it is given and scrolls inside.
    public var growsFrom: CGFloat? {
        didSet {
            guard growsFrom != oldValue else { return }
            scrollView.hasVerticalScroller = growsFrom == nil
            scrollView.verticalScrollElasticity = growsFrom == nil ? .automatic : .none
            (scrollView as? EditorScrollView)?.forwardsScrolling = growsFrom != nil
            // The text view fills the box even when the text is shorter, so every click lands in it.
            textView.minSize.height = growsFrom ?? 0
            contentHeightChanged()
        }
    }

    /// The height the editor needs at `width` to show all of its content without scrolling.
    public func fittingHeight(width: CGFloat) -> CGFloat {
        guard let live = layoutManager, let container = textView.textContainer else { return growsFrom ?? 0 }
        let wanted = max(0, width - Self.inset.width * 2)
        let used: CGFloat
        if abs(container.size.width - wanted) < 0.5 {
            live.ensureLayout(for: container)
            used = bottomExtent(live, container: container)
        } else {
            // Another width (SwiftUI measuring before it resizes the view): lay out a copy, since
            // the view's own container follows the view's width and would snap back mid-layout.
            let storage = NSTextStorage(attributedString: storage)
            let layout = NSLayoutManager()
            layout.delegate = self
            storage.addLayoutManager(layout)
            let scratch = NSTextContainer(containerSize: NSSize(width: wanted, height: CGFloat.greatestFiniteMagnitude))
            scratch.lineFragmentPadding = container.lineFragmentPadding
            layout.addTextContainer(scratch)
            layout.ensureLayout(for: scratch)
            used = bottomExtent(layout, container: scratch)
        }
        return max(growsFrom ?? 0, ceil(used + Self.inset.height * 2))
    }

    private func bottomExtent(_ layout: NSLayoutManager, container: NSTextContainer) -> CGFloat {
        var bottom = layout.usedRect(for: container).maxY
        if layout.extraLineFragmentTextContainer === container {
            let extra = layout.extraLineFragmentRect
            bottom = max(bottom, extra.minY + max(extra.height, lineHeight))
        }
        return bottom
    }

    func focusChanged(_ focused: Bool) {
        guard focused != isFocused else { return }
        isFocused = focused
        onStateChange?()
    }

    /// Asks SwiftUI to measure a growing editor again, then keeps the caret in view.
    private var reportingHeight = false
    func contentHeightChanged() {
        guard let floor = growsFrom, !reportingHeight else { return }
        reportingHeight = true
        defer { reportingHeight = false }
        // A previous inner scroll offset would clip the text, because this scroller no longer moves.
        if scrollView.contentView.bounds.origin != .zero { scrollView.contentView.scroll(to: .zero) }
        let width = scrollView.bounds.width > 1 ? scrollView.bounds.width : textView.bounds.width
        if width > 1 {
            let height = fittingHeight(width: width)
            if abs(scrollView.frame.height - height) > 0.5 {
                var frame = scrollView.frame
                frame.size.height = max(floor, height)
                scrollView.frame = frame
            }
        }
        scrollView.invalidateIntrinsicContentSize()
        onHeightChange?()
        guard isFocused else { return }
        DispatchQueue.main.async { [weak self] in self?.revealCaret() }
    }

    /// Scrolls the pane around a growing editor so the caret stays visible. The inner scroll view
    /// never scrolls — its document is exactly as tall as the text — so the caret is revealed in
    /// the ancestor scroller (the snippet pane).
    func revealCaret() {
        guard growsFrom != nil, isFocused,
              let caret = caretRect(atStorage: textView.selectedRange().upperBound) else { return }
        let expanded = caret.insetBy(dx: -8, dy: -28)
        if let outer = (scrollView as? EditorScrollView)?.nearestAncestorScrollView(), let document = outer.documentView {
            document.scrollToVisible(document.convert(expanded, from: textView))
        } else {
            textView.scrollToVisible(expanded)
        }
    }

    // MARK: Setup

    public init(state initial: EditorState = EditorState(source: "")) {
        let storage = NSTextStorage()
        let layout = NSLayoutManager()
        storage.addLayoutManager(layout)
        let container = NSTextContainer(containerSize: NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = true
        container.lineFragmentPadding = 0
        layout.addTextContainer(container)
        textView = KZTextView(frame: NSRect(x: 0, y: 0, width: 480, height: 168), textContainer: container)
        scrollView = EditorScrollView()
        undoManager = UndoManager()
        undoManager.groupsByEvent = false
        state = initial
        live = initial
        map = EditorMap(source: initial.source, spans: initial.spans)
        super.init()

        layout.delegate = self
        storage.delegate = self
        textView.controller = self
        textView.delegate = self
        configureTextView()
        render(initial, rebuild: true)
    }

    private func configureTextView() {
        let view = textView
        view.isRichText = false
        view.importsGraphics = false
        view.allowsImageEditing = false
        view.allowsUndo = false
        view.isEditable = true
        view.isSelectable = true
        view.usesFontPanel = false
        view.usesRuler = false
        view.usesFindBar = true
        view.drawsBackground = false
        view.backgroundColor = .clear
        view.textContainerInset = Self.inset
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.minSize = NSSize(width: 0, height: 0)
        view.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.insertionPointColor = NSColor.kz.iris
        view.selectedTextAttributes = [.backgroundColor: NSColor.kz.irisSoft]
        view.linkTextAttributes = [:]
        // Template syntax must survive typing: no smart quotes, dashes or replacements.
        view.isAutomaticQuoteSubstitutionEnabled = false
        view.isAutomaticDashSubstitutionEnabled = false
        view.isAutomaticTextReplacementEnabled = false
        view.isAutomaticSpellingCorrectionEnabled = false
        view.isAutomaticLinkDetectionEnabled = false
        view.isAutomaticDataDetectionEnabled = false
        view.isAutomaticTextCompletionEnabled = false
        view.smartInsertDeleteEnabled = false
        view.isContinuousSpellCheckingEnabled = true
        view.placeholderFont = NSFont.systemFont(ofSize: Self.fontSize)
        view.setAccessibilityLabel("Snippet content")

        scrollView.documentView = view
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
    }

    var storage: NSTextStorage { textView.textStorage ?? NSTextStorage() }

    // MARK: Rendering

    /// Shows `next`: rebuilds the storage when the atoms differ, otherwise only restyles it.
    func render(_ next: EditorState, rebuild force: Bool = false) {
        nativeBatch = nil
        let desired = EditorMap(source: next.source, spans: next.spans)
        let formattedChanged = next.isFormatted != state.isFormatted
        state = next
        live = next
        applying += 1
        defer { applying -= 1 }
        if force || formattedChanged || !storageMatches(desired, source: next.source) {
            storage.beginEditing()
            storage.setAttributedString(makeStorage(next, map: desired))
            storage.endEditing()
        } else {
            restyle(next, map: desired)
        }
        map = desired
        textView.typingAttributes = baseAttributes
        let selection = desired.storageRange(next.selection)
        textView.setSelectedRange(NSRange(location: selection.lowerBound, length: selection.count))
        state.selection = desired.sourceRange(selection)
        live.selection = state.selection
        if textView.hoveredAtomIndex != nil { textView.setHovered(nil) }
        contentHeightChanged()
    }

    /// The attributed storage for a state.
    func makeStorage(_ state: EditorState, map: EditorMap) -> NSAttributedString {
        let text = state.source as NSString
        let out = NSMutableAttributedString()
        var at = 0
        for atom in map.atoms {
            if atom.range.lowerBound > at {
                out.append(NSAttributedString(string: text.substring(with: NSRange(location: at, length: atom.range.lowerBound - at))))
            }
            out.append(attachmentString(for: atom, source: text.substring(with: NSRange(location: atom.range.lowerBound, length: atom.length))))
            at = atom.range.upperBound
        }
        if at < text.length { out.append(NSAttributedString(string: text.substring(from: at))) }
        let full = NSRange(location: 0, length: out.length)
        out.addAttributes(baseAttributes, range: full)
        applyStyles(state.spans, map: map, to: out)
        return out
    }

    private func attachmentString(for atom: EditorAtom, source: String) -> NSAttributedString {
        let cell: NSTextAttachmentCell
        var tip: String
        switch atom.kind {
        case .chip(let tag):
            cell = ChipCell(label: Template.chipLabel(tag, snippetName: snippetName), tone: Template.chipTone(tag))
            tip = tag.verbatim ? "Not recognised: \(tag.source)" : tag.source
        case .image(let dataUrl, let width):
            cell = ImageCell(dataUrl: dataUrl, width: width, alt: source)
            tip = source
        }
        if tip.isEmpty { tip = source }
        let attachment = AtomAttachment(source: source, cell: cell)
        attachment.tooltip = tip
        let string = NSMutableAttributedString(attachment: attachment)
        string.addAttribute(.toolTip, value: tip, range: NSRange(location: 0, length: string.length))
        return string
    }

    private func tooltip(for attachment: AtomAttachment) -> String { attachment.tooltip ?? attachment.source }

    /// Re-applies fonts and styles without touching characters or attachments.
    func restyle(_ state: EditorState, map: EditorMap) {
        let full = NSRange(location: 0, length: storage.length)
        storage.beginEditing()
        storage.removeAttribute(.underlineStyle, range: full)
        storage.removeAttribute(.toolTip, range: full)
        storage.addAttributes(baseAttributes, range: full)
        for atom in scan().atoms {
            storage.addAttribute(.toolTip, value: tooltip(for: atom.attachment), range: NSRange(location: atom.index, length: 1))
        }
        applyStyles(state.spans, map: map, to: storage)
        storage.endEditing()
    }

    /// Bold/italic as font traits, underline, links as iris + underline (`.kz-format-*`).
    private func applyStyles(_ spans: [FormatSpan]?, map: EditorMap, to text: NSMutableAttributedString) {
        let fonts = NSFontManager.shared
        for span in spans ?? [] {
            let range = map.storageRange(span.start..<span.end)
            let ns = NSRange(location: range.lowerBound, length: range.count)
            guard ns.length > 0, ns.upperBound <= text.length else { continue }
            switch span.style {
            case .bold, .italic:
                let trait: NSFontTraitMask = span.style == .bold ? .boldFontMask : .italicFontMask
                text.enumerateAttribute(.font, in: ns) { value, run, _ in
                    let font = (value as? NSFont) ?? textFont
                    text.addAttribute(.font, value: fonts.convert(font, toHaveTrait: trait), range: run)
                }
            case .underline:
                text.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: ns)
            case .link(let url):
                text.addAttributes([.underlineStyle: NSUnderlineStyle.single.rawValue, .foregroundColor: NSColor.kz.iris, .toolTip: url], range: ns)
            case .image:
                break
            }
        }
    }

    // MARK: Storage ↔ source

    /// An attachment character in the storage and the source range it stands for.
    struct ScannedAtom {
        var index: Int
        var range: Range<Int>
        var attachment: AtomAttachment
    }

    /// Serializes a storage range back to source text.
    func serialize(_ range: NSRange) -> String {
        let text = storage.string as NSString
        let out = NSMutableString()
        var last = range.location
        storage.enumerateAttribute(.attachment, in: range) { value, run, _ in
            guard let attachment = value as? AtomAttachment else { return }
            for i in run.location..<run.upperBound where text.character(at: i) == 0xFFFC {
                out.append(text.substring(with: NSRange(location: last, length: i - last)))
                out.append(attachment.source)
                last = i + 1
            }
        }
        out.append(text.substring(with: NSRange(location: last, length: range.upperBound - last)))
        return out as String
    }

    /// The whole storage as source text plus its atoms.
    func scan() -> (source: String, atoms: [ScannedAtom]) {
        let text = storage.string as NSString
        var atoms: [ScannedAtom] = []
        var shift = 0
        storage.enumerateAttribute(.attachment, in: NSRange(location: 0, length: storage.length)) { value, run, _ in
            guard let attachment = value as? AtomAttachment else { return }
            for i in run.location..<run.upperBound where text.character(at: i) == 0xFFFC {
                let start = i + shift
                let length = attachment.source.utf16.count
                atoms.append(ScannedAtom(index: i, range: start..<(start + length), attachment: attachment))
                shift += length - 1
            }
        }
        return (serialize(NSRange(location: 0, length: storage.length)), atoms)
    }

    /// Whether the storage already shows exactly the atoms of `desired`.
    func storageMatches(_ desired: EditorMap, source: String) -> Bool {
        let current = scan()
        guard current.source.utf16.elementsEqual(source.utf16), current.atoms.count == desired.atoms.count else { return false }
        for (have, want) in zip(current.atoms, desired.atoms) {
            guard have.range == want.range, let cell = have.attachment.attachmentCell else { return false }
            switch want.kind {
            case .chip(let tag):
                guard let chip = cell as? ChipCell, chip.label == Template.chipLabel(tag, snippetName: snippetName),
                      chip.tone == Template.chipTone(tag) else { return false }
            case .image(let dataUrl, let width):
                guard let image = cell as? ImageCell, image.dataUrl == dataUrl, Int(image.displayWidth) == width else { return false }
            }
        }
        return true
    }

    /// Source text of storage ranges (copy, cut and drag carry the tags, not U+FFFC).
    public func sourceText(ofStorageRanges ranges: [NSRange]) -> String {
        ranges.filter { $0.length > 0 && $0.upperBound <= storage.length }.map(serialize).joined(separator: "\n")
    }

    /// The view's selection in source coordinates, computed from the storage as it is now.
    func sourceSelection() -> Range<Int> {
        let selected = textView.selectedRange()
        let atoms = scan().atoms
        func source(_ index: Int) -> Int {
            index + atoms.filter { $0.index < index }.reduce(0) { $0 + $1.range.count - 1 }
        }
        return source(selected.location)..<source(selected.upperBound)
    }

    /// The state as the view shows it right now (pending edits included, current selection).
    public var currentState: EditorState {
        var current = live
        current.selection = pending.isEmpty ? map.sourceRange(textView.selectedRange().location..<textView.selectedRange().upperBound) : sourceSelection()
        return current
    }

    var isComposing: Bool { textView.hasMarkedText() }
}

/// The editor's scroll view. While the editor grows with its content it has nothing to scroll,
/// so wheel and trackpad scrolling pass to the pane around it. `nextResponder` is the SwiftUI
/// hosting view, which does not scroll; the pane's `NSScrollView` is an ancestor instead.
final class EditorScrollView: NSScrollView {
    var forwardsScrolling = false

    private var measuring = false
    override var intrinsicContentSize: NSSize {
        guard forwardsScrolling, !measuring, let controller = (documentView as? KZTextView)?.controller else {
            return NSSize(width: NSView.noIntrinsicMetric, height: NSView.noIntrinsicMetric)
        }
        measuring = true
        defer { measuring = false }
        let width = bounds.width > 1 ? bounds.width : (controller.textView.bounds.width > 1 ? controller.textView.bounds.width : 480)
        return NSSize(width: NSView.noIntrinsicMetric, height: controller.fittingHeight(width: width))
    }

    /// The nearest scroll view outside this one (the snippet pane), if the editor is on screen.
    func nearestAncestorScrollView() -> NSScrollView? {
        var view = superview
        while let current = view {
            if let scroll = current as? NSScrollView { return scroll }
            view = current.superview
        }
        return nil
    }

    override func scrollWheel(with event: NSEvent) {
        guard forwardsScrolling, let outer = nearestAncestorScrollView() else {
            super.scrollWheel(with: event)
            return
        }
        outer.scrollWheel(with: event)
    }
}
