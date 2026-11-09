import Foundation
import KudzuCore

// The system-wide expansion engine and the OS services around it.
//
// PUBLIC API CONTRACT — port of `kudzu-engine/src/lib.rs`; the app compiles against these
// signatures.
//
// Threading: every method is thread-safe and non-blocking for the caller except
// ``Engine/reload(_:)`` (builds the trigger index) and a few ``System`` calls noted below.
// ``EngineHost`` callbacks arrive on engine threads, never while engine locks are held, so the
// host may call back into the engine from them.

/// Callbacks from the engine into the host app. Called from engine threads.
public protocol EngineHost: AnyObject, Sendable {
    /// Status changed (pause, permissions, Secure Input, tap mode).
    func statusChanged(_ status: EngineStatus)
    /// A snippet expanded.
    func expanded(_ event: ExpansionEvent)
    /// A form snippet fired (typed trigger or palette): show the form, then call
    /// ``Engine/submitForm(token:values:)`` or ``Engine/cancelForm(token:)`` with the same token.
    func formRequested(_ request: FormRequest)
}

/// Everything the engine matches and renders against.
public struct EngineLibrary: Hashable, Sendable {
    public var snippets: [Snippet]
    public var groups: [Group]
    public var config: MatchingConfig
    /// Locale for date tags, e.g. `en_US`.
    public var locale: String

    public init(snippets: [Snippet] = [], groups: [Group] = [], config: MatchingConfig = MatchingConfig(), locale: String = "") {
        self.snippets = snippets
        self.groups = groups
        self.config = config
        self.locale = locale
    }
}

/// The expansion engine. All methods are thread-safe.
///
/// The engine holds its host strongly (like the Rust `Arc<dyn EngineHost>`); a host that keeps the
/// engine should call ``stop()`` when it is done, since the threads only exit once both are gone.
public final class Engine: Sendable {
    private let runtime: EngineRuntime

    /// Creates a stopped engine.
    public init(host: any EngineHost) {
        runtime = EngineRuntime(host: host)
    }

    deinit {
        runtime.shutdown()
    }

    /// Starts the tap and the 1 s status poll (attaches the tap once permissions allow). Idempotent.
    public func start() {
        runtime.start()
    }

    /// Stops listening and restores any pending clipboard.
    public func stop() {
        runtime.stop()
    }

    /// Replaces the library (index rebuilt off the tap thread; no-op if unchanged).
    /// `revision` nil always installs. A revision older than one already installed is ignored.
    public func reload(_ library: EngineLibrary, revision: UInt64? = nil) {
        runtime.reload(library, revision: revision)
    }

    /// Pauses or resumes expansion.
    public func setPaused(_ paused: Bool) {
        runtime.setPaused(paused)
    }

    /// While true (our palette or form window has focus) typed keys never expand.
    public func setSuppressed(_ suppressed: Bool) {
        runtime.setSuppressed(suppressed)
    }

    /// Current status.
    public var status: EngineStatus {
        runtime.currentStatus()
    }

    /// Fills a pending form; the engine reactivates the target app, deletes the trigger (if typed) and inserts.
    public func submitForm(token: UInt64, values: [String: String]) {
        runtime.submitForm(token: token, values: values)
    }

    /// Abandons a pending form; a typed trigger stays as typed.
    public func cancelForm(token: UInt64) {
        runtime.cancelForm(token: token)
    }

    /// Palette insert: reactivates `target` (the app frontmost before the palette opened), renders the
    /// snippet (requesting a form if needed) and inserts it. Counts as an expansion.
    public func insertSnippet(id: UUID, target: AppInfo?) {
        runtime.insertSnippet(id: id, target: target)
    }

    /// Renders a snippet to plain text (no cursor tag, no form → defaults) for "copy" actions.
    public func renderSnippet(id: UUID, formValues: [String: String] = [:]) -> String? {
        renderSnippetFormatted(id: id, formValues: formValues)?.text
    }

    /// Renders both clipboard flavors for explicit copy actions.
    public func renderSnippetFormatted(id: UUID, formValues: [String: String] = [:]) -> RenderResult? {
        let library = runtime.library()
        guard let snippet = library.snippets[id] else { return nil }
        let values = LibrarySnapshot.withDefaults(library.formFields(snippet), formValues)
        let clipboard = library.usesClipboard(snippet) ? System.readClipboard() : nil
        return library.render(snippet, clipboard: clipboard, formValues: values)
    }
}
