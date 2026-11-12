//! The editor's state: the open snippet's draft, its fields and every screen-local part. No
//! `El` here; the parts' rules are unit-tested in the files that own them.

use kudzu_app::watch::Subscription;
use kudzu_core::SnippetRecord;
use rune::{TextEdit, Tx, Waker};
use uuid::Uuid;

use super::autosave::Autosave;
use super::behavior::Rules;
use super::content;
use super::header::Header;
use super::preview::Preview;
use super::triggers::Field as TriggerField;

/// The open snippet: the library's copy and the editor's.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: Uuid,
    /// The record as the library last held it, to tell an outside change from our own.
    pub base: SnippetRecord,
    /// The editor's copy: every edit lands here at once.
    pub draft: SnippetRecord,
    /// The draft holds edits the library has not accepted.
    pub dirty: bool,
}

/// What an outside change did to the open draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outside {
    /// The library holds what the editor last saw.
    Same,
    /// The draft was clean, so it took the change.
    Taken,
    /// The draft has edits of its own; they win when it saves (`snippet-editor.tsx:88-90`).
    Kept,
}

impl Session {
    pub fn open(record: &SnippetRecord) -> Session {
        Session {
            id: record.id,
            base: record.clone(),
            draft: record.clone(),
            dirty: false,
        }
    }

    /// `live` is the library's record now. Taken only when the draft is clean.
    pub fn outside(&mut self, live: &SnippetRecord) -> Outside {
        if *live == self.base {
            return Outside::Same;
        }
        self.base = live.clone();
        if self.dirty {
            return Outside::Kept;
        }
        self.draft = live.clone();
        Outside::Taken
    }

    /// Applies `edit` to the draft. True (and dirty) when it changed something.
    pub fn edit(&mut self, edit: impl FnOnce(&mut SnippetRecord)) -> bool {
        let before = self.draft.clone();
        edit(&mut self.draft);
        if self.draft == before {
            return false;
        }
        self.dirty = true;
        true
    }

    /// The library accepted the draft and returned what it stored.
    pub fn accepted(&mut self, saved: SnippetRecord) {
        self.base = saved.clone();
        self.draft = saved;
        self.dirty = false;
    }
}

/// A motion that runs for a while on the frame clock, then ends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timed {
    started: Option<f64>,
    duration: f64,
}

impl Timed {
    pub fn start(&mut self, now: f64, duration: f64) {
        self.started = Some(now);
        self.duration = duration;
    }

    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    /// Seconds since the start, while it runs.
    pub fn elapsed(&self, now: f64) -> Option<f64> {
        self.started.map(|t| (now - t).max(0.0))
    }

    pub fn deadline(&self) -> Option<f64> {
        self.started.map(|t| t + self.duration)
    }

    /// Ends the run once its time is up. True when it ended now.
    pub fn expire(&mut self, now: f64) -> bool {
        if self.deadline().is_some_and(|d| now + 1e-9 >= d) {
            self.started = None;
            return true;
        }
        false
    }

    pub fn stop(&mut self) {
        self.started = None;
    }

    /// Asks for a frame at the deadline while it runs.
    pub fn wake(&self, tx: &mut Tx) {
        if let Some(due) = self.deadline() {
            tx.wake_at(due);
        }
    }
}

/// Per-screen state of the snippet editor (S51; S52–S54 own the content field's parts).
pub struct State {
    pub session: Option<Session>,
    pub name: TextEdit,
    pub content: content::Field,
    pub trigger: TriggerField,
    pub autosave: Autosave,
    pub preview: Preview,
    pub header: Header,
    pub rules: Rules,
    /// Tauri's `focusNameOf`: a new or duplicated snippet whose name takes focus once it shows.
    /// It outlives the route that asked for it, and clears when another snippet opens.
    pub focus_name_of: Option<Uuid>,
    /// The main window's waker, for the save, conflict, preview and copy watches.
    pub(crate) wake: Waker,
    /// Reduce Motion as the last view saw it, for motion that starts in `poll`.
    pub(crate) reduced: std::cell::Cell<bool>,
    pub(crate) subscriptions: Vec<Subscription>,
}

impl State {
    pub fn new(wake: Waker) -> State {
        State {
            session: None,
            name: TextEdit::new(""),
            content: content::Field::default(),
            trigger: TriggerField::default(),
            autosave: Autosave::default(),
            preview: Preview::new(wake.clone()),
            header: Header::default(),
            rules: Rules::new(wake.clone()),
            focus_name_of: None,
            wake,
            reduced: std::cell::Cell::new(false),
            subscriptions: Vec::new(),
        }
    }

    pub fn open_id(&self) -> Option<Uuid> {
        self.session.as_ref().map(|s| s.id)
    }

    pub fn draft(&self) -> Option<&SnippetRecord> {
        self.session.as_ref().map(|s| &s.draft)
    }

    pub fn is_dirty(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.dirty)
    }

    pub fn is_composing(&self) -> bool {
        self.name.is_composing()
            || self.content.is_composing()
            || self.trigger.edit.is_composing()
            || self.rules.query.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.name.focused
            || self.content.is_focused()
            || self.trigger.edit.focused
            || self.rules.query.focused
    }

    pub fn blur_inputs(&mut self) {
        self.name.focused = false;
        self.content.blur();
        self.trigger.edit.focused = false;
        self.rules.query.focused = false;
    }

    /// Shows `record` in the fields. The name takes focus when it was asked for this snippet.
    pub fn load(&mut self, record: &SnippetRecord) {
        if self.focus_name_of != Some(record.id) {
            self.focus_name_of = None;
        }
        self.session = Some(Session::open(record));
        self.name = TextEdit::new(&record.name);
        self.name.focused = self.focus_name_of == Some(record.id);
        self.content.load(&record.content);
        self.trigger.opened(&record.triggers);
        self.autosave.opened();
        self.header.opened(record);
        self.rules.opened();
    }

    /// The fields after an outside change the clean draft took: text that did not change
    /// keeps its caret and focus.
    pub fn reload(&mut self, record: &SnippetRecord) {
        if self.name.text != record.name {
            let focused = self.name.focused;
            self.name = TextEdit::new(&record.name);
            self.name.focused = focused;
        }
        if self.content.source() != record.content {
            self.content.load(&record.content);
        }
    }

    /// Nothing is open.
    pub fn close(&mut self) {
        self.session = None;
        self.name = TextEdit::new("");
        self.content.load("");
        self.trigger = TriggerField::default();
        self.autosave.opened();
        self.preview.closed();
        self.rules.opened();
        self.header.closed();
    }

    /// Focuses `id`'s name once it shows (`store.ts` `newSnippet`, `duplicate`). The field is
    /// focused through its model, so this needs no `Tx` and works from a route applied in `poll`.
    pub fn request_name_focus(&mut self, id: Uuid) {
        self.blur_inputs();
        self.focus_name_of = Some(id);
        self.name.focused = true;
    }

    /// Tauri's `consumeFocusName`.
    pub fn take_name_focus(&mut self) -> Option<Uuid> {
        self.focus_name_of.take()
    }

    pub fn focus_content(&mut self, tx: &mut Tx) {
        self.blur_inputs();
        self.content.focus(tx);
    }

    pub fn focus_trigger(&mut self, tx: &mut Tx) {
        self.blur_inputs();
        self.trigger.edit.focused = true;
        tx.focus(super::TRIGGER_KEY, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::{CaseMode, FireMode};

    fn record(name: &str) -> SnippetRecord {
        SnippetRecord {
            id: Uuid::from_u128(7),
            name: name.into(),
            triggers: vec![";a".into()],
            content: "body".into(),
            formatting: None,
            group_id: None,
            fire_mode: FireMode::default(),
            case_mode: CaseMode::default(),
            insertion: None,
            rule: None,
            is_favorite: false,
            is_enabled: true,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }

    #[test]
    fn an_outside_change_is_taken_only_while_the_draft_is_clean() {
        let mut s = Session::open(&record("A"));
        assert_eq!(s.outside(&record("A")), Outside::Same);
        assert_eq!(s.outside(&record("B")), Outside::Taken);
        assert_eq!(s.draft.name, "B");
        assert!(s.edit(|d| d.name = "Mine".into()));
        assert!(s.dirty);
        assert_eq!(s.outside(&record("C")), Outside::Kept);
        assert_eq!(s.draft.name, "Mine", "the edit wins when it saves");
        assert_eq!(s.outside(&record("C")), Outside::Same, "seen once");
    }

    #[test]
    fn an_edit_that_changes_nothing_leaves_the_draft_clean() {
        let mut s = Session::open(&record("A"));
        assert!(!s.edit(|d| d.name = "A".into()));
        assert!(!s.dirty);
        assert!(s.edit(|d| d.is_favorite = true));
        s.accepted(s.draft.clone());
        assert!(!s.dirty);
        assert_eq!(s.base, s.draft);
    }

    #[test]
    fn timed_motion_ends_at_its_deadline() {
        let mut t = Timed::default();
        assert!(!t.running());
        t.start(2.0, 0.42);
        assert!(t.running());
        assert!(!t.expire(2.4));
        assert!(t.expire(2.42));
        assert!(!t.running());
        assert!(!t.expire(3.0), "ends once");
    }
}
