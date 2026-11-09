import Foundation
import KudzuCore

// Explicit insertions: form submission and the palette. Port of `Imp::submit`,
// `insert_submitted`, `palette_insert` and `insert_rendered` in `macos/engine.rs`.

extension EngineRuntime {
    func submit(token: UInt64, values: [String: String]) {
        let form = core.withLock { core -> PendingForm? in
            guard let form = core.pendingForm, form.token == token else { return nil }
            core.pendingForm = nil
            return form
        }
        guard let form else { return }
        if form.interrupted, case .typed = form.kind {
            return
        }
        if !explicitAllowed() {
            return
        }
        let target = form.target ?? Apps.frontmostApp()
        if let app = target, !Apps.activateAndWait(app, timeout: RuntimeTiming.activationTimeout) {
            EngineLog.logger.warning("form target \(app.bundleId, privacy: .public) did not come to the front")
            return
        }
        let generation = beginExplicit()
        Thread.sleep(forTimeInterval: RuntimeTiming.settle)
        do {
            try insertSubmitted(generation, form: form, target: target, values: values)
        } catch {
            EngineLog.logger.warning("form insertion failed: \(error.description, privacy: .public)")
        }
        finish(generation)
    }

    private func insertSubmitted(_ generation: UInt64, form: PendingForm, target: AppInfo?, values: [String: String]) throws(InsertError) {
        if !isCurrent(generation) || !explicitAllowed() || !targetIsFront(target) {
            return
        }
        let library = library()
        guard let snippet = library.snippets[form.snippetId] else { return }
        let values = LibrarySnapshot.withDefaults(library.formFields(snippet), values)
        let rendered = render(library, snippet, values: values)
        switch form.kind {
        case .typed(let trigger):
            // The trigger and any separator are in the document as typed.
            try insertExpansion(generation, trigger: trigger, rendered: rendered, mode: .listenOnly, swallowed: nil, guard: .explicit)
        case .palette:
            try insertRendered(generation, library: library, snippet: snippet, rendered: rendered, target: target)
        }
    }

    private func targetIsFront(_ target: AppInfo?) -> Bool {
        guard let app = target else { return true }
        guard let pid = Apps.resolvePid(app) else { return false }
        return Apps.frontmostPid() == pid
    }

    func paletteInsert(snippetId: UUID, target: AppInfo?) {
        if !explicitAllowed() {
            return
        }
        let library = library()
        guard let snippet = library.snippets[snippetId] else { return }
        let target = target ?? Apps.frontmostApp()
        // Global exclusions (password managers, login window) apply to explicit inserts too.
        if library.isDenied(target?.bundleId) {
            return
        }
        invalidate()
        let fields = library.formFields(snippet)
        if !fields.isEmpty {
            let targetFront = target.map { targetIsFront($0) } ?? false
            let formToken = core.withLock { core in
                let formToken = core.nextFormToken
                core.nextFormToken &+= 1
                core.pendingForm = PendingForm(token: formToken, snippetId: snippetId, kind: .palette, target: target, interrupted: false)
                return formToken
            }
            host.formRequested(FormRequest(
                token: formToken,
                snippetId: snippetId,
                snippetName: snippet.name,
                fields: fields,
                caret: targetFront ? AX.caretRect() : nil,
                targetApp: target
            ))
            return
        }
        if let app = target, !Apps.activateAndWait(app, timeout: RuntimeTiming.activationTimeout) {
            EngineLog.logger.warning("palette target \(app.bundleId, privacy: .public) did not come to the front")
            return
        }
        let generation = beginExplicit()
        Thread.sleep(forTimeInterval: RuntimeTiming.settle)
        do {
            if isCurrent(generation) && explicitAllowed() && targetIsFront(target) {
                let rendered = render(library, snippet, values: [:])
                try insertRendered(generation, library: library, snippet: snippet, rendered: rendered, target: target)
            }
        } catch {
            EngineLog.logger.warning("palette insertion failed: \(error.description, privacy: .public)")
        }
        finish(generation)
    }

    /// Inserts a rendered snippet at the caret without deleting anything (palette).
    private func insertRendered(
        _ generation: UInt64,
        library: LibrarySnapshot,
        snippet: Snippet,
        rendered: RenderResult,
        target: AppInfo?
    ) throws(InsertError) {
        let bundle = target.map(\.bundleId).flatMap { $0.isEmpty ? nil : $0 }
        let method = library.insertion(for: snippet, bundleID: bundle)
        let html = rendered.formatting.map { FormattingHelpers.html(rendered.text, spans: $0) }
        let hasImages = method != .type && (rendered.formatting?.contains { if case .image = $0.style { true } else { false } } ?? false)
        let caret = hasImages ? AX.caretSnapshot() : nil
        try deliver(rendered.text, html: html, method: method)
        if !isCurrent(generation) || !explicitAllowed() {
            return
        }
        let delivered: (text: String, cursor: Int?)? = hasImages
            ? Plan.imageDelivery(rendered.text, rendered.formatting ?? [], cursor: rendered.cursorOffset, observed: caret?.insertedUTF16())
            : (rendered.text, rendered.cursorOffset)
        if let delivered, let offset = delivered.cursor {
            do { try Keys.moveLeft(Plan.graphemesAfterUTF16(delivered.text, offset)) } catch { throw .key(error) }
        }
        host.expanded(ExpansionEvent(snippetId: snippet.id, charsSaved: Plan.charsSaved(rendered.text, "")))
    }
}
