//! What the picker remembers between keys: the query and filter, the list, the footer's flash
//! and the timers behind it, the last deletion and the clip being pasted. Pure state: no
//! service, no window. `drive` applies the effects of `keys` to it.
//!
//! Timers are deadlines on the UI clock, so a headless script and the window agree and every
//! one of them expires: [`Picker::expire`] ends them and says when something changed.
//!
//! Clip text never reaches a log: `Debug` prints counts only.

use chrono::{DateTime, FixedOffset};
use kudzu_app::clipboard::DeletedClips;
use kudzu_app::watch::Revision;
use kudzu_core::clipboard::query::ClipFilter;
use kudzu_core::clipboard::ClipEntry;
use rune::TextEdit;
use uuid::Uuid;

use crate::clip::content::loader::FullText;
use crate::clip::list::{Density, ListState};
use crate::widgets::key_hints::Held;

/// A message in the footer (`say(text)`, 1.6 s).
pub const FLASH_SECS: f64 = 1.6;
/// An error in the footer.
pub const ERROR_FLASH_SECS: f64 = 3.0;
/// "Deleted. ⌘Z to undo".
pub const UNDO_FLASH_SECS: f64 = 4.0;
/// The Accessibility fallback sentence.
pub const FALLBACK_FLASH_SECS: f64 = 2.6;
/// ⌘↩ shows "Copied", then closes.
pub const COPY_HIDE_AFTER: f64 = 0.42;
/// Without permission the picker stays this long, so the sentence can be read.
pub const FALLBACK_HIDE_AFTER: f64 = 1.8;
/// Shown when a copy cannot be pasted for want of Accessibility permission.
pub const FALLBACK_NOTICE: &str =
    "Copied. To paste directly, allow Kudzu in Accessibility settings.";

/// The settings the picker reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub recording: bool,
    pub show_secrets: bool,
    pub paste_plain: bool,
    pub retention_days: u32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            recording: true,
            show_secrets: false,
            paste_plain: false,
            retention_days: 7,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flash {
    pub text: String,
    pub until: f64,
}

/// The picker closes at `due`; `restore` gives focus back to the app it came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HideAt {
    pub due: f64,
    pub restore: bool,
}

/// What the list says when it has no rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Empty {
    Paused,
    NothingCopied,
    NoMatch(String),
    NothingHere,
}

impl Empty {
    /// The state for an empty list; `None` while there are rows.
    pub fn of(recording: bool, total: usize, query: &str, rows: usize) -> Option<Self> {
        if rows > 0 {
            return None;
        }
        Some(if !recording {
            Self::Paused
        } else if total == 0 {
            Self::NothingCopied
        } else if !query.is_empty() {
            Self::NoMatch(query.to_owned())
        } else {
            Self::NothingHere
        })
    }

    pub fn title(&self) -> String {
        match self {
            Self::Paused => "Clipboard history is paused.".into(),
            Self::NothingCopied => "Nothing copied yet.".into(),
            Self::NoMatch(query) => format!("Nothing matches “{query}”."),
            Self::NothingHere => "Nothing here yet.".into(),
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            Self::Paused => "Turn it on in Kudzu's settings.",
            Self::NothingCopied => "Copy something in any app and it shows up here.",
            Self::NoMatch(_) | Self::NothingHere => {
                "Try another filter, or app: and is: to narrow it down."
            }
        }
    }

    /// The magnifier is for a search that found nothing among clips that exist.
    pub fn searching(&self) -> bool {
        matches!(self, Self::NoMatch(_))
    }
}

/// The footer's resting label (`pasteLabel`).
pub fn paste_label(can_paste: bool, target: Option<&str>) -> String {
    match (can_paste, target) {
        (false, _) => "Copy".to_owned(),
        (true, Some(name)) => format!("Paste into {name}"),
        (true, None) => "Paste".to_owned(),
    }
}

/// Whether to write plain text: a rich clip, with the setting flipped by ⇧.
pub fn plain_for(entry: &ClipEntry, paste_plain: bool, flip: bool) -> bool {
    entry.rich && paste_plain != flip
}

/// Markers of the last rebuild, so a poll rebuilds the rows only when something changed.
#[derive(Debug, Default)]
pub struct Seen {
    pub history: Option<Revision>,
    pub filter: Option<ClipFilter>,
    pub query: String,
    pub day: i64,
    pub active: Option<(Uuid, String)>,
}

pub struct Picker {
    pub edit: TextEdit,
    pub filter: ClipFilter,
    pub clips: ListState,
    pub full: FullText,
    pub prefs: Prefs,
    pub held: Held,
    /// Entries in the whole history, for the empty states.
    pub total: usize,
    /// The last deletion, for ⌘Z; gone at the next opening.
    pub undo: Option<DeletedClips>,
    pub flash: Option<Flash>,
    pub hide: Option<HideAt>,
    /// The pointer is on a concealed preview.
    pub cover_hover: bool,
    /// Keyboard focus is on a concealed preview.
    pub cover_focus: bool,
    /// Whether this opening can paste, and into which app.
    pub can_paste: bool,
    pub target: Option<String>,
    /// A paste asked for before the session landed: (clip, flip).
    pub pending_paste: Option<(Uuid, bool)>,
    pub seen: Seen,
}

fn compact(now: DateTime<FixedOffset>) -> ListState {
    let mut clips = ListState::new(now);
    clips.look.density = Density::Compact;
    clips.look.numbered = true;
    clips.look.keep_focus = true;
    clips
}

impl Picker {
    /// A concealed preview is uncovered: the pointer is on it, or keyboard focus is.
    pub fn cover_revealed(&self) -> bool {
        self.cover_hover || self.cover_focus
    }

    /// Keyboard focus reaches or leaves the concealed preview (`Tx::focused`, read in `poll`;
    /// Rune has no per-element focus hook). Only keyboard focus reveals, as `focus-visible`
    /// does. True when that changed what shows.
    pub fn note_focus(&mut self, focused: Option<&str>, keyboard: bool) -> bool {
        let on = keyboard && focused == Some(crate::clip::secret::COVER_KEY);
        std::mem::replace(&mut self.cover_focus, on) != on
    }

    pub fn new(now: DateTime<FixedOffset>) -> Self {
        let mut edit = TextEdit::new("");
        edit.focused = true;
        Self {
            edit,
            filter: ClipFilter::All,
            clips: compact(now),
            full: FullText::default(),
            prefs: Prefs::default(),
            held: Held::default(),
            total: 0,
            undo: None,
            flash: None,
            hide: None,
            cover_hover: false,
            cover_focus: false,
            can_paste: true,
            target: None,
            pending_paste: None,
            seen: Seen::default(),
        }
    }

    /// A fresh opening: filter, query, selection, undo, flash, timers and held keys all start
    /// over, and the search field takes focus. Pictures already loaded are kept.
    pub fn reset(&mut self, now: DateTime<FixedOffset>) {
        let media = self.clips.media.take();
        self.clips = compact(now);
        self.clips.media = media;
        self.filter = ClipFilter::All;
        self.edit = TextEdit::new("");
        self.edit.focused = true;
        self.held = Held::default();
        self.undo = None;
        self.flash = None;
        self.hide = None;
        self.cover_hover = false;
        self.cover_focus = false;
        self.pending_paste = None;
        self.target = None;
        self.can_paste = true;
        self.seen = Seen::default();
        self.full = FullText::default();
    }

    pub fn query(&self) -> &str {
        &self.edit.text
    }

    pub fn is_composing(&self) -> bool {
        self.edit.is_composing()
    }

    pub fn active(&self) -> Option<&ClipEntry> {
        self.clips.list.active_entry()
    }

    /// Shows `text` in the footer for `secs` from `clock`.
    pub fn say(&mut self, text: impl Into<String>, secs: f64, clock: f64) {
        self.flash = Some(Flash {
            text: text.into(),
            until: clock + secs,
        });
    }

    pub fn flash_text(&self) -> Option<&str> {
        self.flash.as_ref().map(|f| f.text.as_str())
    }

    /// Closes at `clock + after`; asking again moves the deadline.
    pub fn hide_after(&mut self, after: f64, restore: bool, clock: f64) {
        self.hide = Some(HideAt {
            due: clock + after,
            restore,
        });
    }

    /// Ends the flash when its time is up. True when it ended.
    pub fn expire(&mut self, clock: f64) -> bool {
        if self.flash.as_ref().is_some_and(|f| clock >= f.until) {
            self.flash = None;
            return true;
        }
        false
    }

    /// The close that is due, taken once.
    pub fn take_hide(&mut self, clock: f64) -> Option<HideAt> {
        if self.hide.is_some_and(|h| clock >= h.due) {
            return self.hide.take();
        }
        None
    }

    /// The soonest time something here changes by itself.
    pub fn next_deadline(&self) -> Option<f64> {
        [
            self.flash.as_ref().map(|f| f.until),
            self.hide.map(|h| h.due),
        ]
        .into_iter()
        .flatten()
        .reduce(f64::min)
    }

    pub fn paste_label(&self) -> String {
        paste_label(self.can_paste, self.target.as_deref())
    }
}

impl std::fmt::Debug for Picker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picker")
            .field("filter", &self.filter)
            .field("query_len", &self.edit.text.len())
            .field("rows", &self.clips.list.len())
            .field("total", &self.total)
            .field("has_undo", &self.undo.is_some())
            .field("has_flash", &self.flash.is_some())
            .field("hide", &self.hide)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::{clip, now};

    #[test]
    fn the_four_empty_states_say_what_they_say_in_tauri() {
        let paused = Empty::of(false, 5, "", 0).unwrap();
        assert_eq!(paused.title(), "Clipboard history is paused.");
        assert_eq!(paused.hint(), "Turn it on in Kudzu's settings.");
        let none = Empty::of(true, 0, "", 0).unwrap();
        assert_eq!(none.title(), "Nothing copied yet.");
        assert_eq!(
            none.hint(),
            "Copy something in any app and it shows up here."
        );
        let miss = Empty::of(true, 5, "zzz", 0).unwrap();
        assert_eq!(miss.title(), "Nothing matches “zzz”.");
        assert_eq!(
            miss.hint(),
            "Try another filter, or app: and is: to narrow it down."
        );
        assert!(miss.searching());
        let filter = Empty::of(true, 5, "", 0).unwrap();
        assert_eq!(filter.title(), "Nothing here yet.");
        assert!(!filter.searching() && !paused.searching() && !none.searching());
        assert_eq!(
            Empty::of(true, 5, "zzz", 3),
            None,
            "rows hide the empty state"
        );
    }

    #[test]
    fn paused_wins_over_nothing_copied_and_a_query_over_a_filter() {
        assert_eq!(Empty::of(false, 0, "x", 0), Some(Empty::Paused));
        assert_eq!(Empty::of(true, 0, "x", 0), Some(Empty::NothingCopied));
    }

    #[test]
    fn the_footer_names_the_target_or_says_copy_without_permission() {
        assert_eq!(paste_label(true, Some("Notes")), "Paste into Notes");
        assert_eq!(paste_label(true, None), "Paste");
        assert_eq!(paste_label(false, Some("Notes")), "Copy");
    }

    #[test]
    fn shift_flips_the_plain_setting_for_rich_clips_only() {
        let mut rich = clip("x", 1, false);
        rich.rich = true;
        let plain = clip("x", 1, false);
        assert!(!plain_for(&rich, false, false));
        assert!(plain_for(&rich, false, true));
        assert!(plain_for(&rich, true, false));
        assert!(!plain_for(&rich, true, true));
        assert!(
            !plain_for(&plain, true, false),
            "plain clips have no other flavour"
        );
        assert!(!plain_for(&plain, false, true));
    }

    #[test]
    fn a_flash_ends_at_its_deadline_and_not_before() {
        let mut p = Picker::new(now());
        p.say("Pinned. It won't expire.", FLASH_SECS, 10.0);
        assert!(!p.expire(11.59));
        assert_eq!(p.flash_text(), Some("Pinned. It won't expire."));
        assert!(p.expire(11.6));
        assert_eq!(p.flash_text(), None);
        assert!(!p.expire(99.0), "nothing left to end");
    }

    #[test]
    fn the_hide_is_due_once_and_a_later_ask_moves_it() {
        let mut p = Picker::new(now());
        p.hide_after(COPY_HIDE_AFTER, true, 5.0);
        assert_eq!(p.take_hide(5.4), None);
        assert_eq!(p.next_deadline(), Some(5.42));
        assert_eq!(
            p.take_hide(5.42),
            Some(HideAt {
                due: 5.42,
                restore: true
            })
        );
        assert_eq!(p.take_hide(9.0), None, "taken once");
        p.hide_after(FALLBACK_HIDE_AFTER, false, 8.0);
        assert_eq!(p.take_hide(9.79), None);
        assert_eq!(
            p.take_hide(9.8),
            Some(HideAt {
                due: 9.8,
                restore: false
            })
        );
    }

    #[test]
    fn the_next_deadline_is_the_soonest_of_the_flash_and_the_hide() {
        let mut p = Picker::new(now());
        assert_eq!(p.next_deadline(), None);
        p.say("x", FALLBACK_FLASH_SECS, 0.0);
        p.hide_after(FALLBACK_HIDE_AFTER, false, 0.0);
        assert_eq!(p.next_deadline(), Some(1.8));
    }

    #[test]
    fn an_opening_starts_over() {
        let mut p = Picker::new(now());
        p.filter = ClipFilter::Image;
        p.edit.replace_text("query");
        p.say("x", 5.0, 0.0);
        p.hide_after(1.0, true, 0.0);
        p.pending_paste = Some((Uuid::new_v4(), true));
        p.cover_hover = true;
        p.reset(now());
        assert_eq!(p.filter, ClipFilter::All);
        assert_eq!(p.query(), "");
        assert!(p.edit.focused, "the search field takes focus");
        assert!(p.flash.is_none() && p.hide.is_none() && p.undo.is_none());
        assert!(p.pending_paste.is_none() && !p.cover_hover);
        assert!(p.clips.look.numbered);
    }

    #[test]
    fn debug_never_prints_the_query() {
        let mut p = Picker::new(now());
        p.edit.replace_text("hunter2-secret");
        assert!(!format!("{p:?}").contains("hunter2"));
    }
}
