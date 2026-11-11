//! Forwards engine events to the services that own each effect.
//! Weak handles break the engine → router → service cycle. A retired engine's callbacks
//! no-op once those services are gone.

use crate::{
    activity::ActivityService,
    clipboard::ClipboardService,
    expansion::ExpansionService,
    forms::FormService,
    library::LibraryService,
    ports::{EngineEvents, Expansion, FormRequest, TriggerLost},
};
use kudzu_core::{activity::ActivityEvent, system::EngineStatus};
use std::sync::{Arc, Weak};

pub(super) struct EngineRouter {
    expansion: Weak<ExpansionService>,
    library: Weak<LibraryService>,
    forms: Weak<FormService>,
    activity: Weak<ActivityService>,
    clipboard: Weak<ClipboardService>,
}

impl EngineRouter {
    pub(super) fn new(
        expansion: &Arc<ExpansionService>,
        library: &Arc<LibraryService>,
        forms: &Arc<FormService>,
        activity: &Arc<ActivityService>,
        clipboard: &Arc<ClipboardService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            expansion: Arc::downgrade(expansion),
            library: Arc::downgrade(library),
            forms: Arc::downgrade(forms),
            activity: Arc::downgrade(activity),
            clipboard: Arc::downgrade(clipboard),
        })
    }
}

impl EngineEvents for EngineRouter {
    fn status_changed(&self, status: EngineStatus) {
        if let Some(expansion) = self.expansion.upgrade() {
            expansion.status_changed(status);
        }
    }

    fn expanded(&self, expansion: Expansion) {
        if let Some(library) = self.library.upgrade() {
            if let Err(error) =
                library.record_expansion(expansion.snippet_id, expansion.chars_saved)
            {
                log::warn!("recording an expansion failed: {error}");
            }
        }
    }

    fn form_requested(&self, request: FormRequest) {
        // `requested` takes over a Quick Search hold for this snippet. Do not hand off again.
        if let Some(forms) = self.forms.upgrade() {
            forms.requested(request);
        }
    }

    fn activity(&self, event: ActivityEvent) {
        if let Some(expansion) = self.expansion.upgrade() {
            expansion.activity_outcome(&event);
        }
        if let Some(activity) = self.activity.upgrade() {
            activity.record(event);
        }
    }

    fn trigger_lost(&self, lost: TriggerLost) {
        if let Some(expansion) = self.expansion.upgrade() {
            let event = expansion.trigger_lost(lost);
            self.activity(event);
        }
    }

    fn clipboard_history(&self, index: usize, current: Option<&str>) -> Option<String> {
        self.clipboard
            .upgrade()
            .and_then(|clipboard| clipboard.history_text(index, current))
    }
}
