//! Kudzu's system-wide expansion engine for macOS: the keyboard listener, matching, insertion,
//! and the OS services that share its bookkeeping.
//!
//! # Public API
//!
//! - [`Engine`]: the running engine. [`Engine::new`] takes an [`EngineHost`], which receives
//!   every report; [`Engine::start`] attaches the listener once permissions allow. The library
//!   comes in through [`Engine::reload`]; pausing, suppression, forms and Quick Search inserts
//!   through the other methods.
//! - [`EngineHost`]: what the engine reports: status, expansions, form requests, activity, a
//!   trigger a refused paste couldn't put back ([`TriggerLost`]), and the one question it asks
//!   (an older clipboard item for `{{clipboard history=N}}`).
//! - [`clipboard`]: the general pasteboard, shared with snippet pastes so that Kudzu's own
//!   writes are recognised and a pending restore is never overwritten.
//! - [`system`]: permissions, Secure Input, the frontmost app, the caret and the selection.
//! - The value types come from [`kudzu_core::system`] and are re-exported here.
//!
//! App enumeration, icons, image thumbnails, Settings links and the system locale are not the
//! engine's: they live in `kudzu-system`.
//!
//! # Threading
//!
//! Every [`Engine`] method is thread-safe and returns without waiting on the target app:
//! insertions and form submissions run on the engine's worker thread. [`Engine::reload`] builds
//! the trigger index on the caller's thread, so call it off the UI thread for large libraries.
//! [`Engine::stop`] (and dropping the engine) restores a clipboard a paste replaced.
//!
//! The engine runs three threads of its own: the listener (`kudzu-event-tap`), which matches
//! keys and must never block; the worker (`kudzu-engine-worker`), which renders, deletes,
//! pastes and types; and the poll (`kudzu-engine-poll`), which re-reads permissions and Secure
//! Input every second and lets go of input held too long. [`EngineHost`] callbacks arrive on
//! those threads (and on the caller's thread for [`Engine::set_paused`], [`Engine::stop`] and
//! [`Engine::cancel_form`]), never while an engine lock is held, so a host may call back into the engine from them. A
//! callback should return quickly: the worker waits for it.
//!
//! The [`clipboard`] and [`system`] functions are thread-safe; the ones that block say so.

#[cfg(not(target_os = "macos"))]
compile_error!("kudzu-engine is macOS-only");

mod activity;
mod hold;
mod input;
mod interruption;
mod library;
mod macos;
mod plan;
mod status;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use kudzu_core::matching::MatchingConfig;
use kudzu_core::template::{FormFieldSpec, RenderResult};
use kudzu_core::{GroupRecord, SnippetRecord};
use uuid::Uuid;

pub use kudzu_core::system::{AppInfo, EngineStatus, PermissionStatus, Rect, TapMode};

/// When an input event happened: the event's own timestamp, `CGEventGetTimestamp`. 0 means the
/// event carried none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventStamp(pub u64);

/// An input event as the form window saw it: the CGEvent behind `NSApp.currentEvent`. The key
/// code tells it apart from another event with the same stamp; with neither, the engine can't
/// place the event and assumes the worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub stamp: EventStamp,
    /// The virtual key code (`kCGKeyboardEventKeycode`) for a key event; `None` for a click.
    pub key_code: Option<u16>,
}

/// A change of the form panel's key status, as the form window reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelFocus {
    /// The panel became key (`true`) or stopped being key.
    pub key: bool,
    /// 1 for the first change this form's panel reports, then 2, 3, …: a gap means a report
    /// went missing, which interrupts a typed form.
    pub seq: u64,
    /// The last event the app had taken from its queue when the change happened
    /// (`NSApp.currentEvent`); `None` when there was none. Events up to it were routed before
    /// the change, later ones after it.
    pub after: Option<InputEvent>,
}

/// One of Kudzu's own windows, as the host knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KudzuWindow {
    /// Its window number (`NSWindow.windowNumber`, the window server's id).
    pub number: u32,
    /// It lets clicks through to whatever is under it (an overlay that ignores mouse events).
    pub passes_clicks_through: bool,
}

/// How a form was submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Submission {
    /// The event that submitted it (the Return, the click on Insert); `None` when unavailable,
    /// which counts any typing since the last focus report as after it.
    pub event: Option<InputEvent>,
    /// The `seq` of the panel's last key-status change before the submission (0 if it never
    /// changed). The trigger is deleted only once every one of those reports has arrived.
    pub focus_seq: u64,
}

/// The engine needs a form filled before it can insert a snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct FormRequest {
    /// Pass back to [`Engine::submit_form`], [`Engine::cancel_form`] and
    /// [`Engine::form_panel_focused`].
    pub token: u64,
    pub snippet_id: Uuid,
    pub snippet_name: String,
    /// Fields in first-appearance order, including nested snippets.
    pub fields: Vec<FormFieldSpec>,
    /// Caret rectangle in the target app, when the Accessibility API exposes it.
    pub caret: Option<Rect>,
    /// The app the text will go into.
    pub target_app: Option<AppInfo>,
    /// The selection captured for a Quick Search insert, so the form can preview `{{selection}}`.
    pub selection: Option<String>,
}

/// One successful expansion (for usage stats). Never contains typed or inserted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionEvent {
    pub snippet_id: Uuid,
    /// Inserted characters minus typed trigger characters (≥ 0).
    pub chars_saved: i64,
}

/// A paste-based expansion was refused after it had deleted some of its trigger, and typing it
/// back was refused too, so the document is missing what the user typed (D14). This happens when
/// permission to post events goes away, or Secure Input comes on, mid-insertion. The clipboard is
/// the user's again; the host should tell the user, who can undo in the target app or retype the
/// trigger. Never contains typed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerLost {
    pub snippet_id: Uuid,
    /// The trigger as stored (the user may have typed it in another case).
    pub trigger: String,
}

/// Callbacks from the engine into the host app. Called from engine threads (see the crate's
/// threading notes), never while an engine lock is held.
pub trait EngineHost: Send + Sync + 'static {
    /// Status changed (pause, permissions, Secure Input, tap mode).
    fn status_changed(&self, status: EngineStatus);
    /// A snippet expanded.
    fn expanded(&self, event: ExpansionEvent);
    /// A form snippet fired (typed trigger or palette): show the form, then call
    /// [`Engine::submit_form`] or [`Engine::cancel_form`] with the same token.
    fn form_requested(&self, request: FormRequest);
    /// Plain text of the copy `index` before the current clipboard (1 = the previous copy), for
    /// `{{clipboard history=N}}`. `current` is the clipboard's plain text now. A host without a
    /// clipboard history keeps the default, and those tags render empty.
    fn clipboard_history(&self, _index: usize, _current: Option<&str>) -> Option<String> {
        None
    }

    /// A trigger fully matched, or engine health changed: what happened and why, for the
    /// "Why didn't this expand?" log. Never contains typed or inserted text. Keep it in memory.
    fn activity(&self, _event: kudzu_core::activity::ActivityEvent) {}

    /// See [`TriggerLost`]. The activity log also records the failure.
    fn trigger_lost(&self, _lost: TriggerLost) {}
}

/// Everything the engine matches and renders against.
#[derive(Debug, Clone, Default)]
pub struct EngineLibrary {
    pub snippets: Vec<SnippetRecord>,
    pub groups: Vec<GroupRecord>,
    pub config: MatchingConfig,
    /// Locale for date tags, e.g. `en_US`. The host passes the system locale; the engine never
    /// reads it from the OS, and an empty one means `en_US`.
    pub locale: String,
}

/// The expansion engine. All methods are thread-safe; dropping it stops it.
pub struct Engine {
    imp: Arc<macos::Imp>,
}

impl Engine {
    /// Creates a stopped engine on the real system.
    pub fn new(host: Arc<dyn EngineHost>) -> Arc<Engine> {
        Arc::new(Engine {
            imp: macos::Imp::new(host, macos::Platform::system()),
        })
    }

    /// Creates a stopped engine on `platform`'s seams (the protocol tests' fakes).
    #[cfg(test)]
    pub(crate) fn with_platform(
        host: Arc<dyn EngineHost>,
        platform: macos::Platform,
    ) -> Arc<Engine> {
        Arc::new(Engine {
            imp: macos::Imp::new(host, platform),
        })
    }

    /// Starts the listener and the 1 s status poll (the listener attaches once permissions
    /// allow). Idempotent.
    pub fn start(self: &Arc<Self>) {
        self.imp.start();
    }

    /// Stops listening, lets go of held input and restores any pending clipboard.
    pub fn stop(&self) {
        self.imp.stop();
    }

    /// Replaces the library. Builds the trigger index on the calling thread; a reload that
    /// changes nothing the engine matches or renders is a no-op, so typing in progress and an
    /// armed Backspace undo survive usage-count saves.
    pub fn reload(&self, library: EngineLibrary) {
        self.imp.reload(library);
    }

    /// Pauses or resumes expansion.
    pub fn set_paused(&self, paused: bool) {
        self.imp.set_paused(paused);
    }

    /// While true (one of Kudzu's own windows has focus) typed keys never expand. Suppression
    /// stops matching only: it doesn't hide input from a pending form (see
    /// [`Engine::form_panel_focused`]).
    pub fn set_suppressed(&self, suppressed: bool) {
        self.imp.set_suppressed(suppressed);
    }

    /// Current status.
    pub fn status(&self) -> EngineStatus {
        self.imp.status()
    }

    /// Fills a pending form: the engine brings the target app back, deletes the trigger (if it
    /// was typed) and inserts. The submission is accepted at once, on the caller's thread: a
    /// key-down after its event, any key-down or click from now until the trigger is deleted,
    /// and a focus report it counted that hasn't arrived by then, all make a typed form delete
    /// nothing. A stale or repeated token does nothing.
    pub fn submit_form(&self, token: u64, values: HashMap<String, String>, submission: Submission) {
        self.imp.submit_form(token, values, submission);
    }

    /// Abandons a pending form; a typed trigger stays as typed.
    pub fn cancel_form(&self, token: u64) {
        self.imp.cancel_form(token);
    }

    /// The form panel for `token` changed key status. A form starts out not key. Report every
    /// change, numbered, from the panel's key-status notification, starting when it first
    /// becomes key; reports may arrive late, even after the submission, and still count.
    ///
    /// A typed form whose trigger may have moved is interrupted, and its submission deletes
    /// nothing (FRM-4): a key-down that went anywhere but the key panel (as the reports place
    /// it), a click anywhere but in one of Kudzu's own windows, or a missing report. Typing in
    /// the key panel and clicking in Kudzu's windows are not interruptions.
    pub fn form_panel_focused(&self, token: u64, focus: PanelFocus) {
        self.imp.form_panel_focused(token, focus);
    }

    /// Replaces the set of Kudzu's windows on screen. Call it whenever one opens, closes, or
    /// changes whether it passes clicks through. A click counts as in Kudzu only when the
    /// window the session says can handle it is in this set and doesn't pass clicks through;
    /// before the first call, every click interrupts a typed form.
    pub fn set_kudzu_windows(&self, windows: &[KudzuWindow]) {
        self.imp.set_kudzu_windows(windows);
    }

    /// Quick Search insert: brings `target` (the app frontmost before the palette opened) back,
    /// renders the snippet (requesting a form if it has fields) and inserts it. Counts as an
    /// expansion.
    pub fn insert_snippet(&self, snippet_id: Uuid, target: Option<AppInfo>) {
        self.imp.insert_snippet(snippet_id, target, None);
    }

    /// [`Engine::insert_snippet`] with the text that was selected in `target` before the palette
    /// took focus (see [`system::selection_for_insert`]); `{{selection}}` renders it.
    pub fn insert_snippet_with_selection(
        &self,
        snippet_id: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) {
        self.imp.insert_snippet(snippet_id, target, selection);
    }

    /// Renders a snippet to plain text (no cursor tag, no form → defaults) for "copy" actions.
    pub fn render_snippet(
        &self,
        snippet_id: Uuid,
        form_values: HashMap<String, String>,
    ) -> Option<String> {
        self.render_snippet_formatted(snippet_id, form_values)
            .map(|r| r.text)
    }

    /// Renders both clipboard flavours for explicit copy actions. Key presses and pauses are in
    /// the result's `actions`, never in its text.
    pub fn render_snippet_formatted(
        &self,
        snippet_id: Uuid,
        form_values: HashMap<String, String>,
    ) -> Option<RenderResult> {
        self.render_snippet_with_selection(snippet_id, form_values, None)
    }

    /// [`Engine::render_snippet_formatted`] with a captured selection for `{{selection}}`.
    pub fn render_snippet_with_selection(
        &self,
        snippet_id: Uuid,
        form_values: HashMap<String, String>,
        selection: Option<String>,
    ) -> Option<RenderResult> {
        let library = self.imp.library();
        let snippet = library.snippets.get(&snippet_id)?;
        let values = library::with_defaults(&library.form_fields(snippet), form_values);
        let clipboard = if library.uses_clipboard(snippet) {
            self.imp.clipboard_text()
        } else {
            None
        };
        let host = self.imp.host();
        let history = |index: usize, current: Option<&str>| host.clipboard_history(index, current);
        Some(library.render(
            snippet,
            library::RenderInputs {
                now: library::now(),
                clipboard,
                selection,
                form_values: values,
                history: &history,
            },
        ))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.imp.shutdown();
    }
}

/// Locks `mutex`, recovering from poisoning (a panicking callback must not wedge the engine).
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The general pasteboard, as clipboard history reads and writes it. The model lives in
/// `kudzu_core::clipboard`.
///
/// Snippet pastes go through the same pasteboard record, so Kudzu's own writes (restores,
/// history picks) are recognised as such, and a write here first finishes a snippet paste that
/// is still waiting to put the user's clipboard back.
pub mod clipboard {
    use super::{macos, AppInfo};
    pub use kudzu_core::clipboard::PasteItem;

    /// Why pasting into another app didn't happen.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PasteError {
        /// No Accessibility permission, or Secure Input is on.
        NotAllowed,
        /// The app to paste into quit or couldn't be brought forward.
        TargetGone,
    }

    /// The general pasteboard's change count; cheap enough to poll a few times a second.
    pub fn change_count() -> i64 {
        macos::clipboard::change_count()
    }

    /// Whether a change count came from Kudzu's own writes (snippet pastes, restores, history
    /// picks).
    pub fn is_own_change(count: i64) -> bool {
        macos::clipboard::is_own_change(count)
    }

    /// Every item and type on the pasteboard. May block briefly on data a source app provides
    /// lazily.
    pub fn read() -> Vec<PasteItem> {
        macos::clipboard::read()
    }

    /// Replaces the pasteboard with `items`, recorded as Kudzu's own change. Returns the new
    /// change count.
    pub fn write(items: &[PasteItem]) -> i64 {
        macos::clipboard::write(items)
    }

    /// Whether synthetic ⌘V can reach other apps right now.
    pub fn can_paste() -> bool {
        macos::clipboard::can_paste()
    }

    /// Brings `target` forward (when given) and presses ⌘V. Blocks up to ~1 s: call off the UI
    /// thread.
    pub fn paste(target: Option<&AppInfo>) -> Result<(), PasteError> {
        macos::clipboard::paste(target)
    }
}

/// Permissions, Secure Input, the frontmost app, the caret and the selection. None of these
/// needs a running engine.
pub mod system {
    use super::{macos, AppInfo, PermissionStatus, Rect};

    /// Current permission grants (cheap; safe to poll).
    pub fn permissions() -> PermissionStatus {
        macos::system::permissions()
    }

    /// Shows the system Accessibility prompt.
    pub fn request_accessibility() {
        macos::system::request_accessibility()
    }

    /// Shows the system Input Monitoring prompt.
    pub fn request_input_monitoring() {
        macos::system::request_input_monitoring()
    }

    /// The frontmost application.
    pub fn frontmost_app() -> Option<AppInfo> {
        macos::system::frontmost_app()
    }

    /// Brings `app` (by pid, else bundle id) to the front.
    pub fn activate_app(app: &AppInfo) -> bool {
        macos::system::activate_app(app)
    }

    /// Caret rectangle of the focused text element via Accessibility.
    pub fn caret_rect() -> Option<Rect> {
        macos::system::caret_rect()
    }

    /// Copies the current selection in the frontmost app (⌘C, read, restore clipboard).
    /// Blocks up to ~300 ms: call off the UI thread.
    pub fn copy_selection() -> Option<String> {
        macos::system::copy_selection()
    }

    /// The selection in the frontmost app, for `{{selection}}`: asked through Accessibility, and
    /// copied with ⌘C only when the app doesn't report a selection at all. An app that reports
    /// an empty selection is believed, so opening Quick Search doesn't press ⌘C (and beep)
    /// there. Blocks up to ~300 ms: call off the UI thread.
    pub fn selection_for_insert() -> Option<String> {
        macos::system::selection_for_insert()
    }

    /// Plain-text clipboard.
    pub fn read_clipboard() -> Option<String> {
        macos::system::read_clipboard()
    }

    /// Writes plain text to the clipboard (as Kudzu's own change).
    pub fn write_clipboard(text: &str) -> bool {
        macos::system::write_clipboard(text)
    }

    /// Whether Secure Input is on, and the owning app's name if known.
    pub fn secure_input() -> (bool, Option<String>) {
        macos::system::secure_input()
    }
}
