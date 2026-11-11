//! Expansion coordination. S40 owns EngineEvents routing and the engine's start/stop.
mod feed;
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
    #[error(transparent)]
    Insertion(#[from] crate::ports::engine::InsertionError),
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
    library: Watch<KudzuLibrary>,
    target: Arc<dyn TargetApp>,
    engine: Arc<dyn ExpansionEngine>,
    renderer: Arc<render::Renderer>,
    settings: Arc<SettingsService>,
    suppression: Arc<Suppression>,
    status: Arc<Mutex<Publisher<EngineStatus>>>,
    feed: feed::Feed,
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
        Ok(Self {
            recovery: Mutex::new(recovery::Recovery::new()),
            renderer,
            library,
            target,
            engine: engine.clone(),
            settings,
            suppression: Suppression::new(engine),
            status,
            feed,
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
    pub fn suppress(&self) -> SuppressionGuard {
        self.suppression.acquire()
    }
    /// Source revisions applied by the feed; zero until the initial reload.
    pub fn feed(&self) -> Watch<FeedState> {
        self.feed.watch()
    }
    /// Stop this producer before stopping the engine and draining persistence.
    pub fn stop_feed(&self) {
        self.feed.stop();
    }
}
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
