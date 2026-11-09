import Foundation
import KudzuCore

// The worker: typed expansions, Backspace undo and typed-trigger forms.
// Port of the `// MARK: - Worker` half of `macos/engine.rs`.

extension EngineRuntime {
    func run(_ job: Job) {
        switch job {
        case .matched(let matched):
            let generation = matched.generation
            do {
                try performMatched(matched)
            } catch {
                EngineLog.logger.warning("expansion failed: \(error.description, privacy: .public)")
                core.withLock { core in
                    if core.generation == generation {
                        core.resetMatcher()
                    }
                }
            }
            finish(generation)
        case .submit(let token, let values):
            submit(token: token, values: values)
        case .insert(let snippetId, let target):
            paletteInsert(snippetId: snippetId, target: target)
        case .replay:
            // One event at a time: its callback can start another expansion before we
            // release the rest (including triggers typed during a paste).
            core.withLock { $0.nextReplay() }?.post(Keys.replayMarker)
        }
    }

    /// Ends the insertion `generation` (if still current) and resumes replay.
    func finish(_ generation: UInt64) {
        let next = core.withLock { core -> HeldEvent? in
            guard core.generation == generation else { return nil }
            core.busy = false
            return core.nextReplay()
        }
        next?.post(Keys.replayMarker)
    }

    /// Starts an explicit insertion: holds typing (active tap) and returns its generation.
    func beginExplicit() -> UInt64 {
        let active = flags.withLock { $0.tapMode } == .active
        return core.withLock { core in
            core.generation &+= 1
            core.busy = true
            core.listenOnlyInterrupted = false
            core.resetMatcher()
            if active {
                core.startHold()
            }
            return core.generation
        }
    }

    private func performMatched(_ m: Matched) throws(InsertError) {
        let token = m.generation
        if m.mode == .listenOnly {
            // Listen-only can't hold input: let the final key reach the app before deleting.
            Thread.sleep(forTimeInterval: RuntimeTiming.listenOnlySettle)
        }
        if !isCurrent(token) || !allowed(.typed) || Apps.frontmostPid() != m.pid {
            return
        }
        if m.mode == .listenOnly {
            let interrupted = core.withLock { core in
                if core.listenOnlyInterrupted { core.resetMatcher() }
                return core.listenOnlyInterrupted
            }
            if interrupted {
                return
            }
        }
        switch m.event {
        case .expand(let trigger):
            let library = library()
            guard let snippet = library.snippets[trigger.snippetID] else { return }
            let fields = library.formFields(snippet)
            if !fields.isEmpty {
                return try requestTypedForm(token, trigger: trigger, snippet: snippet, fields: fields, mode: m.mode, swallowed: m.swallowed)
            }
            let rendered = render(library, snippet, values: [:])
            let swallowed = m.mode == .active ? m.swallowed : nil
            try insertExpansion(token, trigger: trigger, rendered: rendered, mode: m.mode, swallowed: swallowed, guard: .typed)
        case .undo(let undo):
            do {
                try Keys.moveRight(m.undoCaret)
                // A listen-only backspace already reached the document.
                let already = m.mode == .listenOnly ? 1 : 0
                try Keys.deleteBackward(max(0, undo.deleteCount - already))
                try Keys.typeText(undo.retype)
            } catch {
                throw .key(error)
            }
        }
    }

    func render(_ library: LibrarySnapshot, _ snippet: Snippet, values: [String: String]) -> RenderResult {
        // Read before any paste of ours replaces the clipboard, and only when the template needs it.
        let clipboard = library.usesClipboard(snippet) ? Pasteboard.readString() : nil
        return library.render(snippet, clipboard: clipboard, formValues: values)
    }

    /// Delivers text by typing or pasting.
    func deliver(_ text: String, html: String?, method: InsertionMethod) throws(InsertError) {
        if text.isEmpty {
            return
        }
        if Plan.shouldType(method, text) && (html == nil || method == .type) {
            do { try Keys.typeText(text) } catch { throw .key(error) }
        } else {
            do { try Pasteboard.pasteText(text, html: html) } catch { throw .paste(error) }
        }
    }

    private static func hasImages(_ spans: [FormatSpan]?) -> Bool {
        spans?.contains { if case .image = $0.style { true } else { false } } ?? false
    }

    func insertExpansion(
        _ token: UInt64,
        trigger: TriggerMatch,
        rendered: RenderResult,
        mode: PlanMode,
        swallowed: String?,
        guard guardKind: InsertGuard
    ) throws(InsertError) {
        let text = CasePropagation.apply(trigger.caseStyle, to: rendered.text)
        let offset = rendered.cursorOffset.map { Plan.casedCursorOffset(trigger.caseStyle, rendered.text, $0) }
        let spans = rendered.formatting.map { Plan.casedFormatting(trigger.caseStyle, rendered.text, $0) }
        let html = spans.map { FormattingHelpers.html(text, spans: $0) }
        let hasImages = trigger.insertion != .type && Self.hasImages(spans)
        var plan = InsertionPlan(trigger, text: text, cursorOffset: offset, mode: mode, swallowed: swallowed)
        do { try Keys.deleteBackward(plan.deleteCount) } catch { throw .key(error) }
        let caret = hasImages ? AX.caretSnapshot() : nil
        try deliver(plan.text, html: html, method: trigger.insertion)
        var canUndo = true
        if hasImages {
            if let (text, cursor) = Plan.imageDelivery(plan.text, spans ?? [], cursor: offset, observed: caret?.insertedUTF16()) {
                plan = InsertionPlan(trigger, text: text, cursorOffset: cursor, mode: mode, swallowed: swallowed)
            } else {
                plan.caretLeftCount = 0
                canUndo = false
            }
        }
        if !isCurrent(token) || !allowed(guardKind) {
            return
        }
        do {
            if let separator = plan.reemitSeparator {
                try Keys.typeText(separator)
            }
            try Keys.moveLeft(plan.caretLeftCount)
        } catch {
            throw .key(error)
        }
        let insertedCount = Plan.graphemeCount(plan.text)
        let caretLeft = plan.caretLeftCount
        core.withLock { core in
            if core.generation == token && canUndo {
                // The undo tracker accounts for the re-emitted separator itself.
                core.matcher.didExpand(match: trigger, insertedCount: insertedCount)
                core.undoCaretLeft = caretLeft
            } else if core.generation == token {
                core.resetMatcher()
            }
        }
        host.expanded(ExpansionEvent(snippetId: trigger.snippetID, charsSaved: Plan.charsSaved(plan.text, trigger.typedTrigger)))
    }

    private func requestTypedForm(
        _ token: UInt64,
        trigger: TriggerMatch,
        snippet: Snippet,
        fields: [FormFieldSpec],
        mode: PlanMode,
        swallowed: String?
    ) throws(InsertError) {
        // Put the withheld key back so the document shows the whole trigger (and separator);
        // the form submission then deletes it like listen-only would.
        if mode == .active, let swallowed {
            do { try Keys.typeText(swallowed) } catch { throw .key(error) }
        }
        let queued = core.withLock { core in
            core.preparingForm = true
            core.preparingInterrupted = false
            core.holdSince = nil
            core.resetMatcher()
            return core.held.drain()
        }
        let interrupted = queued.contains { $0.isKeyDown }
        for held in queued {
            held.post(Keys.syntheticMarker)
        }
        Thread.sleep(forTimeInterval: RuntimeTiming.settle)
        let target = Apps.frontmostApp()
        let formToken = core.withLock { core -> UInt64? in
            core.preparingForm = false
            // Typing already reached the document after the trigger, so deleting it would be
            // wrong. Leave the trigger rather than show a form we'd discard.
            if core.generation != token || interrupted || core.preparingInterrupted {
                return nil
            }
            let formToken = core.nextFormToken
            core.nextFormToken &+= 1
            core.pendingForm = PendingForm(token: formToken, snippetId: snippet.id, kind: .typed(trigger), target: target, interrupted: false)
            return formToken
        }
        guard let formToken else { return }
        if !allowed(.typed) {
            core.withLock { $0.pendingForm = nil }
            return
        }
        host.formRequested(FormRequest(
            token: formToken,
            snippetId: snippet.id,
            snippetName: snippet.name,
            fields: fields,
            caret: AX.caretRect(),
            targetApp: target
        ))
    }
}
