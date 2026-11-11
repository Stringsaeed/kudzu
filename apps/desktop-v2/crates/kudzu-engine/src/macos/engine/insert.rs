//! Rendering and delivering an expansion that replaces a typed trigger: a single delivery, or a
//! sequence of text, key presses and pauses. How the trigger itself is deleted, and put back
//! when a paste is refused (D14), is in `replace`.

use std::collections::HashMap;
use std::time::Duration;

use kudzu_core::matching::case_propagation;
use kudzu_core::template::RenderResult;
use kudzu_core::{InsertionMethod, SnippetRecord};

use super::super::keys;
use super::replace::{Live, TypedTrigger};
use super::{Imp, InsertError, KEY_SETTLE, PAUSE_SLICE};
use crate::library::{self, Library};
use crate::plan::{self, InsertionPlan, Sequence, Step};
use crate::{lock, ExpansionEvent};

impl Imp {
    pub(super) fn render(
        &self,
        library: &Library,
        snippet: &SnippetRecord,
        values: HashMap<String, String>,
        selection: Option<String>,
    ) -> RenderResult {
        // Read before any paste of ours replaces the clipboard, and only when the template needs it.
        let clipboard = if library.uses_clipboard(snippet) {
            self.pasteboard().read_string()
        } else {
            None
        };
        let history =
            |index: usize, current: Option<&str>| self.host.clipboard_history(index, current);
        library.render(
            snippet,
            library::RenderInputs {
                now: library::now(),
                clipboard,
                selection,
                form_values: values,
                history: &history,
            },
        )
    }

    /// Lets the step that starts now hold typing for the usual limit plus `extra` (a `{{delay}}`).
    /// Without this a sequence of several pastes or pauses would hit the hold deadline midway.
    fn keep_holding(&self, token: u64, extra: Duration) {
        let mut core = lock(&self.core);
        if core.generation == token && core.hold_since.is_some() {
            core.hold_since = Some(self.observe().now());
            core.hold_extra = extra;
        }
    }

    /// Inserts an expansion that has `{{key}}` or `{{delay}}` steps: text, key presses and pauses in
    /// template order. The rules (see docs/TEMPLATE_SYNTAX.md, "Key presses and pauses"):
    /// - every text step is typed or pasted by the same rule as a whole expansion, and a paste's
    ///   clipboard is restored before the next step;
    /// - the sequence stops, leaving what was already inserted, as soon as the user clicks, the
    ///   frontmost app changes, or expansion is paused or blocked;
    /// - with key presses there is no Backspace undo and a whole-word trigger's separator isn't
    ///   typed again, because the text may be spread over several fields by then.
    pub(super) fn insert_sequence(
        &self,
        live: Live,
        typed: Option<TypedTrigger<'_>>,
        method: InsertionMethod,
        sequence: Sequence,
    ) -> Result<bool, InsertError> {
        let token = live.generation;
        // D14: a first step that pastes is prepared before the trigger is deleted.
        let mut first = None;
        if let Some(t) = &typed {
            first = self.prepare_sequence(t, &sequence, method)?;
            if !self.delete_trigger(t, &mut first)? {
                return Ok(false);
            }
        }
        // Once the trigger is gone, each step checks the attempt, the guard and the app in front.
        let step = Live {
            listen_only: false,
            form: None,
            ..live
        };
        let stop = |imp: &Imp| {
            lock(&imp.core).reset_matcher();
            Ok(false)
        };
        for item in &sequence.steps {
            if !self.still_live(&step) {
                return stop(self);
            }
            match item {
                Step::Text { text, spans } => {
                    self.keep_holding(token, Duration::ZERO);
                    let delivered = match (first.take(), &typed) {
                        (Some(delivery), Some(t)) => self.complete_or_put_back(t, delivery)?,
                        _ => {
                            let html = spans
                                .as_ref()
                                .map(|spans| kudzu_core::formatting::html(text, spans));
                            self.deliver(text, html.as_deref(), method, &step)?
                        }
                    };
                    if !delivered {
                        return stop(self);
                    }
                }
                Step::Key { key, count } => {
                    self.keep_holding(token, Duration::ZERO);
                    keys::press_key(self.events(), *key, *count)?;
                    self.observe().sleep(KEY_SETTLE);
                }
                Step::Pause(duration) => {
                    self.keep_holding(token, *duration);
                    let until = self.observe().now() + *duration;
                    while let Some(left) = until
                        .checked_duration_since(self.observe().now())
                        .filter(|d| !d.is_zero())
                    {
                        self.observe().sleep(left.min(PAUSE_SLICE));
                        if !self.is_current(token) {
                            break;
                        }
                    }
                }
            }
        }
        self.keep_holding(token, Duration::ZERO);
        if !self.still_live(&step) {
            return stop(self);
        }
        // An image's delivered length depends on the app, so the caret isn't moved by a guess.
        let mut caret_left = if sequence.has_images {
            0
        } else {
            sequence.caret_left
        };
        let separator = typed
            .as_ref()
            .filter(|_| !sequence.has_keys)
            .and_then(|t| t.trigger.separator.as_deref());
        if let Some(separator) = separator {
            keys::retype(self.events(), separator)?;
            if caret_left > 0 {
                caret_left += plan::grapheme_count(separator);
            }
        }
        keys::move_left(self.events(), caret_left)?;
        let mut core = lock(&self.core);
        if core.generation == token {
            match &typed {
                // Only pauses: the text is in one piece, so Backspace can still undo it.
                Some(t) if !sequence.has_keys && !sequence.has_images => {
                    let inserted: usize = sequence
                        .steps
                        .iter()
                        .map(|s| {
                            if let Step::Text { text, .. } = s {
                                plan::grapheme_count(text)
                            } else {
                                0
                            }
                        })
                        .sum();
                    core.matcher.did_expand(t.trigger, inserted);
                    core.undo_caret_left = caret_left;
                }
                _ => core.reset_matcher(),
            }
        }
        Ok(true)
    }

    /// Replaces the typed trigger `typed` with `rendered`.
    pub(super) fn insert_expansion(
        &self,
        typed: TypedTrigger<'_>,
        rendered: RenderResult,
    ) -> Result<(), InsertError> {
        let TypedTrigger {
            trigger,
            mode,
            swallowed,
            live,
        } = typed;
        let (token, guard) = (live.generation, live.guard);
        let text = case_propagation::apply(trigger.case_style, &rendered.text);
        let offset = rendered
            .cursor_offset
            .map(|o| plan::cased_cursor_offset(trigger.case_style, &rendered.text, o));
        let spans = rendered
            .formatting
            .as_ref()
            .map(|spans| plan::cased_formatting(trigger.case_style, &rendered.text, spans));
        if !rendered.actions.is_empty() {
            let actions =
                plan::cased_actions(trigger.case_style, &rendered.text, &rendered.actions);
            let sequence = Sequence::new(&text, spans.as_deref(), &actions, offset);
            if self.insert_sequence(live, Some(typed), trigger.insertion, sequence)? {
                self.host.expanded(ExpansionEvent {
                    snippet_id: trigger.snippet_id,
                    chars_saved: plan::chars_saved(&text, &trigger.typed_trigger),
                });
            }
            return Ok(());
        }
        let html = spans
            .as_ref()
            .map(|spans| kudzu_core::formatting::html(&text, spans));
        let has_images = trigger.insertion != InsertionMethod::Type
            && spans.as_ref().is_some_and(|s| {
                s.iter()
                    .any(|s| matches!(s.style, kudzu_core::formatting::FormatStyle::Image { .. }))
            });
        let mut plan = InsertionPlan::new(trigger, text, offset, mode, swallowed);
        let caret = {
            let delivery =
                self.prepare_or_put_back(&typed, &plan.text, html.as_deref(), trigger.insertion)?;
            let mut prepared = Some(delivery);
            if !self.delete_trigger(&typed, &mut prepared)? {
                return Ok(());
            }
            let caret = has_images
                .then(|| self.observe().caret_snapshot())
                .flatten();
            if let Some(delivery) = prepared.take() {
                if !self.complete_or_put_back(&typed, delivery)? {
                    return Ok(());
                }
            }
            caret
        };
        let mut can_undo = true;
        if has_images {
            if let Some((text, cursor)) = plan::image_delivery(
                &plan.text,
                spans.as_deref().unwrap_or_default(),
                offset,
                caret.and_then(|c| c.inserted_utf16()),
            ) {
                plan = InsertionPlan::new(trigger, text, cursor, mode, swallowed);
            } else {
                plan.caret_left_count = 0;
                can_undo = false;
            }
        }
        if !self.is_current(token) || !self.allowed(guard) {
            return Ok(());
        }
        if let Some(separator) = &plan.reemit_separator {
            keys::retype(self.events(), separator)?;
        }
        keys::move_left(self.events(), plan.caret_left_count)?;
        {
            let mut core = lock(&self.core);
            if core.generation == token && can_undo {
                // The undo tracker accounts for the re-emitted separator itself.
                core.matcher
                    .did_expand(trigger, plan::grapheme_count(&plan.text));
                core.undo_caret_left = plan.caret_left_count;
            } else if core.generation == token {
                core.reset_matcher();
            }
        }
        self.host.expanded(ExpansionEvent {
            snippet_id: trigger.snippet_id,
            chars_saved: plan::chars_saved(&plan.text, &trigger.typed_trigger),
        });
        Ok(())
    }
}
