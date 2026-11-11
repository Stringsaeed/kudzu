//! First-run flow: Welcome → Access → Import → Try it, with import recovery and finish retry.

use crate::ports::ExpansionEngine;
use crate::settings::SettingsService;
use crate::watch::{Publisher, Watch};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

pub const FINISH_ERROR: &str = "We couldn’t finish setup. Your library is safe; please try again.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Welcome,
    Access,
    Import,
    TryIt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportSource {
    Espanso,
    Raycast,
    TextExpander,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedFolder {
    pub path: PathBuf,
    pub snippet_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    Looking,
    Found { folders: Vec<DetectedFolder> },
    NotFound,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub count: usize,
    pub changed: usize,
    pub first_change: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportTally {
    pub imported_snippets: usize,
    pub merged_snippets: usize,
    pub warnings: usize,
    pub first_warning: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingState {
    pub step: Step,
    pub direction: i8,
    pub detection: Detection,
    pub outcomes: BTreeMap<ImportSource, Outcome>,
    pub failures: BTreeMap<ImportSource, String>,
    pub imported: usize,
    pub working: Option<ImportSource>,
    pub picking: bool,
    pub finishing: bool,
    pub error: Option<String>,
}

impl OnboardingState {
    pub fn busy(&self) -> bool {
        self.finishing || self.picking || self.working.is_some()
    }

    pub fn can_continue(&self) -> bool {
        match self.step {
            Step::Import => self.imported > 0,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OnboardingError {
    #[error("Finish this step first.")]
    NotReady,
    #[error("{}", FINISH_ERROR)]
    Finish,
    #[error("Setup is already busy.")]
    Busy,
}

struct Live {
    snapshot: OnboardingState,
    completed: BTreeSet<PathBuf>,
}

pub struct Onboarding {
    settings: Arc<SettingsService>,
    engine: Arc<dyn ExpansionEngine>,
    state: Mutex<Publisher<OnboardingState>>,
    completed: Mutex<BTreeSet<PathBuf>>,
}

impl Onboarding {
    pub fn new(settings: Arc<SettingsService>, engine: Arc<dyn ExpansionEngine>) -> Self {
        let snapshot = OnboardingState {
            step: Step::Welcome,
            direction: 0,
            detection: Detection::Looking,
            outcomes: BTreeMap::new(),
            failures: BTreeMap::new(),
            imported: 0,
            working: None,
            picking: false,
            finishing: false,
            error: None,
        };
        Self {
            settings,
            engine,
            state: Mutex::new(Publisher::new(snapshot)),
            completed: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn watch(&self) -> Watch<OnboardingState> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watch()
    }

    pub fn step(&self) -> Step {
        self.watch().get().step
    }

    pub fn snapshot(&self) -> Arc<OnboardingState> {
        self.watch().get()
    }

    pub fn pending_espanso_folders(&self) -> Vec<DetectedFolder> {
        let detection = self.snapshot().detection.clone();
        let completed = self
            .completed
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match detection {
            Detection::Found { folders } => folders
                .into_iter()
                .filter(|folder| !completed.contains(&folder.path))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn record_detection(&self, result: Result<Vec<DetectedFolder>, ()>) {
        self.edit(|live| {
            live.snapshot.detection = match result {
                Ok(folders) if folders.is_empty() => Detection::NotFound,
                Ok(folders) => Detection::Found { folders },
                Err(()) => Detection::Failed,
            };
        });
    }

    pub fn begin_import(&self, source: ImportSource) -> Result<(), OnboardingError> {
        self.edit(|live| {
            if live.snapshot.busy() {
                return Err(OnboardingError::Busy);
            }
            live.snapshot.working = Some(source);
            live.snapshot.failures.remove(&source);
            Ok(())
        })
    }

    pub fn begin_pick(&self) -> Result<(), OnboardingError> {
        self.edit(|live| {
            if live.snapshot.busy() {
                return Err(OnboardingError::Busy);
            }
            live.snapshot.picking = true;
            Ok(())
        })
    }

    pub fn end_pick(&self) {
        self.edit(|live| live.snapshot.picking = false);
    }

    pub fn record_import(&self, source: ImportSource, tally: ImportTally, accumulate: bool) {
        let added = tally
            .imported_snippets
            .saturating_sub(tally.merged_snippets);
        self.edit(|live| {
            live.snapshot.imported = live.snapshot.imported.saturating_add(added);
            let before = if accumulate {
                live.snapshot.outcomes.get(&source).cloned()
            } else {
                None
            };
            live.snapshot.outcomes.insert(
                source,
                Outcome {
                    count: before.as_ref().map(|o| o.count).unwrap_or(0) + added,
                    changed: before.as_ref().map(|o| o.changed).unwrap_or(0) + tally.warnings,
                    first_change: before.and_then(|o| o.first_change).or(tally.first_warning),
                },
            );
            live.snapshot.failures.remove(&source);
            live.snapshot.working = None;
            live.snapshot.picking = false;
            if let Some(path) = tally.path {
                live.completed.insert(path);
            }
        });
    }

    pub fn fail_import(&self, source: ImportSource, message: String) {
        self.edit(|live| {
            live.snapshot.failures.insert(source, message);
            live.snapshot.working = None;
            live.snapshot.picking = false;
        });
    }

    pub fn advance(&self) -> Result<(), OnboardingError> {
        let current = self.snapshot();
        if current.busy() {
            return Err(OnboardingError::Busy);
        }
        match current.step {
            Step::Welcome => self.go(Step::Access, 1),
            Step::Access => {
                if !crate::permissions::ready(&self.engine.status()) {
                    return Err(OnboardingError::NotReady);
                }
                self.go(Step::Import, 1)
            }
            Step::Import => {
                if current.imported == 0 {
                    return Err(OnboardingError::NotReady);
                }
                self.go(Step::TryIt, 1)
            }
            Step::TryIt => self.finish(),
        }
    }

    pub fn skip(&self) -> Result<(), OnboardingError> {
        let current = self.snapshot();
        if current.busy() {
            return Err(OnboardingError::Busy);
        }
        match current.step {
            Step::Welcome => self.go(Step::Access, 1),
            Step::Access => self.go(Step::Import, 1),
            Step::Import => self.go(Step::TryIt, 1),
            Step::TryIt => self.finish(),
        }
    }

    pub fn back(&self) -> Result<(), OnboardingError> {
        let current = self.snapshot();
        if current.busy() {
            return Err(OnboardingError::Busy);
        }
        match current.step {
            Step::Welcome => Ok(()),
            Step::Access => self.go(Step::Welcome, -1),
            Step::Import => self.go(Step::Access, -1),
            Step::TryIt => self.go(Step::Import, -1),
        }
    }

    pub fn finish(&self) -> Result<(), OnboardingError> {
        {
            let current = self.snapshot();
            if current.busy() && !current.finishing {
                return Err(OnboardingError::Busy);
            }
        }
        self.edit(|live| {
            live.snapshot.finishing = true;
            live.snapshot.error = None;
        });
        match self
            .settings
            .update(|settings| settings.onboarding_completed = true)
        {
            Ok(_) => {
                self.edit(|live| live.snapshot.finishing = false);
                Ok(())
            }
            Err(_) => {
                self.edit(|live| {
                    live.snapshot.finishing = false;
                    live.snapshot.error = Some(FINISH_ERROR.into());
                });
                Err(OnboardingError::Finish)
            }
        }
    }

    fn go(&self, step: Step, direction: i8) -> Result<(), OnboardingError> {
        self.edit(|live| {
            live.snapshot.step = step;
            live.snapshot.direction = direction;
            live.snapshot.error = None;
        });
        Ok(())
    }

    fn edit<T>(&self, update: impl FnOnce(&mut Live) -> T) -> T {
        let publisher = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut completed = self
            .completed
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut live = Live {
            snapshot: (*publisher.watch().get()).clone(),
            completed: completed.clone(),
        };
        let result = update(&mut live);
        *completed = live.completed;
        let committed = publisher.replace(Arc::new(live.snapshot));
        drop(completed);
        drop(publisher);
        committed.notification.notify();
        result
    }
}

#[cfg(test)]
mod tests;
