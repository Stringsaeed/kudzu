//! Replacing a typed trigger: deciding how the expansion goes in, deleting the trigger, and
//! putting it back when the insertion is refused.
//!
//! **A refused paste never loses text (D14).** A paste saves and writes the clipboard before the
//! trigger's first Backspace, so when the clipboard can't be saved or written nothing has been
//! deleted. When a Backspace or the ⌘V is refused after that, what was deleted is typed back,
//! with the key an active listener withheld, so the document shows the trigger as typed. (The
//! engine this was copied from deleted first, and a refused paste left neither trigger nor
//! snippet.)

use kudzu_core::matching::TriggerMatch;
use kudzu_core::InsertionMethod;
use unicode_segmentation::UnicodeSegmentation;

use super::super::input_source;
use super::super::keys;
use super::super::pasteboard::{PasteError, PreparedPaste};
use super::{Imp, InsertError};
use crate::plan::{self, InsertionPlan, PlanMode, Sequence, Step};
use crate::TriggerLost;

/// The typed trigger an insertion replaces.
#[derive(Clone, Copy)]
pub(super) struct TypedTrigger<'a> {
    pub(super) trigger: &'a TriggerMatch,
    pub(super) mode: PlanMode,
    pub(super) swallowed: Option<&'a str>,
    /// The submitted form that replaces it, which must still be uninterrupted when the first
    /// Backspace goes out.
    pub(super) form: Option<u64>,
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

    /// Types the text, or presses ⌘V and restores the clipboard.
    pub(super) fn complete(&self, delivery: Delivery<'_>) -> Result<(), InsertError> {
        match delivery {
            Delivery::Nothing => Ok(()),
            Delivery::Type(text) => Ok(keys::type_text(self.events(), text)?),
            Delivery::Paste(paste) => Ok(paste.paste(self.events(), self.observe())?),
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
        self.prepare(text, html, method)
            .inspect_err(|_| self.put_back(t, 0))
    }

    /// Deletes `t`, or returns `false` with nothing deleted when its submitted form was
    /// interrupted (the final guard, FRM-4). When a Backspace is refused partway, `prepared` is
    /// abandoned (its clipboard comes back) and what was deleted is typed back.
    pub(super) fn delete_trigger(
        &self,
        t: &TypedTrigger<'_>,
        prepared: &mut Option<Delivery<'_>>,
    ) -> Result<bool, InsertError> {
        if t.form.is_some_and(|token| !self.take_submitted_form(token)) {
            prepared.take();
            return Ok(false);
        }
        for deleted in 0..t.delete_count() {
            if let Err(error) = keys::delete_backward(self.events(), 1) {
                prepared.take();
                self.put_back(t, deleted);
                return Err(error.into());
            }
        }
        Ok(true)
    }

    /// [`Imp::complete`] after `t` was deleted. A paste whose ⌘V is refused has already put the
    /// clipboard back; the trigger is typed back too, since nothing was inserted.
    pub(super) fn complete_or_put_back(
        &self,
        t: &TypedTrigger<'_>,
        delivery: Delivery<'_>,
    ) -> Result<(), InsertError> {
        let pastes = matches!(delivery, Delivery::Paste(_));
        self.complete(delivery).inspect_err(|_| {
            if pastes {
                self.put_back(t, t.delete_count());
            }
        })
    }

    /// Prepares a sequence that replaces `t`: its first step when it is text (a paste is saved
    /// and written now), else nothing. When a later step pastes but the clipboard can't be
    /// saved now, nothing is deleted. A paste refused later still stops the sequence where it
    /// is, as any refused step does (EXP-15).
    pub(super) fn prepare_sequence<'a>(
        &'a self,
        t: &TypedTrigger<'_>,
        sequence: &'a Sequence,
        method: InsertionMethod,
    ) -> Result<Option<Delivery<'a>>, InsertError> {
        let first = match sequence.steps.first() {
            Some(Step::Text { text, spans }) => {
                let html = spans
                    .as_ref()
                    .map(|spans| kudzu_core::formatting::html(text, spans));
                Some(self.prepare_or_put_back(t, text, html.as_deref(), method)?)
            }
            _ => None,
        };
        let later = &sequence.steps[usize::from(first.is_some())..];
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

    /// Types back the withheld key and the last `deleted` graphemes of `t` (D14). When that is
    /// refused too, the host hears that the trigger is lost.
    fn put_back(&self, t: &TypedTrigger<'_>, deleted: usize) {
        let text = t.restoration(deleted);
        if text.is_empty() {
            return;
        }
        if let Err(error) = keys::retype(self.events(), &text) {
            log::warn!("could not type the trigger back: {error:?}");
            self.host.trigger_lost(TriggerLost {
                snippet_id: t.trigger.snippet_id,
                trigger: t.trigger.trigger.clone(),
            });
        }
    }
}
