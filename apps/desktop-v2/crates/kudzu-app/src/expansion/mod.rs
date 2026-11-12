//! Expansion coordination. S40 owns EngineEvents routing and the engine's start/stop.
mod feed;
pub(crate) mod hold;
mod outcomes;
pub use outcomes::{InsertionOutcome, OutcomeNotice};
mod recovery;
pub use recovery::{LostTriggerNotice, RecoveryId, RetypeTrigger};
mod health;
pub use health::{sidebar_health, tray_summary, Health, HealthCopy};
mod render;
pub use render::{HistoryReader, PreviewContext, RenderEnvironment};
mod session;
mod suppression;
use crate::{
    ports::{ExpansionEngine, TargetApp},
    settings::{SettingsError, SettingsService},
    watch::{Publisher, Revision, Watch},
};
pub use feed::FeedState;
use kudzu_core::{library::model::KudzuLibrary, system::EngineStatus};
pub use session::QuickSearchSession;
use std::sync::{Arc, Mutex, PoisonError};
use suppression::Suppression;
pub use suppression::SuppressionGuard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExpansionError {
    #[error("That snippet no longer exists.")]
    MissingSnippet,
    #[error("The engine feed could not start.")]
    Start(#[from] crate::error::IoFailure),
    #[error("The engine feed stopped unexpectedly.")]
    WorkerStopped,
    #[error(transparent)]
    Port(#[from] crate::error::PortError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
}

pub struct ExpansionService {
    recovery: Mutex<recovery::Recovery>,
    outcomes: Mutex<Publisher<Option<OutcomeNotice>>>,
    library: Watch<KudzuLibrary>,
    target: Arc<dyn TargetApp>,
    engine: Arc<dyn ExpansionEngine>,
    renderer: Arc<render::Renderer>,
    settings: Arc<SettingsService>,
    suppression: Arc<Suppression>,
    status: Arc<Mutex<Publisher<EngineStatus>>>,
    feed: feed::Feed,
    /// Quick Search picks still keeping triggers suppressed, waiting for their outcome.
    holds: Arc<hold::Holds>,
    outcome_deadline: std::time::Duration,
}
impl ExpansionService {
    pub fn new(
        library: Watch<KudzuLibrary>,
        settings: Arc<SettingsService>,
        engine: Arc<dyn ExpansionEngine>,
        target: Arc<dyn TargetApp>,
        environment: RenderEnvironment,
    ) -> Result<Self, ExpansionError> {
        let status = Arc::new(Mutex::new(Publisher::new(engine.status())));
        let feed = feed::Feed::new(
            library.clone(),
            settings.watch(),
            engine.clone(),
            status.clone(),
            environment.locale.clone(),
        )?;
        let renderer = Arc::new(render::Renderer {
            library: library.clone(),
            environment,
        });
        let outcomes = Publisher::new(None);
        let holds = hold::Holds::new(outcomes.watch(), feed.watch());
        Ok(Self {
            recovery: Mutex::new(recovery::Recovery::new()),
            outcomes: Mutex::new(outcomes),
            renderer,
            library,
            target,
            engine: engine.clone(),
            settings,
            suppression: Suppression::new(engine),
            status,
            feed,
            holds,
            outcome_deadline: hold::OUTCOME_DEADLINE,
        })
    }
    pub fn status(&self) -> Watch<EngineStatus> {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watch()
    }
    /// Called by S40's EngineRouter, after engine locks are released.
    pub fn status_changed(&self, status: EngineStatus) {
        let committed = {
            self.status
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .replace(Arc::new(status))
        };
        committed.notification.notify();
    }
    pub fn set_paused(&self, paused: bool) -> Result<Revision, ExpansionError> {
        Ok(self.settings.update(|s| s.paused = paused)?)
    }
    /// Flips the `paused` setting in one update. Callers must not read `status().paused` and
    /// write its opposite: the status echoes the setting only after the feed applies it.
    pub fn toggle_paused(&self) -> Result<Revision, ExpansionError> {
        Ok(self.settings.update(|s| s.paused = !s.paused)?)
    }
    /// How long a Quick Search pick may keep triggers suppressed while no outcome arrives.
    pub fn with_outcome_deadline(mut self, deadline: std::time::Duration) -> Self {
        self.outcome_deadline = deadline;
        self
    }
    /// The forms service received the form for `snippet` and holds its own guard: a pick of
    /// that snippet, which opens the form instead of reporting, stops holding its own.
    pub(crate) fn hand_off_to_form(&self, snippet: uuid::Uuid) {
        self.holds
            .hand_off(kudzu_core::activity::ActivitySource::QuickSearch, snippet);
    }
    pub fn suppress(&self) -> SuppressionGuard {
        self.suppression.acquire()
    }
    /// Source revisions applied by the feed; zero until the initial reload.
    pub fn feed(&self) -> Watch<FeedState> {
        self.feed.watch()
    }
    /// S40 calls after every engine start, even without a store change. Wait for feed().generation
    /// to reach the returned generation before enabling insertion for the restarted engine.
    pub fn resync(&self) -> u64 {
        self.suppression.resync();
        self.feed.resync()
    }
    /// Stop this producer before stopping the engine and draining persistence.
    pub fn stop_feed(&self) {
        self.feed.stop();
    }
}
impl Drop for ExpansionService {
    fn drop(&mut self) {
        self.holds.clear();
    }
}
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
