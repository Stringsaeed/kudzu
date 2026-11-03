//! The engine's callbacks into the app.

use kudzu_engine::{EngineHost, EngineStatus, ExpansionEvent, FormRequest};
use tauri::{AppHandle, Emitter};

use crate::state::{core, FormPayload, EV_FORM_REQUEST, EV_LIBRARY, EV_STATUS};
use crate::{tray, windows};

pub struct Host {
    pub app: AppHandle,
}

impl EngineHost for Host {
    fn status_changed(&self, status: EngineStatus) {
        let _ = self.app.emit(EV_STATUS, &status);
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || tray::update(&app, &status));
    }

    fn expanded(&self, event: ExpansionEvent) {
        let core = core(&self.app);
        let library = {
            let mut store = core.store.lock();
            store.record_usage(event.snippet_id, event.chars_saved);
            store.save_library();
            store.library.clone()
        };
        let _ = self.app.emit(EV_LIBRARY, &library);
    }

    fn form_requested(&self, request: FormRequest) {
        let core = core(&self.app);
        let payload = {
            let store = core.store.lock();
            let snippet = store.library.snippets.iter().find(|s| s.id == request.snippet_id);
            FormPayload {
                token: request.token,
                snippet_id: request.snippet_id,
                snippet_name: request.snippet_name.clone(),
                initial_values: store.form_memory.initial_values(request.snippet_id, &request.fields),
                fields: request.fields.clone(),
                content: snippet.map(|s| s.content.clone()).unwrap_or_default(),
                target_app: request.target_app.clone(),
            }
        };
        *core.pending_form.lock() = Some(payload.clone());
        if let Some(engine) = core.engine() {
            engine.set_suppressed(true);
        }
        let app = self.app.clone();
        let caret = request.caret;
        let _ = self.app.run_on_main_thread(move || {
            windows::show_form(&app, caret);
            let _ = app.emit(EV_FORM_REQUEST, &payload);
        });
    }
}
