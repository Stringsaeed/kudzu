//! Activity is process-local only: no persistence, sync, log or agent export.
mod remedy;
use crate::{
    library::LibraryService,
    ports::Clock,
    settings::SettingsService,
    watch::{Publisher, Watch},
};
use kudzu_core::{
    activity::{ActivityEntry, ActivityEvent, ActivityLog, ActivityOutcome, ActivitySource},
    system::EngineStatus,
};
pub use remedy::{ActivityError, AppliedRemedy, Remedy, RemedyState};
use std::sync::{Arc, Mutex, PoisonError};

struct State {
    log: ActivityLog,
    output: Publisher<Vec<ActivityEntry>>,
}
/// Only the local Activity screen receives this service. Its content is not serializable;
/// there is no writer, sync port, logging path or agent endpoint.
pub struct ActivityService {
    state: Mutex<State>,
    library: Arc<LibraryService>,
    settings: Arc<SettingsService>,
    status: Watch<EngineStatus>,
    clock: Arc<dyn Clock>,
}
impl ActivityService {
    pub fn new(
        library: Arc<LibraryService>,
        settings: Arc<SettingsService>,
        status: Watch<EngineStatus>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            state: Mutex::new(State {
                log: ActivityLog::new(),
                output: Publisher::new(Vec::new()),
            }),
            library,
            settings,
            status,
            clock,
        }
    }
    pub fn watch(&self) -> Watch<Vec<ActivityEntry>> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .output
            .watch()
    }
    /// Called only for full trigger matches, explicit outcomes and health changes. Never
    /// accept typed context as a trigger; only an exact, stored trigger may enter the ring.
    pub fn record(&self, mut event: ActivityEvent) {
        let library = self.library.watch().get();
        if event.outcome.is_health() {
            event.source = None;
            event.snippet_id = None;
            event.trigger = None;
        } else if event.outcome == ActivityOutcome::TriggerLost
            || !matches!(
                event.source,
                Some(ActivitySource::Typed | ActivitySource::Form)
            )
            || !library.snippets.iter().any(|s| {
                Some(s.id) == event.snippet_id
                    && event
                        .trigger
                        .as_ref()
                        .is_some_and(|trigger| s.triggers.contains(trigger))
            })
        {
            event.trigger = None;
        }
        let committed = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.log.push(event);
            state.output.replace(Arc::new(
                state
                    .log
                    .recent()
                    .into_iter()
                    .map(ActivityEntry::from)
                    .collect(),
            ))
        };
        committed.notification.notify();
    }
    pub fn clear(&self) {
        let committed = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.log.clear();
            state.output.replace(Arc::new(Vec::new()))
        };
        committed.notification.notify();
    }
}
#[cfg(test)]
mod tests;
