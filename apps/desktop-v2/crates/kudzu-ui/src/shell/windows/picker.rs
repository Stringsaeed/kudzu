//! `picker`: clipboard history, a launcher panel (800×520, radius 18, `PLAN.md` S60).
//! Unavailable while recording is off, and hidden if recording turns off while it is open.
//! `begin_pick` reads the frontmost app, so it runs on a worker; the session it returns holds
//! the app to paste into and give focus back to. `dismiss_with` is how the panel hides: it
//! gives focus back to that app only when the user closed the panel (Escape, the shortcut
//! again), never on a blur.
use super::super::{routes::Route, theme_sync::ThemeSync, Shell};
use crate::picker::drive::{Job, PasteJob};
use crate::picker::model::Picker;
use chrono::{DateTime, FixedOffset, Utc};
use kudzu_app::apps::Apps;
use kudzu_app::assembly::Kudzu;
use kudzu_app::clipboard::{ClipboardError, ClipboardService, PickSession};
use kudzu_app::ports::{Clock, FileOpener};
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use std::sync::Arc;

pub struct PickerServices {
    pub settings: Arc<SettingsService>,
    pub clipboard: Arc<ClipboardService>,
    pub apps: Arc<Apps>,
    pub clock: Arc<dyn Clock>,
    pub files: Arc<dyn FileOpener>,
}

impl From<&Arc<Kudzu>> for PickerServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            clipboard: kudzu.clipboard.clone(),
            apps: kudzu.apps.clone(),
            clock: kudzu.clock.clone(),
            files: kudzu.files.clone(),
        }
    }
}

pub struct PickerModel {
    pub theme: ThemeSync,
    pub open: bool,
    pub(crate) begin: Option<Task<Result<PickSession, ClipboardError>>>,
    pub session: Option<PickSession>,
    pub picker: Picker,
    pub(crate) jobs: Vec<Job>,
    /// Pastes that left the panel; they outlive the opening that made them.
    pub(crate) pastes: Vec<PasteJob>,
    /// A new opening wants the search field focused; `poll` does it, so the same code runs
    /// in the window and in the headless tests.
    pub(crate) focus_search: bool,
    /// Counts openings, so an older paste's answer can be told from the current one's.
    pub(crate) opening: u64,
    /// The UI clock at the last key or poll, for deadlines.
    pub clock_secs: f64,
    /// The last route the picker asked `main` to show (⌘S, ⌘,), for tests and the log.
    pub last_route: Option<Route>,
}

pub struct PickerState {
    pub svc: PickerServices,
    pub shell: Shell,
    pub model: PickerModel,
}

impl PickerState {
    pub fn new(svc: PickerServices, shell: Shell) -> Self {
        let now = wall(&*svc.clock, utc());
        Self {
            svc,
            shell,
            model: PickerModel {
                theme: ThemeSync::default(),
                open: false,
                begin: None,
                session: None,
                picker: Picker::new(now),
                jobs: Vec::new(),
                pastes: Vec::new(),
                opening: 0,
                focus_search: false,
                clock_secs: 0.0,
                last_route: None,
            },
        }
    }

    /// Pastes whose answer has not been taken yet.
    pub fn pastes_pending(&self) -> usize {
        self.model.pastes.len()
    }

    fn recording(&self) -> bool {
        self.svc.settings.watch().get().clipboard_enabled
    }

    /// The viewer's zone: the machine's when native, UTC for fixtures.
    pub(crate) fn zone(&self) -> FixedOffset {
        if self.shell.native() {
            *chrono::Local::now().offset()
        } else {
            utc()
        }
    }

    /// The service clock in the viewer's zone.
    pub(crate) fn now(&self) -> DateTime<FixedOffset> {
        wall(&*self.svc.clock, self.zone())
    }

    /// The shortcut and the tray row: closes an open picker, opens a closed one.
    pub fn toggle(&mut self) {
        if self.model.open {
            let restore = !self.shell.main_focused();
            self.dismiss_with(restore);
        } else if self.recording() {
            self.open();
        }
    }

    /// A fresh opening. The rows are pulled before the panel shows, so its first frame is the
    /// real list; the show is queued for the next turn, after the hidden panel drew this state.
    pub fn open(&mut self) {
        let now = self.now();
        self.model.picker.reset(now);
        self.model.jobs.clear();
        self.model.opening += 1;
        self.model.focus_search = true;
        self.model.session = None;
        let clipboard = self.svc.clipboard.clone();
        match Task::spawn("kudzu-picker-begin", move || clipboard.begin_pick()) {
            Ok(task) => self.model.begin = Some(task),
            Err(error) => log::warn!("the clipboard picker could not start: {error}"),
        }
        self.model.open = true;
        crate::picker::drive::pull(self);
        let picker = self.shell.handles().picker.clone();
        self.shell.handles().picker.update(move |s, _| {
            if s.model.open {
                crate::picker::place(s);
                picker.show();
            }
        });
    }

    /// Asks `main` to show `route` (the picker then closes, by its caller).
    pub(crate) fn go_main(&mut self, route: Route) {
        self.model.last_route = Some(route.clone());
        self.shell.open_main(route);
    }

    /// Hides without giving focus back (a blur, or a close that opens another window).
    pub fn dismiss(&mut self) {
        self.dismiss_with(false);
    }

    /// The one way the picker hides. `restore` gives focus back to the app it came from.
    pub fn dismiss_with(&mut self, restore: bool) {
        self.model.begin = None;
        self.model.picker.held = Default::default();
        self.model.picker.clips.look.show_numbers = false;
        self.model.picker.hide = None;
        self.model.picker.pending_paste = None;
        // A copy waiting to become a paste ends with the opening. Its ticket goes with the
        // task (or with the worker, if it is still copying), so the paste gate is free before
        // the restore below waits on it and before the next plain copy.
        self.model
            .jobs
            .retain(|job| !matches!(job, crate::picker::drive::Job::CopyForPaste { .. }));
        if let Some(session) = self.model.session.take() {
            if restore {
                match session.restore_target() {
                    Ok(task) => self.shell.settle("picker restore", task),
                    Err(error) => log::warn!("the app was not given focus back: {error}"),
                }
            }
        }
        if self.model.open {
            self.model.open = false;
            self.shell.handles().picker.hide();
        }
    }

    pub fn sync(&mut self) -> bool {
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());
        if self.model.open && !self.recording() {
            self.dismiss();
            changed = true;
        }
        if let Some(result) = self.model.begin.as_ref().and_then(|task| task.take()) {
            self.model.begin = None;
            match result {
                Ok(Ok(session)) if self.model.open => {
                    self.model.picker.can_paste = session.can_paste;
                    self.model.picker.target = session.target().map(|a| a.name.clone());
                    self.model.session = Some(session);
                    crate::picker::drive::session_landed(self);
                }
                Ok(Ok(_)) => {}
                Ok(Err(error)) => log::warn!("the clipboard picker could not start: {error}"),
                Err(error) => log::warn!("the clipboard picker could not start: {error}"),
            }
            changed = true;
        }
        changed
    }
}

fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap_or_else(|| unreachable!("zero is an offset"))
}

fn wall(clock: &dyn Clock, zone: FixedOffset) -> DateTime<FixedOffset> {
    let at: DateTime<Utc> = clock.now().into();
    at.with_timezone(&zone)
}

pub(super) fn app() -> rune::App<PickerState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        scenes: crate::picker::scenes::SCENES,
        poll: Some(crate::picker::poll),
        on_key: Some(crate::picker::on_key),
        ..rune::AppSpec::new(
            "Clipboard History",
            (crate::picker::WIDTH, crate::picker::HEIGHT),
            super::unreachable_init,
            crate::picker::view,
        )
    };
    rune::App::new(spec)
        .windowless()
        .modifier_keys()
        .on_focus(|s: &mut PickerState, focused, _| {
            if !focused {
                s.dismiss();
            }
        })
        .on_visibility(|s: &mut PickerState, _, _| s.shell.push_windows())
}

pub(super) fn spec(app: rune::App<PickerState>, state: PickerState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    super::finish("picker", app, &shell, state)
        .configured_options(rune::WindowOptions::launcher_panel().macos(|n| n.corner_radius(18.0)))
        .into()
}
