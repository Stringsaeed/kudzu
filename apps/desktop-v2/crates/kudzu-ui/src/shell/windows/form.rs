//! `form`: the fill-in form, a launcher panel (440 wide, radius 16, height 160..640 by
//! content). The window shows when `forms.pending()` appears and hides when it goes.
//! No dismissal on blur (FRM-6).
use super::super::{lifecycle::closes_windows, theme_sync::ThemeSync, Shell};
use crate::form::{events, model::Panel, outcomes, place};
use crate::widgets::key_hints::Held;
use kudzu_app::assembly::Kudzu;
use kudzu_app::expansion::{ExpansionService, InsertionOutcome};
use kudzu_app::forms::{FormRequest, FormService, Submission, SubmitStartError};
use kudzu_app::ports::engine::AdmissionError;
use kudzu_app::ports::{FormSubmission, FormToken, PanelFocus};
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use kudzu_core::template::RenderResult;
use rune::{Key, KeyEvent, TextEdit, Tx};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use uuid::Uuid;

pub struct FormServices {
    pub settings: Arc<SettingsService>,
    pub forms: Arc<FormService>,
    pub expansion: Arc<ExpansionService>,
}

impl From<&Arc<Kudzu>> for FormServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            forms: kudzu.forms.clone(),
            expansion: kudzu.expansion.clone(),
        }
    }
}

#[derive(Default)]
pub struct FormModel {
    pub theme: ThemeSync,
    /// The request on screen. Its values are user content: never logged.
    pub request: Option<Arc<FormRequest>>,
    /// Key-status changes of this form's panel, numbered from 1 (FRM-4).
    pub(crate) focus_seq: u64,
    /// Kept after hide so a resign-key report after submit still has a token.
    pub(crate) last_token: Option<FormToken>,
    pub(crate) panel: Option<Panel>,
    /// Headless `--scene` request, kept when `pending()` is empty.
    pub(crate) scene: Option<Arc<FormRequest>>,
    pub(crate) outcome_seq: u64,
    pub(crate) held: Held,
    /// Top-left origin from the last place. Height changes keep this.
    pub(crate) origin: Option<(f64, f64)>,
    pub(crate) place_count: u32,
    pub(crate) placed_before_show: bool,
    pub(crate) last_snippet: Option<Uuid>,
}

pub struct FormState {
    pub svc: FormServices,
    pub shell: Shell,
    pub model: FormModel,
    /// Live `NSApp.currentEvent`, or a test fake. Never invents a stamp.
    pub(crate) current_event: fn() -> Option<kudzu_app::ports::InputEvent>,
    preview_waker: Option<rune::Waker>,
    pub(crate) submit_admission: Arc<Mutex<Option<Result<(), AdmissionError>>>>,
}

impl FormState {
    pub fn new(svc: FormServices, shell: Shell) -> Self {
        Self {
            svc,
            shell,
            model: FormModel::default(),
            current_event: events::live,
            preview_waker: None,
            submit_admission: Arc::new(Mutex::new(None)),
        }
    }

    fn token(&self) -> Option<FormToken> {
        self.model
            .request
            .as_ref()
            .map(|request| request.token)
            .or(self.model.last_token)
    }

    fn event(&self) -> Option<kudzu_app::ports::InputEvent> {
        (self.current_event)()
    }

    pub(crate) fn load(&mut self, request: Arc<FormRequest>) {
        let replacing = self.model.request.is_some();
        self.model.request = Some(request.clone());
        self.model.last_token = Some(request.token);
        self.model.last_snippet = Some(request.snippet_id);
        self.model.focus_seq = 0;
        self.model.panel = Some(Panel::from_request(&request));
        self.model.origin = None;
        self.model.place_count = 0;
        self.model.placed_before_show = false;
        self.attach_preview_waker();
        let height = self
            .model
            .panel
            .as_ref()
            .map(|panel| panel.height)
            .unwrap_or(place::FORM_H_MIN);
        self.place(height);
        self.model.placed_before_show = self.model.place_count > 0;
        if replacing {
            self.focused(true);
        }
        let form = self.shell.handles().form.clone();
        self.shell.handles().form.update(move |s, _| {
            if s.model.request.is_some() {
                form.show();
            }
        });
    }

    fn attach_preview_waker(&mut self) {
        let Some(waker) = self.preview_waker.clone() else {
            return;
        };
        if let Some(panel) = &self.model.panel {
            panel.preview.set_waker(waker);
        }
    }

    /// Escape: cancel this token once and return focus to the target app.
    pub fn cancel(&mut self) {
        if let Some(token) = self.token() {
            match self.svc.forms.cancel(token) {
                Ok(Some(task)) => self.shell.settle("form cancel", task),
                Ok(None) => {}
                Err(error) => log::warn!("cancelling the form failed to start: {error}"),
            }
        }
        self.hide();
    }

    /// Hide without cancelling. `Shell::close_windows` cancels through `forms` first.
    /// Native hide runs while the request is still set so a resign-key report still has a token.
    pub fn hide(&mut self) {
        if self.model.request.is_some() {
            self.shell.handles().form.hide();
            self.model.request = None;
            self.model.panel = None;
        }
    }

    /// Submit: capture seq and the current event, read the outcomes baseline, then hide.
    pub fn submit(&mut self) {
        let Some(request) = self.model.request.clone() else {
            return;
        };
        if let Some(panel) = &mut self.model.panel {
            panel.pull();
        }
        let values = self
            .model
            .panel
            .as_ref()
            .map(|panel| panel.values.clone())
            .unwrap_or_default();
        let submission = FormSubmission {
            event: self.event(),
            focus_seq: self.model.focus_seq,
        };
        if let Some(notice) = self.svc.forms.outcomes().get().as_ref().as_ref() {
            self.model.outcome_seq = notice.sequence;
        }
        *self
            .submit_admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        self.shell.handles().form.hide();
        match self.svc.forms.submit(request.token, values, submission) {
            Ok(Some(task)) => self.keep_submit(task),
            Ok(None) => {}
            Err(error) => self.refuse(error),
        }
        self.model.request = None;
        self.model.panel = None;
    }

    fn keep_submit(&mut self, task: Task<Submission>) {
        let admission = self.submit_admission.clone();
        match Task::spawn("form-submit-log", move || {
            wait_ready(&task);
            match task.take() {
                Some(Ok(submission)) => {
                    if let Err(error) = submission.admission {
                        log::warn!("form admission failed: {error}");
                    }
                    if let Some(error) = submission.memory_error {
                        log::warn!("form memory persist failed: {error}");
                    }
                    *admission.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some(submission.admission);
                }
                Some(Err(error)) => log::warn!("form submit task failed: {error}"),
                None => log::warn!("form submit task timed out"),
            }
        }) {
            Ok(logged) => self.shell.settle_value("form submit", logged),
            Err(error) => log::warn!("form submit log failed to start: {error}"),
        }
    }

    fn refuse(&mut self, error: SubmitStartError) {
        log::warn!("submitting the form failed to start: {error}");
        match error.restoration {
            Ok(task) => self.shell.settle("form restore", task),
            Err(start) => {
                log::warn!("restoring the target after a refused submit failed to start: {start}")
            }
        }
    }

    /// Settle `SubmitStartError.restoration` like an explicit cancel.
    #[cfg(test)]
    pub(crate) fn settle_restoration(&mut self, error: SubmitStartError) {
        self.refuse(error);
    }

    pub(crate) fn focused(&mut self, key: bool) {
        if let Some(token) = self.token() {
            self.model.focus_seq += 1;
            self.svc.forms.panel_focused(
                token,
                PanelFocus {
                    key,
                    seq: self.model.focus_seq,
                    after: self.event(),
                },
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn report_focus(&mut self, key: bool) {
        self.focused(key);
    }

    #[cfg(test)]
    pub(crate) fn key(&mut self, key: &KeyEvent, tx: &mut Tx) -> bool {
        on_key(self, key, tx)
    }

    pub fn sync(&mut self) -> bool {
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());
        changed |= self.drain_outcomes();
        let pending = self.svc.forms.pending().get();
        let next = pending
            .as_ref()
            .clone()
            .or_else(|| self.model.scene.clone());
        let same = match (&next, &self.model.request) {
            (Some(next), Some(shown)) => next.token == shown.token,
            (None, None) => true,
            _ => false,
        };
        if !same {
            changed = true;
            match next {
                Some(request) => self.load(request),
                None => self.hide(),
            }
        }
        if let Some(panel) = &mut self.model.panel {
            if let Some(result) = panel.preview.take().flatten() {
                panel.preview_result = Some(result);
                changed = true;
            }
        }
        self.request_preview();
        changed
    }

    fn drain_outcomes(&mut self) -> bool {
        let snapshot = self.svc.forms.outcomes().get();
        let Some(notice) = outcomes::next(snapshot.as_ref().as_ref(), self.model.outcome_seq)
        else {
            return false;
        };
        self.model.outcome_seq = notice.sequence;
        let snippet = self
            .model
            .request
            .as_ref()
            .map(|r| r.snippet_id)
            .or(self.model.last_snippet);
        let kind = outcomes::classify(&notice, snippet);
        if kind == outcomes::Kind::Ours {
            match notice.outcome {
                InsertionOutcome::Failed(error) => {
                    log::warn!("form insertion failed: {error:?}");
                }
                InsertionOutcome::TriggerLost => {
                    log::warn!("a stored trigger could not be restored");
                }
                _ => {}
            }
        }
        true
    }

    fn request_preview(&mut self) {
        let Some(request) = self.model.request.clone() else {
            return;
        };
        let Some(panel) = self.model.panel.as_mut() else {
            return;
        };
        panel.pull();
        let values = panel.values.clone();
        let token = request.token;
        let snippet = request.snippet_id;
        let selection = request.selection.clone();
        let forms = self.svc.forms.clone();
        let expansion = self.svc.expansion.clone();
        panel.preview.request_if_changed(&values, move |values| {
            if let Ok(Some(task)) = forms.preview(token, values.clone()) {
                return wait_preview(Some(task));
            }
            wait_preview(expansion.preview_snippet(snippet, selection, values).ok())
        });
    }

    pub(crate) fn warm_preview(&mut self) {
        let Some(request) = self.model.request.clone() else {
            return;
        };
        let values = self
            .model
            .panel
            .as_ref()
            .map(|panel| panel.values.clone())
            .unwrap_or_default();
        let from_library = self
            .svc
            .expansion
            .preview_snippet(
                request.snippet_id,
                request.selection.clone(),
                values.clone(),
            )
            .ok();
        let task = if let Some(result) = wait_preview(from_library) {
            Some(result)
        } else {
            wait_preview(
                self.svc
                    .expansion
                    .preview(
                        crate::form::scenes::DEMO_BODY.into(),
                        kudzu_app::expansion::PreviewContext {
                            snippet_id: Some(request.snippet_id),
                            formatting: None,
                            selection: request.selection.clone(),
                            form_values: values,
                        },
                    )
                    .ok(),
            )
        };
        if let (Some(result), Some(panel)) = (task, self.model.panel.as_mut()) {
            panel.preview_result = Some(result);
        }
    }

    pub(crate) fn edit(&mut self, name: &str) -> &mut TextEdit {
        self.model
            .panel
            .as_mut()
            .expect("form panel")
            .edits
            .entry(name.to_owned())
            .or_default()
    }

    pub(crate) fn edit_snapshot(&self, name: &str) -> TextEdit {
        self.model
            .panel
            .as_ref()
            .and_then(|panel| panel.edits.get(name).cloned())
            .unwrap_or_default()
    }

    pub(crate) fn break_line(&mut self, name: &str) {
        let edit = self.edit(name);
        let start = edit.anchor.min(edit.focus);
        let end = edit.anchor.max(edit.focus);
        if !edit.text.is_char_boundary(start) || !edit.text.is_char_boundary(end) {
            return;
        }
        edit.text.replace_range(start..end, "\n");
        edit.focus = start + 1;
        edit.anchor = edit.focus;
    }

    pub(crate) fn choose(&mut self, name: &str, value: &str) {
        if let Some(panel) = &mut self.model.panel {
            panel.values.insert(name.to_owned(), value.to_owned());
            panel.edits.insert(name.to_owned(), TextEdit::new(value));
            panel.open_choice = None;
        }
    }

    pub(crate) fn toggle_choice(&mut self, name: &str) {
        if let Some(panel) = &mut self.model.panel {
            panel.open_choice = match panel.open_choice.as_deref() {
                Some(open) if open == name => None,
                _ => Some(name.to_owned()),
            };
        }
    }

    pub(crate) fn close_choice(&mut self, name: &str) {
        if let Some(panel) = &mut self.model.panel {
            if panel.open_choice.as_deref() == Some(name) {
                panel.open_choice = None;
            }
        }
    }

    pub(crate) fn fit_height(&mut self, measured: f64, tx: &mut Tx) {
        let height = place::clamp_form_height(measured);
        let dirty = self
            .model
            .panel
            .as_ref()
            .is_some_and(|panel| (panel.height - height).abs() > 0.5);
        if !dirty {
            return;
        }
        if let Some(panel) = &mut self.model.panel {
            panel.height = height;
        }
        tx.resize_window((place::FORM_W as f32, height as f32));
    }

    fn place(&mut self, height: f64) {
        let height = place::clamp_form_height(height);
        let caret = self
            .model
            .request
            .as_ref()
            .and_then(|request| request.caret);
        let Some((x, y)) = self.origin_for(caret, height) else {
            return;
        };
        self.model.origin = Some((x, y));
        self.model.place_count = self.model.place_count.saturating_add(1);
        if !self.shell.native() {
            return;
        }
        self.shell.handles().form.place(
            rune::DisplayTarget::Primary,
            rune::OverlayPlacement::Rect {
                x: x as f32,
                y: y as f32,
                width: place::FORM_W as f32,
                height: height as f32,
            },
        );
    }

    fn origin_for(
        &self,
        caret: Option<kudzu_core::system::Rect>,
        height: f64,
    ) -> Option<(f64, f64)> {
        if !self.shell.native() {
            let screen = place::Frame {
                x: 0.0,
                y: 0.0,
                w: 1440.0,
                h: 900.0,
            };
            let screen = place::screen_holding(&[screen], caret, None)?;
            return Some(place::form_origin(screen, caret, height));
        }
        let mtm = objc2_foundation::MainThreadMarker::new()?;
        let screens = kudzu_appkit::screens::snapshot(mtm);
        let primary_max_y = screens.first()?.frame.max_y();
        let frames: Vec<place::Frame> = screens
            .iter()
            .map(|screen| place::top_left_frame(screen.frame, primary_max_y))
            .collect();
        let pointer = kudzu_appkit::screens::under_pointer(mtm)
            .map(|screen| place::top_left_frame(screen.frame, primary_max_y));
        let screen = place::screen_holding(&frames, caret, pointer)?;
        Some(place::form_origin(screen, caret, height))
    }
}

fn wait_ready<T: Send + 'static>(task: &Task<T>) {
    if *task.ready().get() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let _sub = task.ready().subscribe(move || {
        let _ = tx.send(());
    });
    if !*task.ready().get() {
        let _ = rx.recv_timeout(Duration::from_secs(5));
    }
}

fn wait_preview(
    task: Option<Task<Result<RenderResult, kudzu_app::expansion::ExpansionError>>>,
) -> Option<RenderResult> {
    let task = task?;
    wait_ready(&task);
    match task.take() {
        Some(Ok(Ok(result))) => Some(result),
        _ => None,
    }
}

pub(super) fn app() -> rune::App<FormState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        scenes: crate::form::scenes::SCENES,
        setup: Some(setup),
        poll: Some(|s: &mut FormState, _, _| s.sync()),
        on_key: Some(on_key),
        ..rune::AppSpec::new(
            "Fill In",
            (place::FORM_W as f32, 360.0),
            super::unreachable_init,
            crate::form::view::view,
        )
    };
    rune::App::new(spec)
        .windowless()
        .on_focus(|s: &mut FormState, focused, _| s.focused(focused))
        .on_visibility(|s: &mut FormState, _, _| s.shell.push_windows())
}

fn setup(s: &mut FormState, waker: rune::Waker) {
    s.preview_waker = Some(waker);
    s.attach_preview_waker();
}

fn on_key(s: &mut FormState, key: &KeyEvent, tx: &mut Tx) -> bool {
    if closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    let multiline = s
        .model
        .panel
        .as_ref()
        .is_some_and(|panel| panel.focused_multiline());
    let hint = if key.key == Key::Enter && multiline && !key.mods.cmd {
        None
    } else {
        crate::widgets::key_hints::hint_for(key)
    };
    s.model.held.press(hint);
    crate::actions::route::<_, crate::actions::form::FormAction>(s, key, tx)
}

pub(super) fn spec(app: rune::App<FormState>, state: FormState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    let options = rune::WindowOptions::launcher_panel()
        .macos(|n| n.corner_radius(crate::theme::radius::FORM_WINDOW));
    super::finish("form", app, &shell, state)
        .configured_options(options)
        .into()
}
