// An offscreen harness for the chip editor: behaviour checks plus PNG renders. There is no app test target, so it is
// compiled on its own against the built packages (see `run.sh`); the whole file is empty in the app targets.
#if EDITOR_HARNESS
import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// A window that always claims to be key, so selected chips draw their focused look without showing anything.
private final class HarnessWindow: NSWindow {
    override var isKeyWindow: Bool { true }
    override var canBecomeKey: Bool { true }
}

private final class BackgroundView: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        KzColorToken.surfaceRaised.nsColor.setFill()
        dirtyRect.fill()
    }
}

private final class SourceBox {
    var value: String
    init(_ value: String) { self.value = value }
}

@MainActor
private final class Rig {
    let box: SourceBox
    let window: NSWindow
    let scrollView: NSScrollView
    let textView: ChipTextView
    let coordinator: TemplateEditorCoordinator

    init(source: String, snippets: [SnippetRecord] = [], currentSnippetID: UUID? = nil, size: CGSize = CGSize(width: 560, height: 220)) {
        box = SourceBox(source)
        let box = self.box
        let configuration = TemplateEditorConfiguration(
            source: Binding(get: { box.value }, set: { box.value = $0 }),
            snippets: snippets, currentSnippetID: currentSnippetID, onDiagnostics: { _ in })
        coordinator = TemplateEditorCoordinator(configuration: configuration)
        let (scrollView, textView) = ChipTextView.makeScrollView()
        self.scrollView = scrollView
        self.textView = textView
        coordinator.attach(textView)
        coordinator.update(configuration)
        window = HarnessWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        let background = BackgroundView(frame: NSRect(origin: .zero, size: size))
        scrollView.frame = background.bounds
        scrollView.autoresizingMask = [.width, .height]
        background.addSubview(scrollView)
        window.contentView = background
        window.makeFirstResponder(textView)
        background.layoutSubtreeIfNeeded()
        if let container = textView.textContainer { textView.layoutManager?.ensureLayout(for: container) }
    }

    var segments: [TemplateSegment] { coordinator.segments() }
    var chipIndexes: [Int] {
        var result = [Int]()
        if let storage = textView.textStorage { ChipFactory.forEachChip(in: storage) { _, index in result.append(index) } }
        return result
    }
    func type(_ text: String) {
        for character in text { textView.insertText(String(character), replacementRange: textView.selectedRange()) }
    }
    func command(_ selector: Selector) { textView.doCommand(by: selector) }

    func render(dark: Bool) -> NSBitmapImageRep? {
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        guard let view = window.contentView else { return nil }
        view.layoutSubtreeIfNeeded()
        textView.needsDisplay = true
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        return rep
    }
}

@main
@MainActor
enum EditorHarness {
    static var failures = 0
    static var passes = 0

    static func check(_ condition: @autoclosure () -> Bool, _ name: String, _ detail: @autoclosure () -> String = "") {
        if condition() { passes += 1; print("  ok   \(name)") } else { failures += 1; print("  FAIL \(name) \(detail())") }
    }

    static let signatureID = UUID(uuidString: "5F0B8E4A-3C1D-4C6B-9E8F-2A7D6B1C0E93")!
    static var snippets: [SnippetRecord] {
        [SnippetRecord(id: signatureID, name: "Signature", triggers: [";sig"], content: "— Ada"),
         SnippetRecord(name: "Address", triggers: [";addr"], content: "1 Main St")]
    }

    static let everyKind = """
        Hi {{field name="name" default="there"}},

        Thanks for reaching out on {{date format="EEEE"}}. I'll follow up by {{date offset="+3d"}} at {{time}}.
        Tone: {{choice name="tone" options="Friendly|Formal|Brief" default="Friendly"}}
        Pasted: {{clipboard trim=true}} and {{snippet id="5F0B8E4A-3C1D-4C6B-9E8F-2A7D6B1C0E93"}}
        {{cursor}}
        """

    static let problems = """
        Missing: {{snippet id="00000000-0000-0000-0000-000000000001"}} and unknown {{unknown tag}} and {{DATE}}
        Two cursors {{cursor}} {{cursor}}, an unnamed {{field}} and a bad offset {{date offset="+3x"}}.
        Broken literal: {{ date format= still typed
        """

    static func main() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        KzFonts.register()
        var out: URL?
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--out"), i + 1 < args.count { out = URL(fileURLWithPath: args[i + 1]) }
        behaviour()
        if let out { renders(into: out) }
        print("\n\(passes) passed, \(failures) failed")
        exit(failures == 0 ? 0 : 1)
    }

    // MARK: Behaviour

    static func behaviour() {
        print("round trip")
        do {
            let rig = Rig(source: everyKind, snippets: snippets)
            let expected = TemplateSegments.serialize(TemplateSegments.parse(everyKind))
            check(TemplateSegments.serialize(rig.segments) == expected, "load then serialize is lossless")
            check(rig.box.value == everyKind, "loading does not echo to the binding")
            check(rig.chipIndexes.count == 8, "every tag is a chip", "got \(rig.chipIndexes.count)")
            let string = (rig.textView.textStorage?.string ?? "") as NSString
            check(rig.chipIndexes.allSatisfy { string.character(at: $0) == 0xFFFC }, "a chip is one attachment character")
        }

        print("line metrics")
        do {
            let rig = Rig(source: everyKind, snippets: snippets)
            if let lm = rig.textView.layoutManager {
                var heights = Set<CGFloat>()
                lm.enumerateLineFragments(forGlyphRange: NSRange(location: 0, length: lm.numberOfGlyphs)) { rect, _, _, _, _ in
                    heights.insert(rect.height)
                }
                check(heights == [22], "every line, with or without chips, is 22pt tall", "\(heights)")
            }
            check(rig.textView.textContainerInset == KzEditorStyle.containerInset, "container inset is the design's 14 × 12")
            check(rig.textView.textContainer?.lineFragmentPadding == 0, "no line fragment padding")
        }

        print("atomicity")
        do {
            let rig = Rig(source: "Hi {{clipboard}} there {{cursor}}", snippets: snippets)
            let first = rig.chipIndexes[0]
            rig.textView.setSelectedRange(NSRange(location: first, length: 0))
            rig.command(#selector(NSResponder.moveRight(_:)))
            check(rig.textView.selectedRange() == NSRange(location: first + 1, length: 0), "→ steps over a chip in one press")
            rig.command(#selector(NSResponder.moveLeftAndModifySelection(_:)))
            check(rig.textView.selectedRange() == NSRange(location: first, length: 1), "⇧← selects exactly the chip")
            check(rig.coordinator.selectedChipIndex == first, "a lone selected chip is reported")
            rig.textView.setSelectedRange(NSRange(location: first + 1, length: 0))
            rig.command(#selector(NSResponder.deleteBackward(_:)))
            check(rig.segments == TemplateSegments.parse("Hi  there {{cursor}}"), "Delete removes the whole chip", "\(rig.segments)")
            check(rig.box.value == "Hi  there {{cursor}}", "the binding follows the deletion", rig.box.value)
        }

        print("undo and redo")
        do {
            let rig = Rig(source: "Hello ", snippets: snippets)
            rig.textView.setSelectedRange(NSRange(location: 6, length: 0))
            rig.coordinator.insert(.clipboard, replacing: rig.textView.selectedRange())
            check(rig.segments.count == 2, "insert adds a chip")
            rig.coordinator.undoManager.undo()
            check(rig.segments == [.text("Hello ")], "undo removes it", "\(rig.segments)")
            rig.coordinator.undoManager.redo()
            check(rig.segments.count == 2, "redo restores it", "\(rig.segments)")
            check(rig.box.value == "Hello {{clipboard}}", "binding round-trips", rig.box.value)

            // A popover session: only the first change registers undo.
            let index = rig.chipIndexes[0]
            rig.coordinator.undoManager.removeAllActions()
            let base = rig.segments
            let trimmed = KudzuCore.Tag(name: "clipboard").setting("trim", flag: true)
            rig.coordinator.updateChip(at: index, to: trimmed, coalescing: false)
            rig.coordinator.updateChip(at: index, to: KudzuCore.Tag(name: "clipboard"), coalescing: true)
            rig.coordinator.updateChip(at: index, to: trimmed, coalescing: true)
            rig.coordinator.undoManager.undo()
            check(rig.segments == base, "a popover session is one undo step", "\(rig.segments) vs \(base)")
            check(!rig.coordinator.undoManager.canUndo, "and no more steps remain")
            rig.coordinator.undoManager.redo()
            check(rig.segments.contains { if case .chip(let t) = $0 { t.boolAttribute("trim") } else { false } }, "redo re-applies the latest edit")
        }

        print("typing a tag")
        do {
            let rig = Rig(source: "", snippets: snippets)
            rig.type("Hi {{clipboard}}")
            check(rig.chipIndexes.count == 1, "typing }} converts a tag to a chip", "\(rig.segments)")
            check(rig.box.value == "Hi {{clipboard}}", "source", rig.box.value)
            rig.type(" ok")
            check(rig.segments.last == .text(" ok"), "text after a chip is plain", "\(rig.segments)")
        }

        print("copy and paste")
        do {
            let text = "A {{date}} B {{field name=\"x\"}} C"
            let rig = Rig(source: text, snippets: snippets)
            let board = NSPasteboard(name: NSPasteboard.Name("sh.saeed.kudzu.harness"))
            let length = rig.textView.textStorage?.length ?? 0
            rig.textView.setSelectedRange(NSRange(location: 0, length: length))
            board.clearContents()
            check(rig.textView.writeSelection(to: board, type: .string), "copy writes the selection")
            check(board.string(forType: .string) == text, "copy is template source", board.string(forType: .string) ?? "nil")
            rig.textView.setSelectedRange(NSRange(location: 0, length: length))
            check(rig.textView.readSelection(from: board, type: .string), "paste reads it back")
            check(TemplateSegments.serialize(rig.segments) == text, "paste restores the source", "\(rig.segments)")
            check(rig.chipIndexes.count == 2, "as chips")
        }

        print("slash menu")
        do {
            let rig = Rig(source: "", snippets: snippets)
            rig.type("/")
            check(rig.coordinator.slashAnchor == 0, "/ at line start opens the menu")
            check(rig.coordinator.slashMenu.selectedItem == .date, "the first item is highlighted")
            rig.command(#selector(NSResponder.moveDown(_:)))
            check(rig.coordinator.slashMenu.selectedItem == .time, "↓ moves the highlight")
            rig.command(#selector(NSResponder.moveUp(_:)))
            rig.command(#selector(NSResponder.moveUp(_:)))
            check(rig.coordinator.slashMenu.selectedItem == .snippet, "↑ wraps")
            rig.type("cl")
            check(rig.coordinator.slashMenu.selectedItem == .clipboard, "typing filters")
            check(rig.textView.window?.firstResponder === rig.textView, "the text view stays first responder")
            rig.command(#selector(NSResponder.insertNewline(_:)))
            check(rig.segments == TemplateSegments.parse("{{clipboard}}"), "Return inserts the chip in place of /cl", "\(rig.segments)")
            check(rig.coordinator.slashAnchor == nil, "and closes the menu")

            let escape = Rig(source: "", snippets: snippets)
            escape.type("/da")
            escape.command(#selector(NSResponder.cancelOperation(_:)))
            check(escape.coordinator.slashAnchor == nil && escape.segments == [.text("/da")], "Esc closes and keeps the text")

            let rows = (0..<7).map { SlashMenuMetrics.row(atY: 4 + 24 + CGFloat($0) * 28 + 14, itemCount: 7) }
            check(rows == Array(0..<7), "pointer rows follow the menu layout")
            check(SlashMenuMetrics.row(atY: 10, itemCount: 7) == nil, "the title is not a row")
            check(SlashMenuMetrics.row(atY: 4 + 24 + 7 * 28 + 2, itemCount: 7) == nil, "nor is the bottom padding")
        }

        print("diagnostics")
        do {
            let rig = Rig(source: problems, snippets: snippets)
            let squiggles = (rig.textView.layoutManager as? ChipLayoutManager)?.squiggles ?? []
            check(!squiggles.isEmpty, "broken literal text gets a squiggle", "\(squiggles)")
            var severities = [ChipSeverity?]()
            if let storage = rig.textView.textStorage {
                ChipFactory.forEachChip(in: storage) { chip, _ in severities.append(chip.chipCell?.severity) }
            }
            check(severities.contains(.error), "an unrecognized tag is an error chip")
            check(severities.contains(.warning), "a missing snippet or empty name is a warning chip")
        }

        print("layout")
        do {
            let rig = Rig(source: "Short", snippets: snippets, size: CGSize(width: 560, height: 220))
            check(rig.textView.frame.height >= rig.scrollView.contentSize.height - 0.5,
                  "the text view fills the visible area, so a click below the last line lands in it",
                  "\(rig.textView.frame.height) vs \(rig.scrollView.contentSize.height)")
        }

        print("panels")
        do {
            let rig = Rig(source: "Hi {{field name=\"x\"}} there", snippets: snippets)
            let index = rig.chipIndexes[0]
            rig.textView.setSelectedRange(NSRange(location: index, length: 1))
            rig.command(#selector(NSResponder.insertNewline(_:)))
            check(rig.coordinator.popovers.isOpen, "Return on a selected chip opens its popover")
            rig.coordinator.popovers.close()
            check(!rig.coordinator.popovers.isOpen, "closing it clears the session")
            check(rig.textView.selectedRange() == NSRange(location: index, length: 1), "and the chip stays selected")
            check(rig.textView.window?.firstResponder === rig.textView, "with focus back in the editor")
            rig.coordinator.popovers.show(chipAt: index)
            rig.coordinator.popovers.dismiss()
            check(!rig.coordinator.popovers.isOpen, "dismiss closes without a callback")

            let slash = Rig(source: "", snippets: snippets)
            slash.type("/")
            check(slash.coordinator.slashMenu.isVisible, "the slash menu panel is shown")
            slash.type("zzz")
            check(!slash.coordinator.slashMenu.isVisible, "and hides while nothing matches")
            slash.textView.deleteBackward(nil)
            slash.textView.deleteBackward(nil)
            slash.textView.deleteBackward(nil)
            check(slash.coordinator.slashMenu.isVisible, "and returns when the query matches again")
        }

        print("hover")
        do {
            let rig = Rig(source: "Hi {{clipboard}}", snippets: snippets)
            let index = rig.chipIndexes[0]
            let rect = rig.coordinator.rect(forChipAt: index)
            check(rig.textView.chipIndex(at: NSPoint(x: rect.midX, y: rect.midY)) == index, "a point on the chip finds it")
            check(rig.textView.chipIndex(at: NSPoint(x: rect.midX, y: rect.maxY + 40)) == nil, "a point below does not")
        }
    }

    // MARK: Renders

    static func write(_ rep: NSBitmapImageRep?, _ name: String, dark: Bool, into out: URL) {
        guard let data = rep?.representation(using: .png, properties: [:]) else { print("  could not render \(name)"); failures += 1; return }
        let url = out.appendingPathComponent("\(name)-\(dark ? "dark" : "light")@2x.png")
        do { try data.write(to: url) } catch { print("  could not write \(url.path): \(error)"); failures += 1 }
    }

    static func renders(into out: URL) {
        try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
        print("\nrendering to \(out.path)")
        let size = CGSize(width: 560, height: 230)
        for dark in [false, true] {
            // Every chip kind, resting.
            let kinds = Rig(source: everyKind, snippets: snippets, size: size)
            kinds.textView.setSelectedRange(NSRange(location: 0, length: 0))
            write(kinds.render(dark: dark), "editor-kinds", dark: dark, into: out)

            // A selected chip (keyboard and mouse selection are the same state), and a hovered one.
            let selected = Rig(source: everyKind, snippets: snippets, size: size)
            let chips = selected.chipIndexes
            selected.textView.setSelectedRange(NSRange(location: chips[1], length: 1))
            selected.textView.setHoveredChip(chips[3])
            write(selected.render(dark: dark), "editor-selected-hover", dark: dark, into: out)

            // Errors and squiggles, then a selected error chip.
            let errors = Rig(source: problems, snippets: snippets, size: size)
            write(errors.render(dark: dark), "editor-errors", dark: dark, into: out)
            errors.textView.setSelectedRange(NSRange(location: errors.chipIndexes[1], length: 1))
            write(errors.render(dark: dark), "editor-errors-selected", dark: dark, into: out)

            // A text selection across words and chips, and the empty placeholder.
            let range = Rig(source: everyKind, snippets: snippets, size: size)
            range.textView.setSelectedRange(NSRange(location: 3, length: 40))
            write(range.render(dark: dark), "editor-selection", dark: dark, into: out)
            let empty = Rig(source: "", snippets: snippets, size: CGSize(width: 560, height: 90))
            write(empty.render(dark: dark), "editor-empty", dark: dark, into: out)

            // A selected chip while the editor is not focused.
            let blurred = Rig(source: everyKind, snippets: snippets, size: size)
            blurred.textView.setSelectedRange(NSRange(location: blurred.chipIndexes[1], length: 1))
            blurred.window.makeFirstResponder(nil)
            write(blurred.render(dark: dark), "editor-selected-unfocused", dark: dark, into: out)
        }
        surfaces(into: out)
    }

    /// The slash menu and the inspectors, as static specimens (glass draws its tint offscreen).
    static func surfaces(into out: URL) {
        let model = SlashMenuModel()
        model.items = TemplateInsertItem.allCases
        model.selection = .time
        snapshot("slash-menu", SlashMenuView(model: model), into: out)
        let filtered = SlashMenuModel()
        filtered.items = TemplateInsertItem.matching("c")
        filtered.query = "c"
        filtered.selection = filtered.items.first
        snapshot("slash-menu-filtered", SlashMenuView(model: filtered), into: out)

        let context = ChipInspectorContext(snippets: snippets, currentSnippetID: nil, nameConflicts: { name, _ in name == "taken" })
        func inspector(_ source: String, _ name: String) {
            guard let tag = TemplateSegments.parse(source).compactMap({ segment -> KudzuCore.Tag? in
                if case .chip(let tag) = segment { tag } else { nil }
            }).first else {
                print("  no chip in \(source)")
                failures += 1
                return
            }
            let view = KzPopoverSurface { ChipInspector(tag: tag, context: context, onChange: { _ in }) }
                .padding(KzSpace.s4)
            for dark in [false, true] {
                write(hosted(view, width: 360, dark: dark), "inspector-\(name)", dark: dark, into: out)
            }
        }
        inspector("{{date offset=\"+3d\"}}", "date")
        inspector("{{date format=\"EEE d MMM\" offset=\"+3x\" locale=\"xx\"}}", "date-custom-invalid")
        inspector("{{time}}", "time")
        inspector("{{clipboard trim=true}}", "clipboard")
        inspector("{{cursor}}", "cursor")
        // The same surface through ImageRenderer, to compare the toggle with KudzuUI's own snapshots.
        snapshot("inspector-clipboard-ir", KzPopoverSurface { ClipboardChipInspector(tag: KudzuCore.Tag(name: "clipboard"), onChange: { _ in }).frame(width: ChipInspector.width) }.padding(KzSpace.s4), into: out)
        inspector("{{field name=\"email\" default=\"me@example.com\"}}", "field")
        inspector("{{field name=\"taken\"}}", "field-conflict")
        inspector("{{choice name=\"tone\" options=\"Friendly|Formal|Brief\" default=\"Formal\"}}", "choice")
        inspector("{{snippet id=\"5F0B8E4A-3C1D-4C6B-9E8F-2A7D6B1C0E93\"}}", "snippet")
        inspector("{{unknown tag}}", "unrecognized")
    }

    private static func snapshot<V: View>(_ name: String, _ view: V, into out: URL) {
        do { _ = try KzSnapshot.write(view, named: name, to: out, width: 340, background: .surface) }
        catch { print("  could not render \(name): \(error)"); failures += 1 }
    }

    /// A SwiftUI view drawn through a real hosting view, so AppKit-backed controls (text fields, lists) appear.
    static func hosted<V: View>(_ view: V, width: CGFloat, dark: Bool) -> NSBitmapImageRep? {
        let root = view
            .environment(\.kzSnapshotRendering, true)
            .environment(\.colorScheme, dark ? .dark : .light)
            .frame(width: width)
        let host = NSHostingView(rootView: root)
        host.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        let size = host.fittingSize
        let window = HarnessWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        let background = BackgroundView(frame: NSRect(origin: .zero, size: size))
        host.frame = background.bounds
        background.addSubview(host)
        window.contentView = background
        window.appearance = host.appearance
        background.layoutSubtreeIfNeeded()
        // Let SwiftUI and the list settle.
        RunLoop.current.run(until: Date().addingTimeInterval(0.4))
        background.layoutSubtreeIfNeeded()
        guard let rep = background.bitmapImageRepForCachingDisplay(in: background.bounds) else { return nil }
        background.cacheDisplay(in: background.bounds, to: rep)
        return rep
    }
}
#endif
