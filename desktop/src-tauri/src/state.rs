//! Shared app state: the store, the engine and transient window hand-offs.

use std::collections::HashMap;
use std::sync::Arc;

use kudzu_core::template::FormFieldSpec;
use kudzu_engine::{AppInfo, Engine, EngineLibrary};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::store::Store;

pub const EV_LIBRARY: &str = "kudzu://library";
pub const EV_SETTINGS: &str = "kudzu://settings";
pub const EV_STATUS: &str = "kudzu://status";
pub const EV_PALETTE_SHOW: &str = "kudzu://palette-show";
pub const EV_CLIPBOARD_SHOW: &str = "kudzu://clipboard-show";
pub const EV_FORM_REQUEST: &str = "kudzu://form-request";
pub const EV_NAVIGATE: &str = "kudzu://navigate";
pub const EV_NEW_SNIPPET: &str = "kudzu://new-snippet";

/// What the form window shows: the engine's request plus remembered values and the template.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormPayload {
    pub token: u64,
    pub snippet_id: Uuid,
    pub snippet_name: String,
    pub fields: Vec<FormFieldSpec>,
    pub initial_values: HashMap<String, String>,
    pub content: String,
    pub target_app: Option<AppInfo>,
}

pub struct AppCore {
    pub app: AppHandle,
    pub store: Mutex<Store>,
    pub engine: Mutex<Option<Arc<Engine>>>,
    /// The app that was frontmost when the palette opened.
    pub palette_target: Mutex<Option<AppInfo>>,
    pub pending_form: Mutex<Option<FormPayload>>,
    /// Shortcut currently registered for the palette.
    pub registered_hotkey: Mutex<Option<String>>,
    /// Clipboard history.
    pub clip: Mutex<crate::clipboard::ClipStore>,
    /// The app that was frontmost when the clipboard picker opened (paste goes there).
    pub clip_target: Mutex<Option<AppInfo>>,
    /// Shortcut currently registered for the clipboard picker.
    pub registered_clip_hotkey: Mutex<Option<String>>,
}

impl AppCore {
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
        let _ = self.app.emit(EV_LIBRARY, &library);
        if let Some(engine) = self.engine() {
            engine.reload(engine_lib);
        }
        crate::cloud::local_changed(&self.app, false);
    }

    /// Persists settings, tells every window, and refreshes the engine.
    pub fn settings_changed(&self) {
        let (settings, engine_lib) = {
            let store = self.store.lock();
            store.save_settings();
            (store.settings.clone(), self.engine_library(&store))
        };
        let _ = self.app.emit(EV_SETTINGS, &settings);
        if let Some(engine) = self.engine() {
            engine.set_paused(settings.paused);
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

pub fn core(app: &AppHandle) -> Arc<AppCore> {
    app.state::<Arc<AppCore>>().inner().clone()
}
