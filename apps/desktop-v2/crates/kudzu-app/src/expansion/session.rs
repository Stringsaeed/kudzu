use super::{hold::Holds, ExpansionError, ExpansionService, SuppressionGuard};
use crate::{
    ports::{engine::AdmissionError, TargetApp},
    tasks::{Task, TaskError},
};
use kudzu_core::{activity::ActivitySource, system::AppInfo, template};
use std::{sync::Arc, time::Duration};

/// Hold in the palette's Option. Every hide path takes/drops it, even with a cached window.
/// A completed begin task is the only signal to show the panel: capture has already finished.
pub struct QuickSearchSession {
    target: Option<AppInfo>,
    selection: Option<String>,
    apps: Arc<dyn TargetApp>,
    engine: Arc<dyn crate::ports::ExpansionEngine>,
    holds: Arc<Holds>,
    outcome_deadline: Duration,
    _suppress: SuppressionGuard,
}
impl QuickSearchSession {
    /// Consume the session on insert. The palette hides before calling; the adapter owns
    /// target activation and settle. Typed triggers stay suppressed until the pick's outcome
    /// (see `hold`): the engine activates the target before it holds input. A snippet with
    /// fields opens a form instead, which acquires its own guard via EngineEvents; the pick's
    /// goes once the forms service has the request.
    pub fn insert(
        self,
        snippet: uuid::Uuid,
    ) -> Result<Task<Result<(), AdmissionError>>, TaskError> {
        let Self {
            engine,
            target,
            selection,
            holds,
            outcome_deadline,
            _suppress,
            ..
        } = self;
        let hold = holds.hold(
            ActivitySource::QuickSearch,
            snippet,
            _suppress,
            outcome_deadline,
        );
        let refused = holds.clone();
        Task::spawn("kudzu-insert", move || {
            let admission = engine.insert(snippet, target, selection);
            if admission.is_err() {
                refused.release(hold);
            }
            admission
        })
        .inspect_err(|_| holds.release(hold))
    }
    pub fn target(&self) -> Option<&AppInfo> {
        self.target.as_ref()
    }
    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }
    /// Escape/toggle restoration. Blur or showing the main window simply drops the session.
    pub fn restore_target(self) -> Result<Task<Result<(), ExpansionError>>, TaskError> {
        Task::spawn("kudzu-palette-restore", move || {
            if let Some(target) = &self.target {
                self.apps.activate(target)?;
            }
            Ok(())
        })
    }
}
impl ExpansionService {
    pub fn begin_quick_search(
        &self,
    ) -> Result<Task<Result<QuickSearchSession, ExpansionError>>, TaskError> {
        let apps = self.target.clone();
        let engine = self.engine.clone();
        let library = self.library.clone();
        let (holds, outcome_deadline) = (self.holds.clone(), self.outcome_deadline);
        let guard = self.suppress();
        Task::spawn("kudzu-palette-capture", move || {
            let target = apps
                .frontmost()?
                .filter(|a| a.pid != Some(std::process::id() as i32));
            let needs_selection = library.get().snippets.iter().any(|s| {
                s.content.contains("selection")
                    && template::analyze(&template::parse(&s.content)).uses_selection
            });
            let selection = if target.is_some() && needs_selection {
                apps.selection()?
            } else {
                None
            };
            Ok(QuickSearchSession {
                target,
                selection,
                apps,
                engine,
                holds,
                outcome_deadline,
                _suppress: guard,
            })
        })
    }
}

fn insert(
    engine: Arc<dyn crate::ports::ExpansionEngine>,
    snippet: uuid::Uuid,
    target: Option<AppInfo>,
    selection: Option<String>,
) -> Result<Task<Result<(), AdmissionError>>, TaskError> {
    Task::spawn("kudzu-insert", move || {
        engine.insert(snippet, target, selection)
    })
}
impl ExpansionService {
    pub fn insert(
        &self,
        snippet: uuid::Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<Task<Result<(), AdmissionError>>, TaskError> {
        insert(self.engine.clone(), snippet, target, selection)
    }
}
