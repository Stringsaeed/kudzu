//! The palette's search state: the field, the list, and the preview and copy tasks.
//!
//! The shell window owns the capture session. This struct owns everything the view draws
//! and everything `poll` advances. Nothing here prints a query, a snippet body, or a render.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use kudzu_app::expansion::ExpansionError;
use kudzu_app::ports::engine::AdmissionError;
use kudzu_app::ports::Clock;
use kudzu_app::tasks::{Task, TaskError};
use kudzu_core::search::PaletteCache;
use kudzu_core::template::RenderResult;
use kudzu_core::KudzuLibrary;
use rune::{Latest, TextEdit};
use uuid::Uuid;

use super::keys::PaletteKeys;
use super::preview::PreviewClock;
use super::rank::Presentation;
use crate::widgets::key_hints::Held;

pub(crate) struct RenderInflight {
    pub id: Uuid,
    pub task: Task<Result<RenderResult, ExpansionError>>,
}

pub(crate) enum CopyJob {
    /// Rendering the snippet before it is copied.
    Render(RenderInflight),
    /// The pasteboard write.
    Write(Task<Result<(), ExpansionError>>),
}

/// Enter or ⌘1–9 pressed before the list that answers the field has landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeldList {
    Enter {
        copy: bool,
    },
    /// 1 through 9.
    Digit(u8),
}

pub struct SearchModel {
    pub keys: PaletteKeys,
    pub edit: TextEdit,
    pub presentation: Presentation,
    /// A scene's library. While set, `poll` does not replace `presentation`.
    pub frozen: Option<Arc<KudzuLibrary>>,
    pub search: Latest<Presentation>,
    /// Folded index shared across keystrokes and reopenings of this panel.
    pub cache: Arc<PaletteCache>,
    pub preview: PreviewClock,
    pub dim: bool,
    pub shimmer: bool,
    /// The render on screen. Its text is drawn and never logged.
    pub rendered: Option<(Uuid, RenderResult)>,
    pub(crate) preview_task: Option<RenderInflight>,
    pub(crate) copy: Option<CopyJob>,
    /// Frame-clock time at which a finished copy hides the palette.
    pub copy_due: Option<f64>,
    pub pending: Option<Uuid>,
    pub baseline: u64,
    /// Chosen before the capture session existed. `sync` runs it once the session lands.
    pub(crate) pending_pick: Option<Uuid>,
    /// Wall time after which an insert no longer blocks another opening.
    /// Matches `expansion::hold::OUTCOME_DEADLINE` (crate-private, 10s).
    pub(crate) insert_until: Option<SystemTime>,
    /// The capture landed after a preview had already been asked for.
    pub restart_preview: bool,
    pub held: Held,
    /// Enter or a digit waiting on the list that answers the current field.
    pub(crate) held_list: Option<HeldList>,
    pub(crate) insert_task: Option<Task<Result<(), AdmissionError>>>,
}

impl Default for SearchModel {
    fn default() -> Self {
        Self {
            keys: PaletteKeys::default(),
            edit: TextEdit::new(""),
            presentation: Presentation::default(),
            frozen: None,
            search: Latest::default(),
            cache: Arc::new(PaletteCache::default()),
            preview: PreviewClock::default(),
            dim: false,
            shimmer: false,
            rendered: None,
            preview_task: None,
            copy: None,
            copy_due: None,
            pending: None,
            baseline: 0,
            pending_pick: None,
            insert_until: None,
            restart_preview: false,
            held: Held::default(),
            held_list: None,
            insert_task: None,
        }
    }
}

impl SearchModel {
    /// A fresh opening. The field, the list and the preview start empty.
    ///
    /// Cancelling the search forgets the last `(revision, text, scope)` key, so the same
    /// empty query asks again. An insert that is still inside its hold keeps its pending
    /// id, baseline, task and deadline: a new opening must not drop them.
    pub fn reset(&mut self, now: SystemTime) {
        self.search.cancel();
        let search = std::mem::take(&mut self.search);
        let cache = Arc::clone(&self.cache);
        let (pending, baseline, insert_task, insert_until) = if self.insert_in_flight(now) {
            (
                self.pending.take(),
                self.baseline,
                self.insert_task.take(),
                self.insert_until.take(),
            )
        } else {
            (None, 0, None, None)
        };
        *self = Self {
            search,
            cache,
            pending,
            baseline,
            insert_task,
            insert_until,
            ..Self::default()
        };
    }

    /// An admitted insert whose outcome window has not closed.
    pub(crate) fn insert_in_flight(&self, now: SystemTime) -> bool {
        let live = self.pending.is_some() || self.insert_task.is_some();
        live && self.insert_until.is_some_and(|until| now < until)
    }
}

/// Wall time from the injected clock, in UTC. The palette never reads the host zone.
pub(crate) fn utc_from(clock: &dyn Clock) -> DateTime<Utc> {
    let ms = clock
        .now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::UNIX_EPOCH)
}

/// A failure to start a task, logged without its payload.
pub(crate) fn log_start(what: &str, error: TaskError) {
    log::warn!("{what} failed to start: {error}");
}
