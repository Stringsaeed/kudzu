//! The engine's callbacks into the app. Ported from the Tauri shell's `host.rs`.

use std::sync::Arc;

use kudzu_engine::{EngineHost, EngineStatus, ExpansionEvent, FormRequest};

use super::core::{core, Event, FormPayload, TrayUpdate};
use super::windows;

pub struct Host;

impl EngineHost for Host {
    fn status_changed(&self, status: EngineStatus) {
        let core = core();
        core.emit(Event::Status(status.clone()));
        core.shell().tray(TrayUpdate::Status(status));
    }

    fn expanded(&self, event: ExpansionEvent) {
        let core = core();
        let library = {
            let mut store = core.store.lock();
            store.record_usage(event.snippet_id, event.chars_saved);
            store.save_library();
            store.library.clone()
        };
        core.emit(Event::Library(Arc::new(library)));
    }

    fn form_requested(&self, request: FormRequest) {
        let core = core();
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
        // The form state lands before the window shows, so it never flashes the last form.
        core.emit(Event::FormRequest(Arc::new(payload)));
        windows::show_form(request.caret);
    }
}
