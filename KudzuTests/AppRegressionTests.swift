import AppKit
import Carbon
import KudzuCore
import KudzuUI
import SwiftUI
import Testing

/// Offscreen key-capable window used to verify AppKit/SwiftUI geometry and responder behavior
/// without displaying a window or stealing user focus.
private final class OffscreenTestWindow: NSWindow {
    override var isKeyWindow: Bool { true }
    override var canBecomeKey: Bool { true }
}

@MainActor
@Suite struct AppRegressionTests {
    @Test func practiceExpansionCannotRunInsideTheLibraryOrAnotherApp() {
        #expect(ExpansionEngine.isPracticeTarget(fieldFocused: true, windowID: SceneID.onboarding, appIsFrontmost: true))
        #expect(!ExpansionEngine.isPracticeTarget(fieldFocused: true, windowID: SceneID.main, appIsFrontmost: true))
        #expect(!ExpansionEngine.isPracticeTarget(fieldFocused: false, windowID: SceneID.onboarding, appIsFrontmost: true))
        #expect(!ExpansionEngine.isPracticeTarget(fieldFocused: true, windowID: SceneID.onboarding, appIsFrontmost: false))
        #expect(!ExpansionEngine.isPracticeTarget(fieldFocused: true, windowID: nil, appIsFrontmost: true))
    }

    init() {
        NSApplication.shared.setActivationPolicy(.prohibited)
        KzFonts.register()
    }

    // MARK: - 1. Search Palette Layout, Geometry, Focus Retention, Escape, IME, & Disabled Empty Actions

    @Test func searchPalettePopulatedToNoMatchToClearAndEmptyLibraryGeometryAndFocus() throws {
        let store = try KudzuStore.inMemory()
        try store.upsert(GroupRecord(id: DebugSeed.id(1), name: "Work", sortIndex: 0))
        let sigID = DebugSeed.id(10)
        try store.upsert(SnippetRecord(
            id: sigID,
            name: "Signature",
            triggers: [";sig"],
            content: "Best,\nAda",
            groupID: DebugSeed.id(1)
        ))
        try store.upsert(SnippetRecord(
            id: DebugSeed.id(11),
            name: "Date Stamp",
            triggers: [";date"],
            content: "{{date}}",
            groupID: DebugSeed.id(1)
        ))

        let controller = SearchPaletteController(store: store)
        let model = controller.model
        model.prepareForShow()

        var insertedIDs: [UUID] = []
        var editedIDs: [UUID] = []
        controller.onInsert = { id, _ in insertedIDs.append(id) }
        controller.onEdit = { id in editedIDs.append(id) }

        let paletteView = SearchPaletteView(model: model, controller: controller)

        let size = SearchPaletteController.panelSize
        #expect(size == CGSize(width: 760, height: 460))

        let hosting = NSHostingView(rootView: paletteView)
        hosting.frame = NSRect(origin: .zero, size: size)
        let window = OffscreenTestWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.contentView = hosting
        hosting.layoutSubtreeIfNeeded()

        func findSearchField(in root: NSView) -> NSTextField? {
            if let field = root as? NSTextField, field.isEditable { return field }
            for child in root.subviews {
                if let found = findSearchField(in: child) { return found }
            }
            return nil
        }

        let searchField = try #require(findSearchField(in: hosting))
        _ = window.makeFirstResponder(searchField)

        func searchFieldTopY() -> CGFloat {
            let rectInHost = hosting.convert(searchField.bounds, from: searchField)
            // Convert to top-down coordinates regardless of view flipping.
            return hosting.isFlipped ? rectInHost.minY : (size.height - rectInHost.maxY)
        }

        // State A: Populated
        #expect(model.rows.count == 2)
        #expect(model.selectedRow != nil)
        #expect(hosting.frame.size == size)
        let populatedTopY = searchFieldTopY()
        #expect(populatedTopY >= 0 && populatedTopY <= 30, "Search field must be pinned near top in populated state, got \(populatedTopY)")

        // State B: Query with no matches ("dsadas")
        model.query = "dsadas"
        hosting.layoutSubtreeIfNeeded()
        #expect(model.rows.isEmpty)
        #expect(model.selectedRow == nil)
        #expect(hosting.frame.size == size)

        let noMatchTopY = searchFieldTopY()
        #expect(abs(noMatchTopY - populatedTopY) <= 1.0, "Search field must remain pinned at the exact same top Y on no-match ('dsadas'), got \(noMatchTopY) vs \(populatedTopY)")
        #expect(window.firstResponder === searchField.currentEditor(), "Search field must retain focus when transitioning to no-match")

        // Verify HintBar hides Insert / Copy / Edit when hasSelection is false
        let noSelectionHintBar = PaletteHintBar(hasSelection: model.selectedRow != nil)
        #expect(!noSelectionHintBar.hasSelection)

        // State C: Clear query -> returns to populated without losing focus
        model.clearQuery()
        hosting.layoutSubtreeIfNeeded()
        #expect(model.query.isEmpty)
        #expect(model.rows.count == 2)
        #expect(model.selectedRow != nil)
        #expect(abs(searchFieldTopY() - populatedTopY) <= 1.0)
        #expect(window.firstResponder === searchField.currentEditor(), "Search field must retain focus after clearing query")

        // State D: Truly empty library
        for snippet in try store.library().snippets {
            try store.deleteSnippet(id: snippet.id)
        }
        model.refreshNow()
        hosting.layoutSubtreeIfNeeded()
        #expect(!model.hasSnippets)
        #expect(model.rows.isEmpty)
        #expect(model.selectedRow == nil)
        #expect(abs(searchFieldTopY() - populatedTopY) <= 1.0, "Search field must remain pinned at top when library is empty")
        #expect(insertedIDs.isEmpty)
        #expect(editedIDs.isEmpty)
    }

    @Test func searchPaletteKeyHandlingDisablesActionsOnNoSelectionAndRespectsEscapeAndIME() throws {
        let store = try KudzuStore.inMemory()
        try store.upsert(SnippetRecord(name: "Greeting", triggers: [";hi"], content: "Hello world"))

        let controller = SearchPaletteController(store: store)
        var inserted: [(UUID, pid_t?)] = []
        var edited: [UUID] = []
        controller.onInsert = { id, pid in inserted.append((id, pid)) }
        controller.onEdit = { id in edited.append(id) }

        controller.model.prepareForShow()
        #expect(controller.model.rows.count == 1)

        // Filter to no matches ("dsadas")
        controller.model.query = "dsadas"
        #expect(controller.model.rows.isEmpty)
        #expect(controller.model.selectedRow == nil)

        func makeKeyEvent(keyCode: UInt16, flags: NSEvent.ModifierFlags = [], characters: String = "") -> NSEvent {
            NSEvent.keyEvent(
                with: .keyDown,
                location: .zero,
                modifierFlags: flags,
                timestamp: 0,
                windowNumber: 0,
                context: nil,
                characters: characters,
                charactersIgnoringModifiers: characters,
                isARepeat: false,
                keyCode: keyCode
            )!
        }

        // Return, Cmd+Return, and Cmd+E must be consumed as no-ops when selectedRow == nil
        #expect(controller.handleKeyEvent(makeKeyEvent(keyCode: UInt16(kVK_Return), characters: "\r")))
        #expect(controller.handleKeyEvent(makeKeyEvent(keyCode: UInt16(kVK_Return), flags: [.command], characters: "\r")))
        #expect(controller.handleKeyEvent(makeKeyEvent(keyCode: UInt16(kVK_ANSI_E), flags: [.command], characters: "e")))
        #expect(inserted.isEmpty)
        #expect(edited.isEmpty)

        // Escape in the palette closes the transient palette cleanly
        #expect(controller.handleKeyEvent(makeKeyEvent(keyCode: UInt16(kVK_Escape), characters: "\u{1b}")))
        #expect(!controller.model.isActive)

        // Clearing the query restores selection
        controller.model.clearQuery()
        #expect(controller.model.query.isEmpty)
        #expect(controller.model.rows.count == 1)
        #expect(controller.model.selectedRow != nil)

        // When preflightInsert returns a blocker message, Return surfaces actionError instead of dismissing
        controller.preflightInsert = { _, _ in "Accessibility permission is required to insert snippets." }
        #expect(controller.handleKeyEvent(makeKeyEvent(keyCode: UInt16(kVK_Return), characters: "\r")))
        #expect(controller.model.actionError == "Accessibility permission is required to insert snippets.")
        #expect(inserted.isEmpty)
    }

    // MARK: - 2. Onboarding Honesty & Practice Expansion Verification

    @Test func onboardingPracticeNeverFakesSuccessWhenBlockedAndDistinguishesDemoPreview() {
        // 1. Missing permissions: typing ;kudzu must report .blocked with typedTriggerWithoutExpansion = true
        let missingPerms = ExpansionEngine.evaluateReadiness(
            isPaused: false,
            secureInputActive: false,
            secureInputOwner: nil,
            inputMonitoring: false,
            accessibility: false,
            postEvent: false,
            tapMode: nil
        )
        #expect(!missingPerms.isReady)
        let stateMissingPerms = OnboardingPracticeState.evaluate(
            practiceText: ";kudzu",
            readiness: missingPerms,
            verifiedExpansionCount: 0,
            baselineCount: 0
        )
        if case .blocked(let reason, let typedTrigger) = stateMissingPerms {
            #expect(typedTrigger)
            #expect(reason.contains("Input Monitoring"))
        } else {
            Issue.record("Expected .blocked when permissions are missing, got \(stateMissingPerms)")
        }

        // 2. Paused: typing ;kudzu must report .blocked, never .verified
        let paused = ExpansionEngine.evaluateReadiness(
            isPaused: true,
            secureInputActive: false,
            secureInputOwner: nil,
            inputMonitoring: true,
            accessibility: true,
            postEvent: true,
            tapMode: .active
        )
        let statePaused = OnboardingPracticeState.evaluate(
            practiceText: ";kudzu",
            readiness: paused,
            verifiedExpansionCount: 0,
            baselineCount: 0
        )
        #expect(statePaused == .blocked(reason: "Kudzu expansion is currently paused.", typedTriggerWithoutExpansion: true))

        // 3. Secure Input active: typing ;kudzu must report .blocked, never .verified
        let secure = ExpansionEngine.evaluateReadiness(
            isPaused: false,
            secureInputActive: true,
            secureInputOwner: "1Password",
            inputMonitoring: true,
            accessibility: true,
            postEvent: true,
            tapMode: .active
        )
        let stateSecure = OnboardingPracticeState.evaluate(
            practiceText: ";kudzu",
            readiness: secure,
            verifiedExpansionCount: 0,
            baselineCount: 0
        )
        if case .blocked(let reason, let typedTrigger) = stateSecure {
            #expect(typedTrigger)
            #expect(reason.contains("Secure Input"))
        } else {
            Issue.record("Expected .blocked during Secure Input, got \(stateSecure)")
        }

        // 4. Demo Preview is explicitly labeled and never reports .verified
        let demoState = OnboardingPracticeState.evaluate(
            practiceText: ImportCoordinator.sampleExpansion,
            readiness: missingPerms,
            verifiedExpansionCount: 0,
            baselineCount: 0,
            isDemoPreview: true
        )
        #expect(demoState == .demoPreview(blocker: missingPerms.blockerSummary))

        // 5. Verified requires readiness.isReady AND verifiedExpansionCount > baselineCount
        let ready = ExpansionEngine.evaluateReadiness(
            isPaused: false,
            secureInputActive: false,
            secureInputOwner: nil,
            inputMonitoring: true,
            accessibility: true,
            postEvent: true,
            tapMode: .active
        )
        #expect(ready.isReady)
        let unverifiedManualPaste = OnboardingPracticeState.evaluate(
            practiceText: ImportCoordinator.sampleExpansion,
            readiness: ready,
            verifiedExpansionCount: 2,
            baselineCount: 2
        )
        #expect(unverifiedManualPaste == .awaitingLiveInput)

        let verifiedLive = OnboardingPracticeState.evaluate(
            practiceText: ImportCoordinator.sampleExpansion,
            readiness: ready,
            verifiedExpansionCount: 3,
            baselineCount: 2
        )
        #expect(verifiedLive == .verified)
    }

    // MARK: - 4. FormPanel Dirty-State Escape & Cancel Protection

    @Test func formModelProtectsDirtyValuesOnEscapeAndCancel() {
        let fields = [
            FormFieldSpec(name: "Customer", kind: .text, options: [], defaultValue: "Ada"),
            FormFieldSpec(name: "Notes", kind: .multiline, options: [], defaultValue: "")
        ]
        let model = FormModel(name: "Order Follow-up", fields: fields, values: ["Customer": "Ada", "Notes": ""])
        #expect(!model.isDirty)

        // Clean form closes immediately on Escape or Cancel
        #expect(model.handleEscape() == .closeImmediately)
        #expect(model.requestCancel() == .closeImmediately)

        // Editing a field makes the form dirty
        model.values["Customer"] = "Grace"
        #expect(model.isDirty)

        // Escape on dirty form prompts for discard confirmation instead of closing
        #expect(model.handleEscape() == .promptDiscard)
        #expect(model.isConfirmingDiscard)
        #expect(model.values["Customer"] == "Grace")

        // Pressing Escape while confirmation is shown dismisses the confirmation prompt and keeps editing
        #expect(model.handleEscape() == .dismissedDiscardPrompt)
        #expect(!model.isConfirmingDiscard)
        #expect(model.values["Customer"] == "Grace")

        // Clicking Cancel on dirty form also prompts for discard confirmation
        #expect(model.requestCancel() == .promptDiscard)
        #expect(model.isConfirmingDiscard)
    }

    // MARK: - 5. Template Editor Slash Menu Escaping & Escape Focus Exit

    @Test func templateEditorSlashMenuEscapesLiteralSlashAndExitsFocusCleanly() {
        var source = ""
        let config = TemplateEditorConfiguration(
            source: Binding(get: { source }, set: { source = $0 }),
            snippets: [],
            currentSnippetID: nil,
            onDiagnostics: { _ in }
        )
        let coordinator = TemplateEditorCoordinator(configuration: config)
        let (scrollView, textView) = ChipTextView.makeScrollView()
        coordinator.attach(textView)
        coordinator.update(config)

        let window = OffscreenTestWindow(
            contentRect: NSRect(x: 0, y: 0, width: 480, height: 180),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.contentView = scrollView
        _ = window.makeFirstResponder(textView)

        // Type "/" to open slash menu, then press Escape -> literal "/" stays, then type "date" -> "/date"
        textView.insertText("/", replacementRange: textView.selectedRange())
        #expect(coordinator.slashAnchor == 0)
        textView.doCommand(by: #selector(NSResponder.cancelOperation(_:)))
        #expect(coordinator.slashAnchor == nil)
        for ch in "date" {
            textView.insertText(String(ch), replacementRange: textView.selectedRange())
        }
        #expect(source == "/date")

        // Pressing Escape again when no slash menu or popover is open exits focus without altering source
        textView.doCommand(by: #selector(NSResponder.cancelOperation(_:)))
        #expect(source == "/date")
        #expect(window.firstResponder !== textView)
    }

    // MARK: - 6. Recursive Template Analysis & Preview Diagnostics

    @Test func snippetPreviewDetectsNestedFormFieldsClipboardAndCycleDiagnostics() {
        let childID = UUID()
        let parentID = UUID()
        let child = SnippetRecord(
            id: childID,
            name: "Nested Child",
            triggers: [";child"],
            content: "Hello {{field name=\"recipient\" default=\"World\"}} from {{clipboard}}"
        )
        let parent = SnippetRecord(
            id: parentID,
            name: "Parent",
            triggers: [";parent"],
            content: "Prefix -> {{snippet id=\"\(childID.uuidString)\"}}"
        )
        let contents = [parentID: parent.content, childID: child.content]

        let fields = TemplateAnalysis.formFields(in: parent.content, rootSnippetID: parentID) { contents[$0] }
        let usesClipboard = TemplateAnalysis.usesClipboard(in: parent.content, rootSnippetID: parentID) { contents[$0] }
        #expect(fields.map(\.name) == ["recipient"])
        #expect(usesClipboard)

        let preview = SnippetPreviewView.render(parent.content, rootSnippetID: parentID, contents: contents)
        #expect(preview.text.contains("Prefix -> Hello World from ‹clipboard›"))
        #expect(preview.diagnostics.isEmpty)

        // Self-reference cycle is surfaced in diagnostics and does not recurse infinitely
        let cyclicContent = "Loop {{snippet id=\"\(parentID.uuidString)\"}}"
        let cyclicPreview = SnippetPreviewView.render(cyclicContent, rootSnippetID: parentID, contents: [parentID: cyclicContent])
        #expect(!cyclicPreview.diagnostics.isEmpty)
    }

    // MARK: - 7. Explicit Palette Insert Blocker Evaluation

    @Test func explicitInsertBlockerReportsActionableReasons() {
        // Missing Accessibility / PostEvent
        let noAx = ExpansionEngine.evaluateExplicitInsertBlocker(
            snippetExists: true,
            isPaused: false,
            secureInputActive: false,
            secureInputOwner: nil,
            postEvent: false,
            isBusy: false,
            targetBundleID: "com.apple.TextEdit",
            deniedBundleIDs: []
        )
        #expect(noAx?.contains("Accessibility") == true)

        // Excluded target app
        let excluded = ExpansionEngine.evaluateExplicitInsertBlocker(
            snippetExists: true,
            isPaused: false,
            secureInputActive: false,
            secureInputOwner: nil,
            postEvent: true,
            isBusy: false,
            targetBundleID: "com.1password.1password",
            deniedBundleIDs: ["com.1password.1password"]
        )
        #expect(excluded?.contains("disabled in the active app") == true)

        // Secure Input active
        let secure = ExpansionEngine.evaluateExplicitInsertBlocker(
            snippetExists: true,
            isPaused: false,
            secureInputActive: true,
            secureInputOwner: "Terminal",
            postEvent: true,
            isBusy: false,
            targetBundleID: "com.apple.TextEdit",
            deniedBundleIDs: []
        )
        #expect(secure?.contains("Secure Input is active (Terminal)") == true)
    }
}
