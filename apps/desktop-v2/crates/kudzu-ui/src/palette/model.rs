//! The palette's search state: the field, the list, and the preview and copy tasks.
//!
//! The shell window owns the capture session. This struct owns everything the view draws
//! and everything `poll` advances. Nothing here prints a query, a snippet body, or a render.

use std::sync::Arc;

use kudzu_app::expansion::{ExpansionError, InsertionOutcome};
use kudzu_app::ports::engine::AdmissionError;
use kudzu_app::tasks::{Task, TaskError};
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

pub struct SearchModel {
    pub keys: PaletteKeys,
    pub edit: TextEdit,
    pub presentation: Presentation,
    /// A scene's library. While set, `poll` does not replace `presentation`.
    pub frozen: Option<Arc<KudzuLibrary>>,
    pub search: Latest<Presentation>,
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
    /// The capture landed after a preview had already been asked for.
    pub restart_preview: bool,
    pub held: Held,
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
            preview: PreviewClock::default(),
            dim: false,
            shimmer: false,
            rendered: None,
            preview_task: None,
            copy: None,
            copy_due: None,
            pending: None,
            baseline: 0,
            restart_preview: false,
            held: Held::default(),
            insert_task: None,
        }
    }
}

impl SearchModel {
    /// A fresh opening. The field, the list and the preview all start empty.
    pub fn reset(&mut self) {
        let search = std::mem::take(&mut self.search);
        *self = Self {
            search,
            ..Self::default()
        };
    }
}

/// A failure to start a task, logged without its payload.
pub(crate) fn log_start(what: &str, error: TaskError) {
    log::warn!("{what} failed to start: {error}");
}

/// An outcome the palette was waiting on. The variant is a name, never snippet text.
pub(crate) fn log_outcome(outcome: InsertionOutcome) {
    log::debug!("quick search finished: {outcome:?}");
}
