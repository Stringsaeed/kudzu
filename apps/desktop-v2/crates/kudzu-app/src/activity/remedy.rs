use super::ActivityService;
use crate::{library::LibraryError, settings::SettingsError, watch::Revision};
use kudzu_core::activity::{explain, ActivityEntry, ActivityOutcome, Fix};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemedyState {
    Action(Fix),
    FixedSince,
}
/// A suggestion for one historical event. apply re-evaluates it; callers cannot turn a stale
/// button into a different mutation. Routes/help stay in the local UI, never the OS protection.
#[derive(Clone)]
pub struct Remedy {
    entry_id: u64,
    pub state: RemedyState,
    pub label: &'static str,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppliedRemedy {
    Changed(Revision),
    FixedSince,
    Unavailable,
    Show { fix: Fix, snippet_id: Option<Uuid> },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ActivityError {
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
}
impl ActivityService {
    pub fn remedy(&self, entry: &ActivityEntry) -> Option<Remedy> {
        let fix = explain(&entry.event).fix?;
        let library = self.library.watch().get();
        let status = self.status.get();
        let snippet = library
            .snippets
            .iter()
            .find(|s| Some(s.id) == entry.event.snippet_id);
        let (fixed, label) = match &fix {
            Fix::Resume => (!status.paused, "Resume Kudzu"),
            Fix::TurnOnSnippet => (snippet?.is_enabled, "Turn on"),
            Fix::OpenSnippet => {
                snippet?;
                (false, "Snippet rules")
            }
            Fix::OpenGroup { group_id } => {
                let group = library.groups.iter().find(|g| g.id == *group_id)?;
                if matches!(entry.event.outcome, ActivityOutcome::GroupOff { .. }) {
                    (group.is_enabled, "Group settings")
                } else {
                    (false, "Group rules")
                }
            }
            Fix::OpenAppSettings => (false, "App settings"),
            Fix::OpenPermissions { .. } => (false, "Fix permission"),
            Fix::ExplainSecureInput => (false, "What's this?"),
        };
        Some(Remedy {
            entry_id: entry.event.id,
            state: if fixed {
                RemedyState::FixedSince
            } else {
                RemedyState::Action(fix)
            },
            label: if fixed { "Fixed since" } else { label },
        })
    }
    pub fn apply(&self, remedy: Remedy) -> Result<AppliedRemedy, ActivityError> {
        let entries = self.watch().get();
        let Some(entry) = entries.iter().find(|e| e.event.id == remedy.entry_id) else {
            return Ok(AppliedRemedy::Unavailable);
        };
        let Some(current) = self.remedy(entry) else {
            return Ok(AppliedRemedy::Unavailable);
        };
        match current.state {
            RemedyState::FixedSince => Ok(AppliedRemedy::FixedSince),
            RemedyState::Action(Fix::Resume) => Ok(AppliedRemedy::Changed(
                self.settings.update(|s| s.paused = false)?,
            )),
            RemedyState::Action(Fix::TurnOnSnippet) => {
                let id = entry.event.snippet_id.ok_or(LibraryError::MissingSnippet)?;
                let now = self.clock.now().into();
                let (_, revision) = self.library.transact(|library| {
                    let snippet = library
                        .snippets
                        .iter_mut()
                        .find(|s| s.id == id)
                        .ok_or(LibraryError::MissingSnippet)?;
                    if !snippet.is_enabled {
                        snippet.is_enabled = true;
                        snippet.updated_at = now;
                    }
                    Ok(())
                })?;
                Ok(AppliedRemedy::Changed(revision))
            }
            RemedyState::Action(fix) => Ok(AppliedRemedy::Show {
                fix,
                snippet_id: entry.event.snippet_id,
            }),
        }
    }
}
