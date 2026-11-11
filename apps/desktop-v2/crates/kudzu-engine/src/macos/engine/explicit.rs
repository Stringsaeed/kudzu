//! Insertions the user asked for (Quick Search), which have nothing to delete.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use kudzu_core::template::RenderResult;
use kudzu_core::{InsertionMethod, SnippetRecord};
use uuid::Uuid;

use super::super::keys;
use super::form_flow::{FormKind, PendingForm};
use super::replace::Live;
use super::session::decode;
use super::{activity, Guard, Imp, InsertError, ACTIVATION_TIMEOUT, SETTLE};
use crate::interruption::Interruption;
use crate::library::Library;
use crate::plan::{self, Sequence};
use crate::{lock, AppInfo, ExpansionEvent, FormRequest, TapMode};

impl Imp {
    /// Starts an explicit insertion: holds typing (active tap) and returns its generation.
    pub(super) fn begin_explicit(&self) -> u64 {
        let active = decode(self.flags.tap_mode.load(Ordering::SeqCst)) == TapMode::Active;
        let now = self.observe().now();
        let mut core = lock(&self.core);
        core.generation += 1;
        core.busy = true;
        core.listen_only_interrupted = false;
        core.reset_matcher();
        if active {
            core.start_hold(now);
        }
        core.generation
    }

    pub(super) fn target_is_front(&self, target: Option<&AppInfo>) -> bool {
        match target {
            Some(app) => self
                .observe()
                .resolve_pid(app)
                .is_some_and(|pid| self.observe().frontmost_pid() == Some(pid)),
            None => true,
        }
    }

    pub(super) fn palette_insert(
        &self,
        snippet_id: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) {
        if !self.explicit_allowed() {
            return;
        }
        let library = self.library();
        let Some(snippet) = library.snippets.get(&snippet_id) else {
            return;
        };
        let target = target.or_else(|| self.observe().frontmost_app());
        // Global exclusions (password managers, login window) apply to explicit inserts too.
        if self.note_excluded(&library, target.as_ref().map(|t| t.bundle_id.as_str())) {
            return;
        }
        self.invalidate();
        let fields = library.form_fields(snippet);
        if !fields.is_empty() {
            let target_front = target
                .as_ref()
                .is_some_and(|t| self.target_is_front(Some(t)));
            let form_token = {
                let mut core = lock(&self.core);
                let form_token = core.next_form_token;
                core.next_form_token += 1;
                core.pending_form = Some(PendingForm {
                    token: form_token,
                    snippet_id,
                    kind: FormKind::Palette,
                    target: target.clone(),
                    input: Interruption::default(),
                    selection: selection.clone(),
                });
                form_token
            };
            self.host.form_requested(FormRequest {
                token: form_token,
                snippet_id,
                snippet_name: snippet.name.clone(),
                fields,
                caret: if target_front {
                    self.observe().caret_rect()
                } else {
                    None
                },
                target_app: target,
                selection,
            });
            return;
        }
        if let Some(app) = &target {
            if !self.observe().activate_and_wait(app, ACTIVATION_TIMEOUT) {
                log::warn!("palette target {} did not come to the front", app.bundle_id);
                self.recorder.resolve(activity::target_gone());
                return;
            }
        }
        let generation = self.begin_explicit();
        self.observe().sleep(SETTLE);
        let result = if self.is_current(generation)
            && self.explicit_allowed()
            && self.target_is_front(target.as_ref())
        {
            let rendered = self.render(&library, snippet, HashMap::new(), selection);
            self.insert_rendered(generation, &library, snippet, rendered, target.as_ref())
        } else {
            Ok(())
        };
        if let Err(error) = result {
            log::warn!("palette insertion failed: {error}");
            self.recorder.resolve(activity::failed(&error));
        }
        self.finish(generation);
    }

    /// Inserts a rendered snippet at the caret without deleting anything (palette).
    pub(super) fn insert_rendered(
        &self,
        generation: u64,
        library: &Library,
        snippet: &SnippetRecord,
        rendered: RenderResult,
        target: Option<&AppInfo>,
    ) -> Result<(), InsertError> {
        let live = Live {
            generation,
            guard: Guard::Explicit,
            pid: self.observe().frontmost_pid(),
            listen_only: false,
            form: None,
        };
        let method = library.insertion_for(
            snippet,
            target
                .map(|t| t.bundle_id.as_str())
                .filter(|b| !b.is_empty()),
        );
        if !rendered.actions.is_empty() {
            let sequence = Sequence::new(
                &rendered.text,
                rendered.formatting.as_deref(),
                &rendered.actions,
                rendered.cursor_offset,
            );
            if self.insert_sequence(live, None, method, sequence)? {
                self.host.expanded(ExpansionEvent {
                    snippet_id: snippet.id,
                    chars_saved: plan::chars_saved(&rendered.text, ""),
                });
            }
            return Ok(());
        }
        let html = rendered
            .formatting
            .as_ref()
            .map(|spans| kudzu_core::formatting::html(&rendered.text, spans));
        let has_images = method != InsertionMethod::Type
            && rendered.formatting.as_ref().is_some_and(|s| {
                s.iter()
                    .any(|s| matches!(s.style, kudzu_core::formatting::FormatStyle::Image { .. }))
            });
        let caret = has_images
            .then(|| self.observe().caret_snapshot())
            .flatten();
        if !self.deliver(&rendered.text, html.as_deref(), method, &live)? {
            return Ok(());
        }
        if !self.is_current(generation) || !self.explicit_allowed() {
            return Ok(());
        }
        let delivered = if has_images {
            plan::image_delivery(
                &rendered.text,
                rendered.formatting.as_deref().unwrap_or_default(),
                rendered.cursor_offset,
                caret.and_then(|c| c.inserted_utf16()),
            )
        } else {
            Some((rendered.text.clone(), rendered.cursor_offset))
        };
        if let Some((text, Some(offset))) = delivered {
            keys::move_left(self.events(), plan::graphemes_after_utf16(&text, offset))?;
        }
        self.host.expanded(ExpansionEvent {
            snippet_id: snippet.id,
            chars_saved: plan::chars_saved(&rendered.text, ""),
        });
        Ok(())
    }
}
