//! Autosave (EDT-2, FEATURES §7). The draft takes every edit at once and is written 350 ms after
//! the last one, or at once on window blur, navigation, hiding, quit and ⌘S
//! (`snippet-editor.tsx:55-110`).
//!
//! Two deliberate differences from Tauri, which clears its dirty flag before the save finishes:
//!
//! - "Saved" appears only once `library.saved()` reports the revision the draft was accepted at
//!   as durable.
//! - A failed write shows "Couldn't save" (with the writer's sentence) until a later write
//!   succeeds. The failure lives on the editor, not the snippet, so it stays across navigation
//!   and hiding the window. A draft the library refuses stays dirty, so the next flush retries.
//!
//! [`Autosave`] is the pure part: deadlines on the frame clock and the status rules. `flush` and
//! `poll` bind it to the library.

use kudzu_app::persist::{Revision, SaveState};
use uuid::Uuid;

use crate::main::toasts::ToastSpec;
use crate::shell::MainState;

/// `SAVE_DELAY` in `snippet-editor.tsx`, in seconds on the frame clock.
pub const AUTOSAVE_DELAY: f64 = 0.350;
/// The header's copy for each status.
pub const SAVING: &str = "Saving…";
pub const SAVED: &str = "Saved";
pub const FAILED: &str = "Couldn't save";
const TOAST_ID: &str = "editor-save-error";

/// What the header's save status shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SaveStatus {
    /// Nothing to report: the label is there but transparent, as in Tauri.
    #[default]
    Idle,
    Saving,
    Saved,
    /// The library's sentence for the failure.
    Failed(String),
}

impl SaveStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle | Self::Saved => SAVED,
            Self::Saving => SAVING,
            Self::Failed(_) => FAILED,
        }
    }
}

/// The debounce and the write bookkeeping of the open editor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Autosave {
    /// When the debounced write is due, on the frame clock.
    due: Option<f64>,
    /// The highest revision this editor had accepted that is not durable yet, with its snippet.
    pending: Option<(Revision, Uuid)>,
    /// The snippet whose last accepted revision reached the disk.
    saved_for: Option<Uuid>,
    /// The failure shown until a write succeeds.
    failure: Option<String>,
    /// The failure is the library refusing the open draft, not the writer failing: it goes with
    /// that draft when another snippet opens (the toast already said why).
    refusal: bool,
    /// The failure a toast already announced.
    announced: Option<String>,
}

impl Autosave {
    /// An edit at `now`: the write is due `AUTOSAVE_DELAY` later, restarting the debounce.
    pub fn edited(&mut self, now: f64) {
        self.due = Some(now + AUTOSAVE_DELAY);
    }

    pub fn due(&self) -> Option<f64> {
        self.due
    }

    pub fn is_due(&self, now: f64) -> bool {
        self.due.is_some_and(|due| now >= due)
    }

    /// Nothing left to write (the draft is clean, or the snippet is gone).
    pub fn cancel(&mut self) {
        self.due = None;
    }

    /// The library accepted `id`'s draft at `revision`; it is in memory, not yet on disk.
    pub fn accepted(&mut self, id: Uuid, revision: Revision) {
        self.due = None;
        let newest = self.pending.map_or(revision, |(r, _)| r.max(revision));
        self.pending = Some((newest, id));
        self.saved_for = None;
    }

    /// The library refused the draft. The draft stays dirty; the failure shows until a later
    /// write succeeds.
    pub fn refused(&mut self, message: String) {
        self.due = None;
        self.failure = Some(message);
        self.refusal = true;
    }

    /// Reads the writer's state. True when what the header shows may have changed.
    pub fn observe(&mut self, saved: &SaveState) -> bool {
        let before = (self.pending, self.saved_for, self.failure.clone());
        if let Some((revision, id)) = self.pending {
            if saved.durable >= revision {
                self.pending = None;
                self.saved_for = Some(id);
                self.failure = None;
                self.refusal = false;
                self.announced = None;
            } else if let Some(error) = saved.error {
                self.failure = Some(error.to_string());
                self.refusal = false;
            }
        }
        before != (self.pending, self.saved_for, self.failure.clone())
    }

    /// The status for the open snippet `open` whose draft is `dirty`.
    pub fn status(&self, open: Uuid, dirty: bool) -> SaveStatus {
        if let Some(message) = &self.failure {
            return SaveStatus::Failed(message.clone());
        }
        if dirty || self.pending.is_some_and(|(_, id)| id == open) {
            return SaveStatus::Saving;
        }
        if self.saved_for == Some(open) {
            SaveStatus::Saved
        } else {
            SaveStatus::Idle
        }
    }

    /// A failure no toast has announced yet, once.
    pub fn take_announcement(&mut self) -> Option<String> {
        let failure = self.failure.clone()?;
        if self.announced.as_ref() == Some(&failure) {
            return None;
        }
        self.announced = Some(failure.clone());
        Some(failure)
    }

    /// Opening another snippet: its status starts idle. A writer failure and pending writes
    /// stay; a refusal of the draft that was open goes with it.
    pub fn opened(&mut self) {
        self.due = None;
        self.saved_for = None;
        if self.refusal {
            self.failure = None;
            self.refusal = false;
            self.announced = None;
        }
    }
}

/// Writes the open draft now: on blur, navigation, hiding, quit, ⌘S and when the debounce ends.
/// Commits a typed trigger first, quietly. Never blocks: the library accepts in memory and its
/// writer persists. True when something visible changed.
pub fn flush(s: &mut MainState) -> bool {
    let mut changed = super::triggers::commit_quietly(s);
    changed |= super::pull_fields(s);
    let Some(session) = s.editor.session.as_ref() else {
        s.editor.autosave.cancel();
        return changed;
    };
    if !session.dirty {
        s.editor.autosave.cancel();
        return changed;
    }
    let id = session.id;
    match s.svc.library.update_snippet(session.draft.clone()) {
        Ok((saved, revision)) => {
            if let Some(session) = s.editor.session.as_mut() {
                session.accepted(saved);
            }
            s.editor.autosave.accepted(id, revision);
            s.refresh_library_snapshot();
        }
        Err(error) => s.editor.autosave.refused(error.to_string()),
    }
    reconcile(s);
    true
}

/// The debounce, and what the writer reported since the last poll.
pub(crate) fn poll(s: &mut MainState) -> bool {
    let mut changed = false;
    if s.editor.autosave.is_due(s.frame.clock_secs) {
        changed |= flush(s);
    }
    changed |= reconcile(s);
    changed
}

/// Reads `library.saved()` and announces a new failure with the "Couldn't save" toast.
fn reconcile(s: &mut MainState) -> bool {
    let saved = s.svc.library.saved().get();
    let changed = s.editor.autosave.observe(&saved);
    if let Some(message) = s.editor.autosave.take_announcement() {
        let spec = ToastSpec::new(FAILED).description(message);
        s.frame.toasts.show(TOAST_ID, spec, s.frame.clock_secs);
        return true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_app::persist::PersistError;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn state(durable: u64, error: Option<PersistError>) -> SaveState {
        SaveState {
            durable: Revision(durable),
            error,
        }
    }

    #[test]
    fn the_write_is_due_350_ms_after_the_last_edit() {
        let mut a = Autosave::default();
        a.edited(1.0);
        assert!(!a.is_due(1.349));
        a.edited(1.2);
        assert!(!a.is_due(1.5), "a later edit restarts the debounce");
        assert!(a.is_due(1.55));
        a.cancel();
        assert!(!a.is_due(9.0));
    }

    #[test]
    fn saved_shows_only_once_the_accepted_revision_is_durable() {
        let mut a = Autosave::default();
        assert_eq!(a.status(id(1), false), SaveStatus::Idle);
        assert_eq!(a.status(id(1), true), SaveStatus::Saving);
        a.accepted(id(1), Revision(5));
        assert_eq!(a.status(id(1), false), SaveStatus::Saving);
        assert!(!a.observe(&state(4, None)));
        assert_eq!(a.status(id(1), false), SaveStatus::Saving);
        assert!(a.observe(&state(5, None)));
        assert_eq!(a.status(id(1), false), SaveStatus::Saved);
        assert_eq!(a.status(id(1), true), SaveStatus::Saving, "a new edit");
    }

    #[test]
    fn a_failed_write_shows_until_a_later_write_succeeds() {
        let mut a = Autosave::default();
        a.accepted(id(1), Revision(3));
        assert!(a.observe(&state(2, Some(PersistError::Timeout))));
        let failed = SaveStatus::Failed(PersistError::Timeout.to_string());
        assert_eq!(a.status(id(1), false), failed);
        assert_eq!(
            a.take_announcement(),
            Some(PersistError::Timeout.to_string())
        );
        assert_eq!(a.take_announcement(), None, "announced once");
        // Another snippet opens: the failure stays.
        a.opened();
        assert_eq!(a.status(id(2), false), failed);
        assert_eq!(a.status(id(2), true), failed, "editing keeps it");
        a.accepted(id(2), Revision(4));
        assert!(!a.observe(&state(2, Some(PersistError::Timeout))));
        assert_eq!(a.status(id(2), false), failed);
        // The writer's retry lands both revisions.
        assert!(a.observe(&state(4, None)));
        assert_eq!(a.status(id(2), false), SaveStatus::Saved);
    }

    #[test]
    fn a_refused_draft_fails_until_an_accepted_one_is_durable() {
        let mut a = Autosave::default();
        a.edited(0.0);
        a.refused("That snippet no longer exists.".into());
        assert_eq!(a.due(), None);
        let failed = SaveStatus::Failed("That snippet no longer exists.".into());
        assert_eq!(a.status(id(1), true), failed);
        assert!(!a.observe(&state(9, None)), "no write of ours was durable");
        assert_eq!(a.status(id(1), true), failed);
        a.accepted(id(1), Revision(10));
        assert!(a.observe(&state(10, None)));
        assert_eq!(a.status(id(1), false), SaveStatus::Saved);
    }

    #[test]
    fn a_refusal_goes_with_its_draft_and_a_writer_failure_stays() {
        let mut a = Autosave::default();
        a.refused("That snippet no longer exists.".into());
        assert_eq!(
            a.take_announcement().as_deref(),
            Some("That snippet no longer exists.")
        );
        a.opened();
        assert_eq!(
            a.status(id(2), false),
            SaveStatus::Idle,
            "the next snippet starts clean"
        );
        a.accepted(id(2), Revision(3));
        a.observe(&state(2, Some(PersistError::Timeout)));
        a.opened();
        assert_eq!(
            a.status(id(3), false),
            SaveStatus::Failed(PersistError::Timeout.to_string()),
            "a writer failure is not the draft's"
        );
    }

    #[test]
    fn another_snippet_starts_idle_while_the_first_is_written() {
        let mut a = Autosave::default();
        a.accepted(id(1), Revision(2));
        a.opened();
        assert_eq!(a.status(id(2), false), SaveStatus::Idle);
        assert!(a.observe(&state(2, None)));
        assert_eq!(a.status(id(2), false), SaveStatus::Idle);
        assert_eq!(a.status(id(1), false), SaveStatus::Saved);
    }

    #[test]
    fn each_status_has_its_tauri_copy() {
        assert_eq!(SaveStatus::Saving.label(), "Saving…");
        assert_eq!(SaveStatus::Saved.label(), "Saved");
        assert_eq!(SaveStatus::Idle.label(), "Saved");
        assert_eq!(SaveStatus::Failed("x".into()).label(), "Couldn't save");
    }
}
