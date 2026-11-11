//! `form`: the fill-in form, a launcher panel (440 wide, radius 16, height 160..640 by
//! content). S57 builds its content and placement. The window decides for itself: it shows
//! when `forms.pending()` appears and hides when it goes. No dismissal on blur (FRM-6).
use super::super::{lifecycle::closes_windows, theme_sync::ThemeSync, Shell};
use kudzu_app::assembly::Kudzu;
use kudzu_app::forms::{FormRequest, FormService};
use kudzu_app::ports::{FormToken, PanelFocus};
use kudzu_app::settings::SettingsService;
use rune::{Key, KeyEvent, Tx};
use std::sync::Arc;

pub struct FormServices {
    pub settings: Arc<SettingsService>,
    pub forms: Arc<FormService>,
}

impl From<&Arc<Kudzu>> for FormServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            forms: kudzu.forms.clone(),
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
}

pub struct FormState {
    pub svc: FormServices,
    pub shell: Shell,
    pub model: FormModel,
}

impl FormState {
    pub fn new(svc: FormServices, shell: Shell) -> Self {
        Self {
            svc,
            shell,
            model: FormModel::default(),
        }
    }

    fn token(&self) -> Option<FormToken> {
        self.model.request.as_ref().map(|request| request.token)
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
    pub fn hide(&mut self) {
        if self.model.request.take().is_some() {
            self.shell.handles().form.hide();
        }
    }

    /// Report a key-status change of the panel. S57 adds the event stamp (`after`).
    fn focused(&mut self, key: bool) {
        if let Some(token) = self.token() {
            self.model.focus_seq += 1;
            let focus = PanelFocus {
                key,
                seq: self.model.focus_seq,
                after: None,
            };
            self.svc.forms.panel_focused(token, focus);
        }
    }

    pub fn sync(&mut self) -> bool {
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());
        let pending = self.svc.forms.pending().get();
        let next = pending.as_ref().clone();
        let same = match (&next, &self.model.request) {
            (Some(next), Some(shown)) => next.token == shown.token,
            (None, None) => true,
            _ => false,
        };
        if !same {
            changed = true;
            match next {
                Some(request) => {
                    self.model.request = Some(request);
                    self.model.focus_seq = 0;
                    let form = self.shell.handles().form.clone();
                    self.shell.handles().form.update(move |s, _| {
                        if s.model.request.is_some() {
                            form.show();
                        }
                    });
                }
                None => self.hide(),
            }
        }
        changed
    }
}

pub(super) fn app() -> rune::App<FormState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        poll: Some(|s: &mut FormState, _, _| s.sync()),
        on_key: Some(on_key),
        ..rune::AppSpec::new(
            "Fill In",
            (440.0, 220.0),
            super::unreachable_init,
            super::super::placeholder::form,
        )
    };
    rune::App::new(spec)
        .windowless()
        .on_focus(|s: &mut FormState, focused, _| s.focused(focused))
        .on_visibility(|s: &mut FormState, _, _| s.shell.push_windows())
}

fn on_key(s: &mut FormState, key: &KeyEvent, _: &mut Tx) -> bool {
    if closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    if key.key == Key::Escape {
        s.cancel();
        return true;
    }
    false
}

pub(super) fn spec(app: rune::App<FormState>, state: FormState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    let options = rune::WindowOptions::launcher_panel().macos(|n| n.corner_radius(16.0));
    super::finish("form", app, &shell, state)
        .configured_options(options)
        .into()
}
