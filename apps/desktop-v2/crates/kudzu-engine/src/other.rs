//! Inert stand-ins for platforms without an expansion engine yet.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::library::Library;
use crate::status::{self, StatusInputs};
use crate::{lock, AppInfo, EngineHost, EngineLibrary, EngineStatus, PermissionStatus, TapMode};

/// Keeps the library (for `render_snippet`) and pause state; never listens or types.
pub(crate) struct Imp {
    host: Arc<dyn EngineHost>,
    library: Mutex<Arc<Library>>,
    paused: AtomicBool,
    last_status: Mutex<Option<EngineStatus>>,
}

impl Imp {
    pub fn new(host: Arc<dyn EngineHost>) -> Arc<Imp> {
        Arc::new(Imp {
            host,
            library: Mutex::new(Arc::new(Library::default())),
            paused: AtomicBool::new(false),
            last_status: Mutex::new(None),
        })
    }

    pub fn start(&self) {
        self.publish_status();
    }

    pub fn stop(&self) {}

    pub fn shutdown(&self) {}

    pub fn reload(&self, library: EngineLibrary) {
        *lock(&self.library) = Arc::new(Library::new(&library, system::locale));
    }

    pub fn library(&self) -> Arc<Library> {
        lock(&self.library).clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        self.publish_status();
    }

    pub fn set_suppressed(&self, _suppressed: bool) {}

    pub fn status(&self) -> EngineStatus {
        status::build(StatusInputs {
            paused: self.paused.load(Ordering::SeqCst),
            tap: TapMode::Off,
            permissions: PermissionStatus::default(),
            post_event: false,
            secure_input: false,
            secure_input_owner: None,
        })
    }

    pub fn submit_form(&self, _token: u64, _values: HashMap<String, String>) {}

    pub fn cancel_form(&self, _token: u64) {}

    pub fn form_panel_focused(&self, _token: u64, _focused: bool) {}

    pub fn clipboard_text(&self) -> Option<String> {
        None
    }

    pub fn insert_snippet(
        &self,
        _snippet_id: Uuid,
        _target: Option<AppInfo>,
        _selection: Option<String>,
    ) {
    }

    pub fn host(&self) -> &dyn EngineHost {
        self.host.as_ref()
    }

    fn publish_status(&self) {
        let status = self.status();
        {
            let mut last = lock(&self.last_status);
            if last.as_ref() == Some(&status) {
                return;
            }
            *last = Some(status.clone());
        }
        self.host.status_changed(status);
    }
}

/// OS services behind [`crate::system`]; all inert.
pub(crate) mod system {
    use crate::{AppInfo, PermissionStatus, Rect};

    pub fn permissions() -> PermissionStatus {
        PermissionStatus::default()
    }
    pub fn request_accessibility() {}
    pub fn request_input_monitoring() {}
    pub fn open_settings(_pane: &str) {}
    pub fn frontmost_app() -> Option<AppInfo> {
        None
    }
    pub fn activate_app(_app: &AppInfo) -> bool {
        false
    }
    pub fn installed_apps() -> Vec<AppInfo> {
        Vec::new()
    }
    pub fn app_icon_png(_path: &str, _size: u32) -> Option<Vec<u8>> {
        None
    }
    pub fn caret_rect() -> Option<Rect> {
        None
    }
    pub fn copy_selection() -> Option<String> {
        None
    }
    pub fn selection_for_insert() -> Option<String> {
        None
    }
    pub fn read_clipboard() -> Option<String> {
        None
    }
    pub fn write_clipboard(_text: &str) -> bool {
        false
    }
    pub fn secure_input() -> (bool, Option<String>) {
        (false, None)
    }
    pub fn locale() -> String {
        std::env::var("LANG")
            .ok()
            .and_then(|l| l.split('.').next().map(str::to_string))
            .filter(|l| !l.is_empty() && l != "C" && l != "POSIX")
            .unwrap_or_else(|| "en_US".to_string())
    }
}

/// Clipboard history plumbing; inert.
pub(crate) mod clipboard {
    use crate::clipboard::{PasteError, Thumbnail};
    use crate::AppInfo;

    pub fn change_count() -> i64 {
        0
    }
    pub fn is_own_change(_count: i64) -> bool {
        false
    }
    pub fn read() -> Vec<Vec<(String, Vec<u8>)>> {
        Vec::new()
    }
    pub fn write(_items: &[Vec<(String, Vec<u8>)>]) -> i64 {
        0
    }
    pub fn can_paste() -> bool {
        false
    }
    pub fn paste(_target: Option<&AppInfo>) -> Result<(), PasteError> {
        Err(PasteError::NotAllowed)
    }
    pub fn thumbnail(_data: &[u8], _max_px: u32) -> Option<Thumbnail> {
        None
    }
}
