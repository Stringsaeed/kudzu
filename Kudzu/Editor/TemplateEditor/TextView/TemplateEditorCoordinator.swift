import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// Values the coordinator reads from the SwiftUI side on every update.
struct TemplateEditorConfiguration {
    var source: Binding<String>
    var snippets: [SnippetRecord]
    var currentSnippetID: UUID?
    var onDiagnostics: ([TemplateDiagnostic]) -> Void
}

/// Owns the chip text view: keeps it in sync with the `source` binding and drives menus and popovers.
@MainActor
final class TemplateEditorCoordinator: NSObject {
    var configuration: TemplateEditorConfiguration
    private(set) weak var textView: ChipTextView?
    var factory = ChipFactory()
    /// A private stack, so switching snippets can clear history without touching other fields in the window.
    let undoManager = UndoManager()

    /// The source last loaded from or written to the binding. A binding value equal to this is our own echo.
    private var lastSource: String?
    private var lastDiagnostics: [TemplateDiagnostic]?
    /// The most recent edit announced through `shouldChangeTextIn`, consumed by `textDidChange`.
    var pendingEdit: (range: NSRange, text: String)?
    /// Character index of the `/` that opened the insert menu, while it is open.
    var slashAnchor: Int?

    lazy var slashMenu = SlashMenuController { [weak self] item in self?.insertFromSlash(item) }
    lazy var popovers = ChipPopoverController(coordinator: self)

    init(configuration: TemplateEditorConfiguration) {
        self.configuration = configuration
    }

    var storage: NSTextStorage? { textView?.textStorage }

    /// Connects the text view built by the representable.
    func attach(_ textView: ChipTextView) {
        self.textView = textView
        textView.delegate = self
        textView.clipboard = self
    }

    /// Applies new SwiftUI values; rebuilds content only when the source changed externally.
    func update(_ configuration: TemplateEditorConfiguration) {
        let previousSnippetID = self.configuration.currentSnippetID
        self.configuration = configuration
        let names = Dictionary(configuration.snippets.map { ($0.id, $0.name) }, uniquingKeysWith: { first, _ in first })
        let namesChanged = names != factory.snippetNames
        factory.snippetNames = names
        let source = configuration.source.wrappedValue
        if source != lastSource {
            load(source)
        } else if namesChanged || previousSnippetID != configuration.currentSnippetID {
            relabelChips()
            refreshDecorations()
        }
    }

    /// Replaces the whole content from template source without registering undo or echoing back.
    func load(_ source: String) {
        guard let textView, let storage else { return }
        closeSlashMenu()
        popovers.dismiss()
        let selection = textView.selectedRange()
        storage.setAttributedString(factory.attributedString(for: TemplateSegments.parse(source)))
        textView.setSelectedRange(NSRange(location: min(selection.location, storage.length), length: 0))
        textView.typingAttributes = KzEditorStyle.typingAttributes()
        textView.needsDisplay = true
        lastSource = source
        undoManager.removeAllActions()
        refreshDecorations()
    }

    /// Current content as segments.
    func segments() -> [TemplateSegment] {
        storage.map { ChipFactory.segments(in: $0) } ?? []
    }

    /// Serializes the content and writes it to the binding if it changed.
    func syncSource(_ segments: [TemplateSegment]) {
        let source = TemplateSegments.serialize(segments)
        guard source != lastSource else { return }
        lastSource = source
        configuration.source.wrappedValue = source
    }

    /// Re-derives chip labels, e.g. after snippets were renamed.
    func relabelChips() {
        guard let storage, let layoutManager = textView?.layoutManager else { return }
        ChipFactory.forEachChip(in: storage) { chip, index in
            let label = factory.label(for: chip.tag)
            guard let cell = chip.chipCell, cell.label != label else { return }
            cell.label = label
            layoutManager.invalidateLayout(forCharacterRange: NSRange(location: index, length: 1), actualCharacterRange: nil)
        }
    }

    /// Underlines broken literal tags, tints chips with lint warnings and reports diagnostics.
    func refreshDecorations(_ segments: [TemplateSegment]? = nil) {
        guard let textView, let storage, let layoutManager = textView.layoutManager else { return }
        let segments = segments ?? self.segments()
        let diagnostics = TemplateEditorLint.diagnostics(
            for: segments, knownSnippetIDs: Set(factory.snippetNames.keys), currentSnippetID: configuration.currentSnippetID)
        let length = storage.length
        var squiggles = [ChipSquiggle]()
        var chipSeverities = [Int: ChipSeverity]()
        for diagnostic in diagnostics {
            let severity = ChipSeverity(diagnostic.severity)
            if diagnostic.isChip {
                let index = diagnostic.displayRange.lowerBound
                chipSeverities[index] = max(chipSeverities[index] ?? severity, severity)
                continue
            }
            let range = NSRange(location: diagnostic.displayRange.lowerBound, length: diagnostic.displayRange.count)
            guard NSMaxRange(range) <= length else { continue }
            // Errors in `danger`, warnings in `warning`.
            squiggles.append(ChipSquiggle(range: range, severity: severity))
        }
        (layoutManager as? ChipLayoutManager)?.setSquiggles(squiggles)
        ChipFactory.forEachChip(in: storage) { chip, index in
            guard let cell = chip.chipCell else { return }
            let severity = chipSeverities[index]
            guard cell.lintSeverity != severity else { return }
            cell.lintSeverity = severity
            layoutManager.invalidateDisplay(forCharacterRange: NSRange(location: index, length: 1))
        }
        let reported = diagnostics.map(\.sourceDiagnostic)
        guard reported != lastDiagnostics else { return }
        lastDiagnostics = reported
        // Deferred: this can run inside a SwiftUI update, where the host may not mutate state.
        let callback = configuration.onDiagnostics
        DispatchQueue.main.async { callback(reported) }
    }

    /// Closes floating UI when the view leaves the hierarchy.
    func teardown() {
        closeSlashMenu()
        popovers.dismiss()
    }
}
