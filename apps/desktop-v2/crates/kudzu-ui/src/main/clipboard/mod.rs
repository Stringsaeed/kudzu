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

use crate::clip::content::loader::{FullText, Media, Want};
use crate::clip::list::{Hooks, ListState};
use crate::clip::model::Click;
use crate::main::nav;
use crate::main::state::{MainPane, MainView};
use crate::shell::MainState;

pub use keys::handle as handle_key;
pub use view::view;
#[cfg(test)]
pub(crate) use view::VIEWS;

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
    /// The colour value just copied from the preview, and when its check ends.
    pub copied_value: Option<(String, f64)>,
    /// The raw-formats list under the details is open.
    pub formats_open: bool,
    /// The complete text of the active clip, loaded off the UI thread.
    pub full: FullText,
    /// Height of the content card, for the large preview.
    pub card_h: f32,
    pub(crate) jobs: Vec<actions::Job>,
    seen: Seen,
    visible: bool,
}

#[derive(Default)]
struct Seen {
    active: Option<(Uuid, String)>,
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
            card_h: 702.0,
            copied_value: None,
            formats_open: false,
            full: FullText::default(),
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

    /// `full` belongs to this clip, not the one shown before it.
    pub(crate) fn seen_active_is(&self, id: Uuid) -> bool {
        self.seen.active.as_ref().is_some_and(|(a, _)| *a == id)
    }

    pub fn has_query(&self) -> bool {
        !self.search.text.trim().is_empty()
    }
}

pub(crate) fn list_hooks() -> Hooks<MainState> {
    Hooks {
        state: |s: &MainState| &s.clipboard.clips,
        visible: want_rows,
        press: |s: &mut MainState, id: Uuid, click: Click, tx: &mut Tx| {
            s.clipboard.clips.list.click(id, click);
            s.clipboard.large_preview = false;
            // A row press takes keyboard focus from the search field, as clicking a list does.
            nav::focus_list_pane(s, LIST_KEY, tx);
            track_active(s);
        },
        pick: |s: &mut MainState, id: Uuid, tx: &mut Tx| {
            s.clipboard.clips.list.activate(id);
            actions::copy_active(s, false, tx);
        },
    }
}

/// The waker the image workers and text loads use.
fn waker(s: &MainState) -> std::sync::Arc<dyn Fn() + Send + Sync> {
    let handle = s.shell.handles().main.clone();
    std::sync::Arc::new(move || handle.wake())
}

fn media_mut(s: &mut MainState) -> &mut Media {
    if s.clipboard.clips.media.is_none() {
        let wake = waker(s);
        s.clipboard.clips.media = Some(Media::new(wake));
    }
    s.clipboard
        .clips
        .media
        .get_or_insert_with(|| unreachable!("created above"))
}

/// Starts the pictures the rows in view need: thumbnails and file icons. Bookkeeping only, so
/// it never changes what a scroll draws; a picture that arrives redraws once.
fn want_rows(s: &mut MainState, range: std::ops::Range<usize>) {
    use crate::clip::model::RowKind;
    use kudzu_core::clipboard::ClipKind;
    let wanted: Vec<(Uuid, Option<Want>)> = s
        .clipboard
        .clips
        .list
        .rows()
        .get(range)
        .unwrap_or_default()
        .iter()
        .filter_map(|r| match &r.kind {
            RowKind::Entry { id, index, .. } => s
                .clipboard
                .clips
                .list
                .history()
                .entries
                .get(*index)
                .map(|e| {
                    let want = match e.kind {
                        ClipKind::Image => Some(Want::Thumbnail),
                        ClipKind::File if e.image.is_some() => Some(Want::Thumbnail),
                        ClipKind::File => Some(Want::Icon(0)),
                        _ => None,
                    };
                    (*id, want)
                }),
            RowKind::Header { .. } => None,
        })
        .collect();
    let clipboard = s.svc.clipboard.clone();
    let media = media_mut(s);
    for (id, want) in wanted {
        if let Some(want) = want {
            media.want(&clipboard, id, want);
        }
    }
}

/// Starts the icons of the apps clips came from, for the row badges. The few distinct sources
/// load when the history changes, so scrolling never waits on one.
fn want_app_icons(s: &mut MainState) {
    let mut bundles: Vec<String> = Vec::new();
    for e in &s.clipboard.clips.list.history().entries {
        if let Some(src) = &e.source {
            if !bundles.contains(&src.bundle_id) {
                bundles.push(src.bundle_id.clone());
            }
        }
        if bundles.len() >= 48 {
            break;
        }
    }
    let apps = s.svc.apps.clone();
    let media = media_mut(s);
    for bundle in bundles {
        media.want_app(&apps, &bundle);
    }
}

/// Points the preview at the active clip: loads its full text and the pictures it shows.
/// Cheap when the active clip did not change.
pub(crate) fn track_active(s: &mut MainState) -> bool {
    use kudzu_core::clipboard::ClipKind;
    let Some(e) = s.clipboard.clips.list.active_entry().cloned() else {
        let had = s.clipboard.seen.active.take().is_some();
        s.clipboard.full = FullText::default();
        // The large preview is only in the tree while a clip is active.
        s.clipboard.large_preview = false;
        return had;
    };
    let key = (e.id, e.hash.clone());
    if s.clipboard.seen.active.as_ref() == Some(&key) {
        return false;
    }
    s.clipboard.seen.active = Some(key);
    s.clipboard.formats_open = false;
    let (clipboard, apps, wake) = (s.svc.clipboard.clone(), s.svc.apps.clone(), waker(s));
    s.clipboard.full.show(&clipboard, &wake, &e);
    let media = media_mut(s);
    match e.kind {
        ClipKind::Image => {
            media.want(&clipboard, e.id, Want::Image);
            media.want(&clipboard, e.id, Want::Thumbnail);
        }
        ClipKind::File => {
            for i in 0..e.files.len().min(crate::clip::content::FILE_LIMIT) {
                media.want(&clipboard, e.id, Want::Icon(i));
            }
            if e.image.is_some() && e.files.len() == 1 {
                media.want(&clipboard, e.id, Want::File(0));
                media.want(&clipboard, e.id, Want::Thumbnail);
            }
        }
        _ => {}
    }
    if let Some(src) = &e.source {
        media.want_app(&apps, &src.bundle_id);
    }
    true
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
    if s.clipboard
        .copied_value
        .as_ref()
        .is_some_and(|(_, until)| now_secs >= *until)
    {
        s.clipboard.copied_value = None;
        changed = true;
    }
    changed |= s.clipboard.full.poll();
    changed |= s.clipboard.clips.media.as_mut().is_some_and(Media::poll);
    if !matches!(s.frame.view, MainView::Clipboard(_)) {
        // Rows are rebuilt (without motion) when the screen opens again.
        s.clipboard.seen.history = None;
        return changed;
    }
    changed |= pull(s);
    changed | track_active(s)
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
    if (
        c.prefs.recording,
        c.prefs.retention_days,
        c.prefs.show_secrets,
        c.prefs.paste_plain,
    ) != (
        prefs.recording,
        prefs.retention_days,
        prefs.show_secrets,
        prefs.paste_plain,
    ) {
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
    c.clips.list.set(
        results.history.clone(),
        &results.indices,
        RowOptions::history(&query_text, filter),
        local,
    );
    let query_unchanged = c.seen.query == query_text && c.seen.filter == Some(filter);
    if !first && query_unchanged {
        c.clips
            .census
            .observe(&before, &results.history, c.visible, s.frame.clock_secs);
    } else {
        c.clips.census.load();
    }
    c.seen.history = Some(revision);
    c.seen.filter = Some(filter);
    c.seen.query = query_text;
    c.seen.day = day;
    actions::refresh_usage(s, revision);
    want_app_icons(s);
    true
}
