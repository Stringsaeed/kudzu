//! Forms: a typed trigger's form from the request to the submission, and the panel's focus.

use std::collections::HashMap;

use kudzu_core::matching::TriggerMatch;
use kudzu_core::SnippetRecord;
use uuid::Uuid;

use super::super::keys::{self, SYNTHETIC_MARKER};
use super::super::tap::HeldEvent;
use super::replace::{Live, TypedTrigger};
use super::{activity, Guard, Imp, InsertError, ACTIVATION_TIMEOUT, SETTLE};
use crate::interruption::Interruption;
use crate::library;
use crate::plan::PlanMode;
use crate::{lock, AppInfo, FormRequest, PanelFocus, Submission};

#[derive(Debug, Clone)]
pub(super) enum FormKind {
    /// Typed trigger, still in the document (as typed, with any separator).
    Typed(TriggerMatch),
    /// Palette insert: nothing to delete.
    Palette,
}

#[derive(Debug, Clone)]
pub(super) struct PendingForm {
    pub(super) token: u64,
    pub(super) snippet_id: Uuid,
    pub(super) kind: FormKind,
    pub(super) target: Option<AppInfo>,
    /// What reached the session since the form was requested, and where the panel's focus is:
    /// whether deleting a typed trigger now could delete the wrong text (FRM-4).
    pub(super) input: Interruption,
    /// Selection captured before a palette insert (never set for a typed trigger).
    pub(super) selection: Option<String>,
}

impl Imp {
    /// The form panel for `token` became key (`true`) or stopped being key (`false`): where
    /// key-downs go from now on (see `crate::interruption`).
    pub fn form_panel_focused(&self, token: u64, focus: PanelFocus) {
        let mut core = lock(&self.core);
        if let Some(form) = core.pending_form.as_mut().filter(|f| f.token == token) {
            form.input.focus(focus);
        }
    }

    /// Marks the pending form `token` submitted by the event stamped `at`, once. Whether it was
    /// accepted (a stale or repeated token isn't).
    pub(super) fn accept_submission(&self, token: u64, submission: Submission) -> bool {
        let mut core = lock(&self.core);
        match core
            .pending_form
            .as_mut()
            .filter(|f| f.token == token && !f.input.submitted())
        {
            Some(form) => {
                form.input.submit(submission);
                true
            }
            None => false,
        }
    }

    pub fn cancel_form(&self, token: u64) {
        {
            let mut core = lock(&self.core);
            if core.pending_form.as_ref().is_some_and(|f| f.token == token) {
                core.pending_form = None;
            }
        }
        self.recorder.form_cancelled(token);
    }

    pub(super) fn request_typed_form(
        &self,
        token: u64,
        trigger: TriggerMatch,
        snippet: &SnippetRecord,
        fields: Vec<kudzu_core::template::FormFieldSpec>,
        mode: PlanMode,
        swallowed: Option<String>,
    ) -> Result<(), InsertError> {
        // Put the withheld key back so the document shows the whole trigger (and separator);
        // the form submission then deletes it like listen-only would.
        if mode == PlanMode::Active {
            if let Some(text) = &swallowed {
                keys::retype(self.events(), text)?;
            }
        }
        let queued = {
            let mut core = lock(&self.core);
            core.preparing_form = true;
            core.preparing_interrupted = false;
            core.hold_since = None;
            core.reset_matcher();
            core.held.drain()
        };
        let interrupted = queued.iter().any(HeldEvent::is_key_down);
        for held in queued {
            self.events().post_held(held, SYNTHETIC_MARKER);
        }
        self.observe().sleep(SETTLE);
        let target = self.observe().frontmost_app();
        let form_token = {
            let mut core = lock(&self.core);
            core.preparing_form = false;
            let interrupted = interrupted || core.preparing_interrupted;
            // Typing already reached the document after the trigger, so deleting it would be
            // wrong. Leave the trigger rather than show a form we'd discard.
            if core.generation != token || interrupted {
                return Ok(());
            }
            let form_token = core.next_form_token;
            core.next_form_token += 1;
            core.pending_form = Some(PendingForm {
                token: form_token,
                snippet_id: snippet.id,
                kind: FormKind::Typed(trigger),
                target: target.clone(),
                input: Interruption::default(),
                selection: None,
            });
            form_token
        };
        if !self.allowed(Guard::Typed) {
            lock(&self.core).pending_form = None;
            return Ok(());
        }
        self.host.form_requested(FormRequest {
            token: form_token,
            snippet_id: snippet.id,
            snippet_name: snippet.name.clone(),
            fields,
            caret: self.observe().caret_rect(),
            target_app: target,
            selection: None,
        });
        Ok(())
    }

    /// Submits the pending form `token`, once. A typed form stays pending through the target's
    /// activation and settling, still watching for input that interrupts it, until the final
    /// guard before its first Backspace takes it (FRM-4).
    pub(super) fn submit(&self, token: u64, values: HashMap<String, String>) {
        let Some(form) = self.begin_submission(token) else {
            return;
        };
        self.submit_now(&form, values);
        let mut core = lock(&self.core);
        if core.pending_form.as_ref().is_some_and(|f| f.token == token) {
            core.pending_form = None;
        }
    }

    /// The accepted form `token`. A palette form has nothing to delete, so it stops being
    /// pending now.
    fn begin_submission(&self, token: u64) -> Option<PendingForm> {
        let mut core = lock(&self.core);
        let pending = core
            .pending_form
            .as_ref()
            .filter(|f| f.token == token && f.input.submitted())?;
        let form = pending.clone();
        if matches!(form.kind, FormKind::Palette) {
            core.pending_form = None;
        }
        Some(form)
    }

    fn submit_now(&self, form: &PendingForm, values: HashMap<String, String>) {
        if form.input.interrupted() && matches!(form.kind, FormKind::Typed(_)) {
            return;
        }
        if !self.explicit_allowed() {
            return;
        }
        let target = form
            .target
            .clone()
            .or_else(|| self.observe().frontmost_app());
        if let Some(app) = &target {
            if !self.observe().activate_and_wait(app, ACTIVATION_TIMEOUT) {
                log::warn!("form target {} did not come to the front", app.bundle_id);
                self.recorder.resolve(activity::target_gone());
                return;
            }
        }
        let generation = self.begin_explicit();
        self.observe().sleep(SETTLE);
        let result = self.insert_submitted(generation, form, target.as_ref(), values);
        if let Err(error) = result {
            log::warn!("form insertion failed: {error}");
            self.recorder.resolve(activity::failed(&error));
        }
        self.finish(generation);
    }

    fn insert_submitted(
        &self,
        generation: u64,
        form: &PendingForm,
        target: Option<&AppInfo>,
        values: HashMap<String, String>,
    ) -> Result<(), InsertError> {
        // Bound to the target now, before rendering waits on anything.
        let Some(pid) = self
            .front_target(target)
            .filter(|_| self.is_current(generation) && self.explicit_allowed())
        else {
            return Ok(());
        };
        let library = self.library();
        let Some(snippet) = library.snippets.get(&form.snippet_id) else {
            return Ok(());
        };
        let values = library::with_defaults(&library.form_fields(snippet), values);
        let rendered = self.render(&library, snippet, values, form.selection.clone());
        match &form.kind {
            // The trigger and any separator are in the document as typed.
            FormKind::Typed(trigger) => {
                let typed = TypedTrigger {
                    trigger,
                    mode: PlanMode::ListenOnly,
                    swallowed: None,
                    live: Live {
                        listen_only: true,
                        form: Some(form.token),
                        ..self.explicit_live(generation, pid)
                    },
                };
                self.insert_expansion(typed, rendered)
            }
            FormKind::Palette => {
                let live = self.explicit_live(generation, pid);
                self.insert_rendered(live, &library, snippet, rendered, target)
            }
        }
    }
}
