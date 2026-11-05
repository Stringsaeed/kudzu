//! The system-wide expansion engine and the OS services around it.
//!
//! PUBLIC API CONTRACT — the Tauri app compiles against these exact signatures.
//! macOS is fully implemented; other platforms compile with inert stubs.
//!
//! Threading: every method is thread-safe and non-blocking for the caller except
//! [`Engine::reload`] (builds the trigger index) and a few [`system`] calls noted below.
//! [`EngineHost`] callbacks arrive on engine threads, never while engine locks are held, so the
//! host may call back into the engine from them.

// The planning/hold helpers only have callers in the macOS engine.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod hold;
mod library;
mod plan;
mod status;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(not(target_os = "macos"))]
mod other;
#[cfg(not(target_os = "macos"))]
use other as platform;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use kudzu_core::matching::MatchingConfig;
use kudzu_core::template::FormFieldSpec;
use kudzu_core::{GroupRecord, SnippetRecord};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A screen rectangle in global top-left-origin points (logical pixels).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A running or installed application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub bundle_id: String,
    pub name: String,
    /// Process id when running.
    pub pid: Option<i32>,
    /// Bundle path on disk, when known.
    pub path: Option<String>,
}

/// Privacy permissions the engine needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    /// Accessibility (post events, read caret position, active tap).
    pub accessibility: bool,
    /// Input Monitoring (listen-only tap).
    pub input_monitoring: bool,
}

/// How the keyboard tap is currently attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TapMode {
    /// Not listening (missing permissions or stopped).
    Off,
    /// Can observe but not swallow keystrokes.
    ListenOnly,
    /// Can observe and swallow keystrokes (best reliability).
    Active,
}

/// Engine health for the tray and UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// Permission to insert synthetic keystrokes, independent of pause/Secure Input.
    #[serde(default)]
    pub can_post_events: bool,
    /// A live listener can insert, and neither pause nor Secure Input blocks it.
    #[serde(default)]
    pub can_expand: bool,
    pub paused: bool,
    pub tap: TapMode,
    pub permissions: PermissionStatus,
    pub secure_input: bool,
    /// App holding Secure Input, when known.
    pub secure_input_owner: Option<String>,
    /// One-line human summary ("Ready", "Paused", "Secure Input on — expansion blocked"…).
    pub summary: String,
}

/// The engine needs a form filled before it can insert a snippet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormRequest {
    /// Pass back to `submit_form` / `cancel_form`.
    pub token: u64,
    pub snippet_id: Uuid,
    pub snippet_name: String,
    /// Fields in first-appearance order, including nested snippets.
    pub fields: Vec<FormFieldSpec>,
    /// Caret rectangle in the target app, when the Accessibility API exposes it.
    pub caret: Option<Rect>,
    /// The app the text will go into.
    pub target_app: Option<AppInfo>,
}

/// One successful expansion (for usage stats). Never contains typed or inserted text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpansionEvent {
    pub snippet_id: Uuid,
    /// Inserted characters minus typed trigger characters (≥ 0).
    pub chars_saved: i64,
}

/// Callbacks from the engine into the host app. Called from engine threads.
pub trait EngineHost: Send + Sync + 'static {
    /// Status changed (pause, permissions, Secure Input, tap mode).
    fn status_changed(&self, status: EngineStatus);
    /// A snippet expanded.
    fn expanded(&self, event: ExpansionEvent);
    /// A form snippet fired (typed trigger or palette): show the form, then call
    /// `Engine::submit_form` or `Engine::cancel_form` with the same token.
    fn form_requested(&self, request: FormRequest);
}

/// Everything the engine matches and renders against.
#[derive(Debug, Clone, Default)]
pub struct EngineLibrary {
    pub snippets: Vec<SnippetRecord>,
    pub groups: Vec<GroupRecord>,
    pub config: MatchingConfig,
    /// Locale for date tags, e.g. `en_US`.
    pub locale: String,
}

/// The expansion engine. Cheap to clone the `Arc`; all methods are thread-safe.
pub struct Engine {
    imp: Arc<platform::Imp>,
}

impl Engine {
    /// Creates a stopped engine.
    pub fn new(host: Arc<dyn EngineHost>) -> Arc<Engine> {
        Arc::new(Engine { imp: platform::Imp::new(host) })
    }

    /// Starts the tap and the 1 s status poll (attaches the tap once permissions allow). Idempotent.
    pub fn start(self: &Arc<Self>) {
        self.imp.start();
    }

    /// Stops listening and restores any pending clipboard.
    pub fn stop(&self) {
        self.imp.stop();
    }

    /// Replaces the library (index rebuilt off the tap thread; no-op if unchanged).
    pub fn reload(&self, library: EngineLibrary) {
        self.imp.reload(library);
    }

    /// Pauses or resumes expansion.
    pub fn set_paused(&self, paused: bool) {
        self.imp.set_paused(paused);
    }

    /// While true (our palette or form window has focus) typed keys never expand.
    pub fn set_suppressed(&self, suppressed: bool) {
        self.imp.set_suppressed(suppressed);
    }

    /// Current status.
    pub fn status(&self) -> EngineStatus {
        self.imp.status()
    }

    /// Fills a pending form; the engine reactivates the target app, deletes the trigger (if typed) and inserts.
    pub fn submit_form(&self, token: u64, values: HashMap<String, String>) {
        self.imp.submit_form(token, values);
    }

    /// Abandons a pending form; a typed trigger stays as typed.
    pub fn cancel_form(&self, token: u64) {
        self.imp.cancel_form(token);
    }

    /// Palette insert: reactivates `target` (the app frontmost before the palette opened), renders the
    /// snippet (requesting a form if needed) and inserts it. Counts as an expansion.
    pub fn insert_snippet(&self, snippet_id: Uuid, target: Option<AppInfo>) {
        self.imp.insert_snippet(snippet_id, target);
    }

    /// Renders a snippet to plain text (no cursor tag, no form → defaults) for "copy" actions.
    pub fn render_snippet(&self, snippet_id: Uuid, form_values: HashMap<String, String>) -> Option<String> {
        let library = self.imp.library();
        let snippet = library.snippets.get(&snippet_id)?;
        let values = library::with_defaults(&library.form_fields(snippet), form_values);
        let clipboard = if library.uses_clipboard(snippet) { system::read_clipboard() } else { None };
        Some(library.render(snippet, library::now(), clipboard, values).text)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.imp.shutdown();
    }
}

/// Locks `mutex`, recovering from poisoning (a panicking callback must not wedge the engine).
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Clipboard history plumbing (macOS; inert elsewhere). The model lives in `kudzu_core::clipboard`.
pub mod clipboard {
    use super::{platform, AppInfo};
    pub use kudzu_core::clipboard::PasteItem;

    /// A PNG thumbnail plus the original image's pixel size.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Thumbnail {
        pub png: Vec<u8>,
        pub width: u32,
        pub height: u32,
    }

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
        platform::clipboard::change_count()
    }

    /// Whether a change count came from Kudzu's own writes (snippet pastes, restores, history picks).
    pub fn is_own_change(count: i64) -> bool {
        platform::clipboard::is_own_change(count)
    }

    /// Every item and type on the pasteboard. May block briefly on data a source app provides lazily.
    pub fn read() -> Vec<PasteItem> {
        platform::clipboard::read()
    }

    /// Replaces the pasteboard with `items`, recorded as Kudzu's own change. Returns the new change count.
    pub fn write(items: &[PasteItem]) -> i64 {
        platform::clipboard::write(items)
    }

    /// Whether synthetic ⌘V can reach other apps right now.
    pub fn can_paste() -> bool {
        platform::clipboard::can_paste()
    }

    /// Brings `target` forward (when given) and presses ⌘V. Blocks up to ~1 s: call off the UI thread.
    pub fn paste(target: Option<&AppInfo>) -> Result<(), PasteError> {
        platform::clipboard::paste(target)
    }

    /// Decodes image data (PNG, TIFF, JPEG, HEIC, PDF …) and renders a PNG at most `max_px` on its
    /// longer side (never upscaled).
    pub fn thumbnail(data: &[u8], max_px: u32) -> Option<Thumbnail> {
        platform::clipboard::thumbnail(data, max_px)
    }
}

/// OS services that don't need a running engine.
pub mod system {
    use super::{platform, AppInfo, PermissionStatus, Rect};

    /// Current permission grants (cheap; safe to poll).
    pub fn permissions() -> PermissionStatus {
        platform::system::permissions()
    }

    /// Shows the system Accessibility prompt.
    pub fn request_accessibility() {
        platform::system::request_accessibility()
    }

    /// Shows the system Input Monitoring prompt.
    pub fn request_input_monitoring() {
        platform::system::request_input_monitoring()
    }

    /// Opens System Settings at `pane`: "accessibility" | "inputMonitoring" | "loginItems".
    pub fn open_settings(pane: &str) {
        platform::system::open_settings(pane)
    }

    /// The frontmost application.
    pub fn frontmost_app() -> Option<AppInfo> {
        platform::system::frontmost_app()
    }

    /// Brings `app` (by pid, else bundle id) to the front.
    pub fn activate_app(app: &AppInfo) -> bool {
        platform::system::activate_app(app)
    }

    /// Installed apps (/Applications, ~/Applications, /System/Applications, …), sorted by name.
    /// Reads every bundle's Info.plist: call off the UI thread.
    pub fn installed_apps() -> Vec<AppInfo> {
        platform::system::installed_apps()
    }

    /// PNG bytes of an app's icon at `size` px, by bundle path.
    pub fn app_icon_png(path: &str, size: u32) -> Option<Vec<u8>> {
        platform::system::app_icon_png(path, size)
    }

    /// Caret rectangle of the focused text element via Accessibility.
    pub fn caret_rect() -> Option<Rect> {
        platform::system::caret_rect()
    }

    /// Copies the current selection in the frontmost app (⌘C, read, restore clipboard).
    /// Blocks up to ~300 ms: call off the UI thread.
    pub fn copy_selection() -> Option<String> {
        platform::system::copy_selection()
    }

    /// Plain-text clipboard.
    pub fn read_clipboard() -> Option<String> {
        platform::system::read_clipboard()
    }

    /// Writes plain text to the clipboard.
    pub fn write_clipboard(text: &str) -> bool {
        platform::system::write_clipboard(text)
    }

    /// Whether Secure Input is on, and the owning app's name if known.
    pub fn secure_input() -> (bool, Option<String>) {
        platform::system::secure_input()
    }

    /// Best-effort system locale identifier, e.g. `en_US`.
    pub fn locale() -> String {
        platform::system::locale()
    }
}
