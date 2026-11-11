//! The native engine's host: forwards its reports to the port's sink, typed, while this
//! engine instance is current and a sink is attached.
use kudzu_app::ports::{
    EngineEvents, EngineInstance, EngineInstances, Expansion, FormRequest, TriggerLost,
};
use kudzu_core::{activity::ActivityEvent, system::EngineStatus};
use std::sync::{Arc, PoisonError, RwLock};

pub(super) struct ForwardingHost {
    instance: EngineInstance,
    instances: Arc<EngineInstances>,
    sink: RwLock<Option<Arc<dyn EngineEvents>>>,
}

impl ForwardingHost {
    pub(super) fn new(instance: EngineInstance, instances: Arc<EngineInstances>) -> Self {
        Self {
            instance,
            instances,
            sink: RwLock::new(None),
        }
    }

    pub(super) fn attach(&self, events: Arc<dyn EngineEvents>) {
        *self.sink.write().unwrap_or_else(PoisonError::into_inner) = Some(events);
    }

    pub(super) fn detach(&self) {
        self.sink
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }

    /// The sink, cloned so no lock is held while it runs; none for a retired instance.
    fn sink(&self) -> Option<Arc<dyn EngineEvents>> {
        if !self.instances.is_current(self.instance) {
            return None;
        }
        self.sink
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl kudzu_engine::EngineHost for ForwardingHost {
    fn status_changed(&self, status: EngineStatus) {
        if let Some(sink) = self.sink() {
            sink.status_changed(status);
        }
    }

    fn expanded(&self, event: kudzu_engine::ExpansionEvent) {
        if let Some(sink) = self.sink() {
            sink.expanded(Expansion {
                snippet_id: event.snippet_id,
                chars_saved: event.chars_saved,
            });
        }
    }

    fn form_requested(&self, request: kudzu_engine::FormRequest) {
        if let Some(sink) = self.sink() {
            sink.form_requested(FormRequest {
                token: self.instance.form_token(request.token),
                snippet_id: request.snippet_id,
                snippet_name: request.snippet_name,
                fields: request.fields,
                target: request.target_app,
                caret: request.caret,
                selection: request.selection,
            });
        }
    }

    fn clipboard_history(&self, index: usize, current: Option<&str>) -> Option<String> {
        self.sink()?.clipboard_history(index, current)
    }

    fn activity(&self, event: ActivityEvent) {
        if let Some(sink) = self.sink() {
            sink.activity(event);
        }
    }

    fn trigger_lost(&self, lost: kudzu_engine::TriggerLost) {
        if let Some(sink) = self.sink() {
            sink.trigger_lost(TriggerLost {
                snippet_id: lost.snippet_id,
                trigger: lost.trigger,
            });
        }
    }
}
