//! Replacing a typed trigger: deciding how the expansion goes in, deleting the trigger, and
//! putting it back when the insertion is refused.
//!
//! **A refused paste never loses text (D14).** A paste saves and writes the clipboard before the
//! trigger's first Backspace, so when the clipboard can't be saved or written nothing has been
//! deleted. When a Backspace or the ⌘V is refused after that, what was deleted is typed back,
//! with the key an active listener withheld, so the document shows the trigger as typed. (The
//! engine this was copied from deleted first, and a refused paste left neither trigger nor
//! snippet.)
//!
//! **Nothing goes into a changed context.** Preparing can wait (behind a selection copy, on a
//! slow app's promised data), so the insertion re-checks [`Live`] right after it, before each
//! Backspace, before the delivery and before typing anything back. When the context changed it
//! stops there; if part of the trigger is gone by then, the host hears it is lost (D17).

use kudzu_core::matching::TriggerMatch;
use kudzu_core::InsertionMethod;
use unicode_segmentation::UnicodeSegmentation;

use super::super::input_source;
use super::super::keys::{self, KeyError, Press};
use super::super::pasteboard::{PasteError, PreparedPaste};
use super::{Guard, Imp, InsertError};
use crate::plan::{self, InsertionPlan, PlanMode, Step};
use crate::{lock, TriggerLost};

/// What an insertion re-checks before each step that changes the document.
#[derive(Debug, Clone, Copy)]
pub(super) struct Live {
    pub(super) generation: u64,
    pub(super) guard: Guard,
    /// The app in front when the insertion started.
    pub(super) pid: Option<i32>,
    /// Listen-only typing (which the listener can't withhold) stops it: true while what the
    /// typing would land in is about to be deleted.
    pub(super) listen_only: bool,
    /// A submitted form that must still be uninterrupted (FRM-4).
    pub(super) form: Option<u64>,
}

/// The typed trigger an insertion replaces.
#[derive(Clone, Copy)]
pub(super) struct TypedTrigger<'a> {
    pub(super) trigger: &'a TriggerMatch,
    pub(super) mode: PlanMode,
    pub(super) swallowed: Option<&'a str>,
    /// The checks for this insertion.
    pub(super) live: Live,
}

impl TypedTrigger<'_> {
    /// Backspaces that remove the trigger, and the separator when it reached the document.
    fn delete_count(&self) -> usize {
        InsertionPlan::new(self.trigger, String::new(), None, self.mode, self.swallowed)
            .delete_count
    }

    /// What those Backspaces delete, as typed.
    fn visible(&self) -> String {
        let m = self.trigger;
        match (self.mode, self.withheld()) {
            (PlanMode::ListenOnly, _) => {
                format!(
                    "{}{}",
                    m.typed_trigger,
                    m.separator.as_deref().unwrap_or("")
                )
            }
            (PlanMode::Active, _) if m.separator.is_some() => m.typed_trigger.clone(),
            (PlanMode::Active, withheld) => {
                match withheld.and_then(|s| m.typed_trigger.strip_suffix(s)) {
                    Some(visible) => visible.to_string(),
                    None => {
                        let mut graphemes: Vec<&str> = m.typed_trigger.graphemes(true).collect();
                        graphemes.pop();
                        graphemes.concat()
                    }
                }
            }
        }
    }

    /// The key an active listener withheld: the trigger's last key, or a whole-word separator.
    fn withheld(&self) -> Option<&str> {
        self.swallowed
            .filter(|s| self.mode == PlanMode::Active && !s.is_empty())
    }

    /// What to type so the document shows the trigger as typed again, after `deleted` of its
    /// Backspaces went out.
    fn restoration(&self, deleted: usize) -> String {
        let visible = self.visible();
        let graphemes: Vec<&str> = visible.graphemes(true).collect();
        let from = graphemes.len().saturating_sub(deleted);
        format!(
            "{}{}",
            graphemes[from..].concat(),
            self.withheld().unwrap_or("")
        )
    }
}

/// How a piece of text will be inserted, decided (and for a paste, prepared) before anything is
/// deleted.
pub(super) enum Delivery<'a> {
    Nothing,
    Type(&'a str),
    Paste(PreparedPaste<'a>),
}

impl Imp {
    /// Whether the insertion `live` describes may still change the document: the same
    /// attempt, still allowed, the same app in front, no listen-only typing into a deletion
    /// range, and its submitted form not interrupted.
    pub(super) fn still_live(&self, live: &Live) -> bool {
        let unchanged = {
            let core = lock(&self.core);
            core.generation == live.generation
                && !(live.listen_only && core.listen_only_interrupted)
                && live.form.is_none_or(|token| {
                    core.pending_form
                        .as_ref()
                        .is_some_and(|f| f.token == token && f.input.may_delete())
                })
        };
        unchanged && self.allowed(live.guard) && self.observe().frontmost_pid() == live.pid
    }

    /// Posts `presses` one at a time, re-checking `live` before each: `false` when the context
    /// changed and the rest weren't posted.
    pub(super) fn post_checked(&self, live: &Live, presses: &[Press]) -> Result<bool, KeyError> {
        for press in presses {
            if !self.still_live(live) {
                return Ok(false);
            }
            press.post(self.events())?;
        }
        Ok(true)
    }

    /// Delivers text by typing or pasting, unless the context changes while preparing or
    /// between presses: then the rest doesn't go in and the answer is `false`.
    pub(super) fn deliver(
        &self,
        text: &str,
        html: Option<&str>,
        method: InsertionMethod,
        live: &Live,
    ) -> Result<bool, InsertError> {
        let delivery = self.prepare(text, html, method)?;
        if !self.still_live(live) {
            return Ok(false);
        }
        self.complete(delivery, live)
    }

    /// Decides how `text` goes in; a paste saves and writes the clipboard now.
    pub(super) fn prepare<'a>(
        &'a self,
        text: &'a str,
        html: Option<&str>,
        method: InsertionMethod,
    ) -> Result<Delivery<'a>, InsertError> {
        if text.is_empty() {
            Ok(Delivery::Nothing)
        } else if plan::types(method, text, html.is_some(), input_source::composing()) {
            Ok(Delivery::Type(text))
        } else {
            Ok(Delivery::Paste(
                self.pasteboard().prepare_paste(text, html)?,
            ))
        }
    }

    /// Types the text (checking `live` before each chunk; `false` when it stopped partway), or
    /// presses ⌘V and restores the clipboard.
    pub(super) fn complete(
        &self,
        delivery: Delivery<'_>,
        live: &Live,
    ) -> Result<bool, InsertError> {
        match delivery {
            Delivery::Nothing => Ok(true),
            Delivery::Type(text) => Ok(self.post_checked(live, &keys::text_presses(text))?),
            Delivery::Paste(paste) => {
                paste.paste(self.events(), self.observe())?;
                Ok(true)
            }
        }
    }

    /// [`Imp::prepare`] for an insertion that replaces `t`. When the clipboard can't be saved or
    /// written nothing is deleted yet, and the withheld key is typed back.
    pub(super) fn prepare_or_put_back<'a>(
        &'a self,
        t: &TypedTrigger<'_>,
        text: &'a str,
        html: Option<&str>,
        method: InsertionMethod,
    ) -> Result<Delivery<'a>, InsertError> {
        self.prepare(text, html, method).inspect_err(|_| {
            self.put_back(t, 0);
        })
    }

    /// Deletes `t`, checking before each Backspace that the context is unchanged (for a
    /// submitted form, that it wasn't interrupted: the final guard, FRM-4). When it changed,
    /// `prepared` is abandoned (its clipboard comes back) and the answer is `false`; if some of
    /// the trigger was deleted by then, it is lost. When a Backspace is refused partway, what
    /// was deleted is typed back.
    pub(super) fn delete_trigger(
        &self,
        t: &TypedTrigger<'_>,
        prepared: &mut Option<Delivery<'_>>,
    ) -> Result<bool, InsertError> {
        for deleted in 0..t.delete_count() {
            if !self.still_live(&t.live) {
                prepared.take();
                if deleted > 0 {
                    self.lost(t);
                }
                return Ok(false);
            }
            if let Err(error) = keys::backspace_presses(1)[0].post(self.events()) {
                prepared.take();
                self.put_back(t, deleted);
                return Err(error.into());
            }
        }
        Ok(true)
    }

    /// [`Imp::complete`] after `t` was deleted, if the context is unchanged; else nothing goes
    /// in, the trigger is lost and the answer is `false`. A paste whose ⌘V is refused has
    /// already put the clipboard back; the trigger is typed back too, since nothing was
    /// inserted.
    pub(super) fn complete_or_put_back(
        &self,
        t: &TypedTrigger<'_>,
        delivery: Delivery<'_>,
    ) -> Result<bool, InsertError> {
        if !self.still_live(&t.live) {
            drop(delivery);
            self.lost(t);
            return Ok(false);
        }
        let pastes = matches!(delivery, Delivery::Paste(_));
        match self.complete(delivery, &t.live) {
            Ok(true) => Ok(true),
            // Typing stopped partway: the trigger is gone and the expansion incomplete.
            Ok(false) => {
                self.lost(t);
                Ok(false)
            }
            Err(error) => {
                if pastes {
                    self.put_back(t, t.delete_count());
                }
                Err(error)
            }
        }
    }

    /// Prepares the `steps` of a sequence that replaces `t` (its leading pauses already ran):
    /// the first one when it is text (a paste is saved and written now), else nothing. When a
    /// later step pastes but the clipboard can't be saved now, nothing is deleted. A paste
    /// refused after a key press still stops the sequence where it is, as any refused step does
    /// (EXP-15).
    pub(super) fn prepare_sequence<'a>(
        &'a self,
        t: &TypedTrigger<'_>,
        steps: &'a [Step],
        method: InsertionMethod,
    ) -> Result<Option<Delivery<'a>>, InsertError> {
        let first = match steps.first() {
            Some(Step::Text { text, spans }) => {
                let html = spans
                    .as_ref()
                    .map(|spans| kudzu_core::formatting::html(text, spans));
                Some(self.prepare_or_put_back(t, text, html.as_deref(), method)?)
            }
            _ => None,
        };
        let later = &steps[usize::from(first.is_some())..];
        let pastes_later = later.iter().any(|step| {
            matches!(step, Step::Text { text, spans }
                if !plan::types(method, text, spans.is_some(), input_source::composing()))
        });
        let saved = matches!(first, Some(Delivery::Paste(_)));
        if pastes_later && !saved && !self.pasteboard().can_save() {
            drop(first);
            self.put_back(t, 0);
            return Err(PasteError::Unavailable.into());
        }
        Ok(first)
    }

    /// Types back the withheld key and the last `deleted` graphemes of `t` (D14), re-checking
    /// the context before each press: never into another app or at a moved caret. Whether the
    /// document shows the trigger again; when not, the host hears that it is lost.
    pub(super) fn put_back(&self, t: &TypedTrigger<'_>, deleted: usize) -> bool {
        let text = t.restoration(deleted);
        if text.is_empty() {
            return true;
        }
        match self.post_checked(&t.live, &keys::retype_presses(&text)) {
            Ok(true) => true,
            Ok(false) => {
                self.lost(t);
                false
            }
            Err(error) => {
                log::warn!("could not type the trigger back: {error:?}");
                self.lost(t);
                false
            }
        }
    }

    /// Tells the host that `t` is missing from the document (D17).
    pub(super) fn lost(&self, t: &TypedTrigger<'_>) {
        self.host.trigger_lost(TriggerLost {
            snippet_id: t.trigger.snippet_id,
            trigger: t.trigger.trigger.clone(),
        });
    }
}
