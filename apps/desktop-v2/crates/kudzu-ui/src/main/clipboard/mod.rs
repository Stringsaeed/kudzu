//! The clipboard history screen (`PLAN.md` S58): the list side. A header with the recording
//! badge and options, search, sections, rows, bulk actions and the retention footer on the
//! left; the preview pane on the right is a frame whose body S59 fills (`view::preview_body`).
//!
//! - `mod`: [`State`] and [`sync`] (pull the history, rebuild the rows).
//! - `keys`: the list's and the search field's keys.
//! - `actions`: copy, pin, delete with Undo, save as snippet, open, reveal, clear.
//! - `view`, `toolbar`, `bulk`: the layout, the header and footer, the selection bar and the
//!   clear dialog. `scenes`: fixture states for headless runs.
//!
//! Clip text never reaches a log, a toast or a label of a hidden secret.

pub(crate) mod actions;
mod bulk;
mod keys;
pub mod scenes;
mod toolbar;
mod view;

use chrono::FixedOffset;
use kudzu_app::watch::Revision;
use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
use kudzu_core::clipboard::sections::RowOptions;
use rune::{TextEdit, Tx};
use uuid::Uuid;

use crate::clip::list::{Hooks, ListState};
use crate::clip::model::Click;
use crate::main::nav;
use crate::main::state::{MainPane, MainView};
use crate::shell::MainState;

pub use keys::handle as handle_key;
pub use view::view;

pub const SEARCH_KEY: &str = "clipboard-search";
pub const LIST_KEY: &str = "clip-list";
/// How long the check stays beside a copied row.
const COPIED_SECS: f64 = 1.2;

/// The clear-history dialog.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClearDialog {
    pub include_pinned: bool,
}

/// The settings the list reads, as of the last sync.
#[derive(Clone, Copy, Debug)]
pub struct Prefs {
    pub recording: bool,
    pub retention_days: u32,
    pub show_secrets: bool,
    pub paste_plain: bool,
}

/// Per-screen state owned by the clipboard history module.
pub struct State {
    pub search: TextEdit,
    pub clips: ListState,
    pub prefs: Prefs,
    pub zone: FixedOffset,
    /// Bytes on disk, from a worker; `None` until the first answer.
    pub usage: Option<u64>,
    pub(crate) usage_fixed: bool,
    pub options_open: bool,
    pub retention_open: bool,
    pub clear: Option<ClearDialog>,
    /// Space: the large preview S59 draws.
    pub large_preview: bool,
    /// The last thing announced to assistive technology.
    pub announcement: String,
    /// Width of the content card, for the list's `clamp(300, 30%, 372)`.
    pub card_w: f32,
    pub(crate) copied_until: Option<f64>,
    pub(crate) jobs: Vec<actions::Job>,
    seen: Seen,
    visible: bool,
}

#[derive(Default)]
struct Seen {
    history: Option<Revision>,
    filter: Option<ClipFilter>,
    query: String,
    day: i64,
    usage_for: Option<Revision>,
}

impl Default for State {
    fn default() -> Self {
        let utc = FixedOffset::east_opt(0).unwrap_or_else(|| unreachable!("zero is an offset"));
        Self {
            search: TextEdit::new(""),
            clips: ListState::new(chrono::Utc::now().with_timezone(&utc)),
            prefs: Prefs {
                recording: true,
                retention_days: 7,
                show_secrets: false,
                paste_plain: false,
            },
            zone: utc,
            usage: None,
            usage_fixed: false,
            options_open: false,
            retention_open: false,
            clear: None,
            large_preview: false,
            announcement: String::new(),
            card_w: 948.0,
            copied_until: None,
            jobs: Vec::new(),
            seen: Seen::default(),
            visible: true,
        }
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("rows", &self.clips.list.len())
            .field("query_len", &self.search.text.len())
            .finish()
    }
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.search.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.search.focused
    }

    pub fn focus_search(&mut self, tx: &mut Tx) {
        self.search.focused = true;
        tx.focus(SEARCH_KEY, true);
    }

    pub fn blur_inputs(&mut self) {
        self.search.focused = false;
    }

    /// The window shows or hides: nothing arrives with motion while it is hidden.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn query(&self) -> &str {
        &self.search.text
    }

    /// Before the first sync nothing is known, and the list stays blank rather than say "empty".
    pub(crate) fn seen_nothing(&self) -> bool {
        self.seen.history.is_none()
    }

    pub fn has_query(&self) -> bool {
        !self.search.text.trim().is_empty()
    }
}

pub(crate) fn list_hooks() -> Hooks<MainState> {
    Hooks {
        state: |s: &MainState| &s.clipboard.clips,
        press: |s: &mut MainState, id: Uuid, click: Click, _tx: &mut Tx| {
            s.clipboard.clips.list.click(id, click);
            s.clipboard.large_preview = false;
            s.frame.focus.move_to(MainPane::List);
        },
        pick: |s: &mut MainState, id: Uuid, tx: &mut Tx| {
            s.clipboard.clips.list.activate(id);
            actions::copy_active(s, false, tx);
        },
    }
}

/// `⌘F` on the clipboard screen.
pub fn focus_search(s: &mut MainState, tx: &mut Tx) {
    s.blur_all_inputs();
    s.frame.focus.move_to(MainPane::List);
    s.clipboard.focus_search(tx);
}

/// `⌘L` on the clipboard screen.
pub fn focus_list(s: &mut MainState, tx: &mut Tx) {
    nav::focus_list_pane(s, LIST_KEY, tx);
}

/// The filter the open view shows.
pub(crate) fn filter_of(s: &MainState) -> ClipFilter {
    match s.frame.view {
        MainView::Clipboard(f) => f,
        _ => ClipFilter::All,
    }
}

/// Pulls settings, the history and finished jobs. True when something visible changed.
pub fn sync(s: &mut MainState) -> bool {
    let mut changed = actions::finish_jobs(s);
    let now_secs = s.frame.clock_secs;
    if s.clipboard.copied_until.is_some_and(|t| now_secs >= t) {
        s.clipboard.copied_until = None;
        s.clipboard.clips.look.copied = None;
        changed = true;
    }
    changed |= s.clipboard.clips.census.expire(now_secs);
    if !matches!(s.frame.view, MainView::Clipboard(_)) {
        // Rows are rebuilt (without motion) when the screen opens again.
        s.clipboard.seen.history = None;
        return changed;
    }
    changed | pull(s)
}

fn pull(s: &mut MainState) -> bool {
    let filter = filter_of(s);
    let settings = s.svc.settings.watch().get();
    let prefs = Prefs {
        recording: settings.clipboard_enabled,
        retention_days: settings.clipboard_retention_days,
        show_secrets: settings.clipboard_show_secrets,
        paste_plain: settings.clipboard_paste_plain,
    };
    let mut changed = false;
    let c = &mut s.clipboard;
    if (c.prefs.recording, c.prefs.retention_days, c.prefs.show_secrets, c.prefs.paste_plain)
        != (prefs.recording, prefs.retention_days, prefs.show_secrets, prefs.paste_plain)
    {
        c.prefs = prefs;
        changed = true;
    }
    c.clips.look.show_secrets = c.prefs.show_secrets;
    c.zone = if s.shell.native() {
        *chrono::Local::now().offset()
    } else {
        c.zone
    };
    let service = s.svc.clipboard.clone();
    let revision = service.watch().revision();
    let now: chrono::DateTime<chrono::Utc> = service.clock().now().into();
    let local = now.with_timezone(&c.zone);
    let day = i64::from(chrono::Datelike::num_days_from_ce(&local.date_naive()));
    let query_text = c.search.text.clone();
    let stale = c.seen.history != Some(revision)
        || c.seen.filter != Some(filter)
        || c.seen.query != query_text
        || c.seen.day != day;
    if !stale {
        return changed;
    }
    let before = c.clips.list.history().clone();
    let first = c.seen.history.is_none();
    let query = ClipQuery::parse(&query_text);
    let results = service.query(filter, &query);
    c.clips.look.terms = query.terms.clone();
    c.clips.look.now = local;
    c.clips
        .list
        .set(results.history.clone(), &results.indices, RowOptions::history(&query_text, filter), local);
    let query_unchanged = c.seen.query == query_text && c.seen.filter == Some(filter);
    if !first && query_unchanged {
        c.clips
            .census
            .observe(&before, &results.history, c.visible, s.frame.clock_secs);
    } else {
        c.clips.census = Default::default();
    }
    c.seen.history = Some(revision);
    c.seen.filter = Some(filter);
    c.seen.query = query_text;
    c.seen.day = day;
    actions::refresh_usage(s, revision);
    true
}

