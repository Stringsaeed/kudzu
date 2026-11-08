//! Shared app state: the store, the engine and transient window hand-offs.
//!
//! This is the Tauri shell's `state.rs` without Tauri. Where Tauri emitted an event to every
//! webview, the core publishes an [`Event`] on its [`Bus`]; each Rune window subscribes and
//! applies it to its own state on the UI thread. Where Tauri managed windows, the core asks the
//! UI's [`Shell`].

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use kudzu_core::template::FormFieldSpec;
use kudzu_core::KudzuLibrary;
use kudzu_engine::{AppInfo, Engine, EngineLibrary, EngineStatus};
use parking_lot::Mutex;
use uuid::Uuid;

use super::clipboard::{ClipEvent, ClipStore};
use super::cloud::SyncStatus;
use super::updater::UpdateState;
use crate::settings::{Settings, Theme};
use crate::store::Store;

/// What the form window shows: the engine's request plus remembered values and the template.
#[derive(Debug, Clone)]
pub struct FormPayload {
    pub token: u64,
    pub snippet_id: Uuid,
    pub snippet_name: String,
    pub fields: Vec<FormFieldSpec>,
    pub initial_values: HashMap<String, String>,
    pub content: String,
    pub target_app: Option<AppInfo>,
}

/// A snippet the main window should create and open (from the palette, clipboard or tray).
#[derive(Debug, Clone, Default)]
pub struct NewSnippetDraft {
    pub content: Option<String>,
    pub name: Option<String>,
    pub trigger: Option<String>,
}

/// Everything windows hear about. Mirrors the Tauri events (`kudzu://library`, …).
#[derive(Debug, Clone)]
pub enum Event {
    Library(Arc<KudzuLibrary>),
    Settings(Arc<Settings>),
    Status(EngineStatus),
    Clipboard(Arc<ClipEvent>),
    /// Open a snippet, or a view written as `kind` or `kind:arg` (`settings:about`, `clipboard:image`).
    Navigate { snippet_id: Option<Uuid>, view: Option<String> },
    NewSnippet(NewSnippetDraft),
    PaletteShow { target: Option<AppInfo> },
    ClipboardShow { target: Option<AppInfo>, can_paste: bool },
    FormRequest(Arc<FormPayload>),
    Update(UpdateState),
    Sync(SyncStatus),
}

type Subscriber = Box<dyn Fn(&Event) + Send + Sync>;

/// Fans events out to every window. Subscribers run on the publishing thread and only queue
/// work for their window (`AppHandle::update`), so publishing never blocks on the UI.
#[derive(Default)]
pub struct Bus {
    subscribers: Mutex<Vec<Subscriber>>,
}

impl Bus {
    pub fn subscribe(&self, f: impl Fn(&Event) + Send + Sync + 'static) {
        self.subscribers.lock().push(Box::new(f));
    }

    pub fn emit(&self, event: Event) {
        for s in self.subscribers.lock().iter() {
            s(&event);
        }
    }
}

/// Kudzu's windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Win {
    Main,
    Palette,
    Clipboard,
    Form,
    Permissions,
}

/// Which global shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Palette,
    Clipboard,
}

/// What the menu-bar item shows.
#[derive(Debug, Clone)]
pub enum TrayUpdate {
    Status(EngineStatus),
    ClipboardEnabled(bool),
    PaletteHint(String),
    ClipboardHint(String),
    Update(UpdateState),
}

/// The UI's side of window management. Implemented by `ui::shell` over Rune handles.
/// Every method may be called from any thread.
pub trait Shell: Send + Sync {
    fn show(&self, win: Win);
    fn hide(&self, win: Win);
    fn is_visible(&self, win: Win) -> bool;
    fn is_focused(&self, win: Win) -> bool;
    /// Centres `win` horizontally on the display under the pointer, `top` of its height down.
    fn place_under_pointer(&self, win: Win, top: f32);
    /// Places `win` at a global top-left-origin rect in points.
    fn place_rect(&self, win: Win, x: f64, y: f64, width: f64, height: f64);
    /// The display under the pointer, as (x, y, w, h) in global points.
    fn pointer_display(&self) -> Option<(f64, f64, f64, f64)>;
    fn set_dock_icon(&self, visible: bool);
    fn register_hotkey(&self, slot: Slot, accelerator: &str) -> Result<(), String>;
    fn unregister_hotkey(&self, slot: Slot);
    fn tray(&self, update: TrayUpdate);
    fn apply_theme(&self, theme: Theme);
    fn quit(&self);
}

pub struct AppCore {
    pub store: Mutex<Store>,
    pub engine: Mutex<Option<Arc<Engine>>>,
    /// The app that was frontmost when the palette opened.
    pub palette_target: Mutex<Option<AppInfo>>,
    pub pending_form: Mutex<Option<FormPayload>>,
    /// Shortcut currently registered for the palette.
    pub registered_hotkey: Mutex<Option<String>>,
    /// Clipboard history.
    pub clip: Mutex<ClipStore>,
    /// The app that was frontmost when the clipboard picker opened (paste goes there).
    pub clip_target: Mutex<Option<AppInfo>>,
    /// Shortcut currently registered for the clipboard picker.
    pub registered_clip_hotkey: Mutex<Option<String>>,
    pub bus: Bus,
    shell: OnceLock<Box<dyn Shell>>,
}

static CORE: OnceLock<Arc<AppCore>> = OnceLock::new();

/// The app's core. Panics before [`install`] (as Tauri's `state()` did before `manage`).
pub fn core() -> Arc<AppCore> {
    CORE.get().expect("AppCore installed at launch").clone()
}

pub fn try_core() -> Option<Arc<AppCore>> {
    CORE.get().cloned()
}

pub fn install(core: Arc<AppCore>) {
    let _ = CORE.set(core);
}

impl AppCore {
    pub fn new(store: Store, clip: ClipStore) -> Self {
        Self {
            store: Mutex::new(store),
            engine: Mutex::new(None),
            palette_target: Mutex::new(None),
            pending_form: Mutex::new(None),
            registered_hotkey: Mutex::new(None),
            clip: Mutex::new(clip),
            clip_target: Mutex::new(None),
            registered_clip_hotkey: Mutex::new(None),
            bus: Bus::default(),
            shell: OnceLock::new(),
        }
    }

    pub fn set_shell(&self, shell: Box<dyn Shell>) {
        let _ = self.shell.set(shell);
    }

    /// The UI's window manager. Before the UI starts, calls go nowhere.
    pub fn shell(&self) -> &dyn Shell {
        self.shell.get().map(|b| b.as_ref()).unwrap_or(&NoShell)
    }

    pub fn emit(&self, event: Event) {
        self.bus.emit(event);
    }

    pub fn engine(&self) -> Option<Arc<Engine>> {
        self.engine.lock().clone()
    }

    /// Persists the library, tells every window, and refreshes the engine's index.
    pub fn library_changed(&self) {
        let (library, engine_lib) = {
            let store = self.store.lock();
            store.save_library();
            (store.library.clone(), self.engine_library(&store))
        };
        self.emit(Event::Library(Arc::new(library)));
        if let Some(engine) = self.engine() {
            engine.reload(engine_lib);
        }
        super::cloud::local_changed(false);
    }

    /// Persists settings, tells every window, and refreshes the engine.
    pub fn settings_changed(&self) {
        let (settings, engine_lib) = {
            let store = self.store.lock();
            store.save_settings();
            (store.settings.clone(), self.engine_library(&store))
        };
        let paused = settings.paused;
        self.emit(Event::Settings(Arc::new(settings)));
        if let Some(engine) = self.engine() {
            engine.set_paused(paused);
            engine.reload(engine_lib);
        }
    }

    pub fn engine_library(&self, store: &Store) -> EngineLibrary {
        EngineLibrary {
            snippets: store.library.snippets.clone(),
            groups: store.library.groups.clone(),
            config: store.settings.matching_config(),
            locale: kudzu_engine::system::locale(),
        }
    }
}

/// The shell before the UI exists: nothing to show.
struct NoShell;

impl Shell for NoShell {
    fn show(&self, _: Win) {}
    fn hide(&self, _: Win) {}
    fn is_visible(&self, _: Win) -> bool {
        false
    }
    fn is_focused(&self, _: Win) -> bool {
        false
    }
    fn place_under_pointer(&self, _: Win, _: f32) {}
    fn place_rect(&self, _: Win, _: f64, _: f64, _: f64, _: f64) {}
    fn pointer_display(&self) -> Option<(f64, f64, f64, f64)> {
        None
    }
    fn set_dock_icon(&self, _: bool) {}
    fn register_hotkey(&self, _: Slot, _: &str) -> Result<(), String> {
        Err("Kudzu isn't ready yet.".into())
    }
    fn unregister_hotkey(&self, _: Slot) {}
    fn tray(&self, _: TrayUpdate) {}
    fn apply_theme(&self, _: Theme) {}
    fn quit(&self) {}
}
