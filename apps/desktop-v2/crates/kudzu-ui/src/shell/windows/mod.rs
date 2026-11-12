//! The window table of PLAN §1.3: five windows, each with its own state and only the services
//! it uses. S41, S56, S57, S60 and S66 replace a placeholder model and view; the spec, hooks and
//! service bundle here stay the integration point.
mod form;
#[allow(special_module_name)]
pub(crate) mod main;
mod palette;
mod permissions;
mod picker;

pub use form::{FormServices, FormState};
pub use main::{MainServices, MainState};
pub use palette::{PaletteServices, PaletteState};
pub use permissions::{PermissionsServices, PermissionsState};
pub use picker::{PickerServices, PickerState};

use super::{hotkeys::Registrar, settle::Settle, window_list::WindowList, Handles, Identity};
use super::{Inner, Shell};
use kudzu_app::assembly::Kudzu;
use kudzu_app::ports::HotkeyRegistrar;
use std::sync::{Arc, Mutex, PoisonError};

/// The window keys, in table order. `--list-windows` prints these.
pub const KEYS: [&str; 5] = ["main", "palette", "picker", "form", "permissions"];

/// How this process was launched.
#[derive(Clone, Copy, Debug)]
pub struct Launch {
    pub identity: Identity,
    /// The real app (menus, status item, appearance). False in a headless run.
    pub native: bool,
    /// `--hidden`, from the login item. Ignored on first run.
    pub hidden: bool,
}

/// The built windows. Keep `wakers` alive for the app's lifetime (compose stores them in
/// the shell, which every window state holds).
pub struct Windows {
    pub specs: Vec<rune::AnyWindowSpec>,
    pub shell: Shell,
}

impl Windows {
    /// The tray's Quit, for the relaunch helper: drafts, shutdown, then exit.
    pub fn quitter(&self) -> impl Fn() + Send + Sync + 'static {
        let shell = self.shell.clone();
        move || shell.quit()
    }

    /// Wake each window when a watch it reads changes. Its `poll` then pulls the snapshot.
    pub fn wake_on_changes(&self, kudzu: &Kudzu) {
        let h = self.shell.handles();
        let settings = kudzu.settings.watch();
        let subs = [
            kudzu.onboarding.watch().subscribe(waker(&h.main)),
            settings.subscribe(waker(&h.main)),
            settings.subscribe(waker(&h.palette)),
            settings.subscribe(waker(&h.picker)),
            settings.subscribe(waker(&h.form)),
            settings.subscribe(waker(&h.permissions)),
            kudzu.expansion.status().subscribe(waker(&h.main)),
            kudzu.updates.state().subscribe(waker(&h.main)),
            kudzu.library.watch().subscribe(waker(&h.main)),
            kudzu.library.watch().subscribe(waker(&h.palette)),
            kudzu.expansion.outcomes().subscribe(waker(&h.palette)),
            kudzu.clipboard.watch().subscribe(waker(&h.picker)),
            kudzu.forms.pending().subscribe(waker(&h.form)),
            kudzu.forms.outcomes().subscribe(waker(&h.form)),
            kudzu.permissions.watch().subscribe(waker(&h.permissions)),
        ];
        self.shell
            .0
            .wakers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(subs);
    }
}

/// A waker only wakes: the window's poll pulls whatever changed.
fn waker<S: 'static>(handle: &rune::AppHandle<S>) -> impl Fn() + Send + Sync + 'static {
    let handle = handle.clone();
    move || handle.wake()
}

/// `main` opens at launch unless `--hidden`, and always on the first run.
pub(crate) fn opens_at_launch(hidden: bool, first_run: bool) -> bool {
    !hidden || first_run
}

/// The five apps, in table order, before they become windows.
pub(crate) type Apps = (
    rune::App<MainState>,
    rune::App<PaletteState>,
    rune::App<PickerState>,
    rune::App<FormState>,
    rune::App<PermissionsState>,
);

/// The five apps (not yet windows), the shell over their handles, and each window's state.
pub(crate) struct Parts {
    pub shell: Shell,
    pub apps: Apps,
    pub main: MainState,
    pub palette: PaletteState,
    pub picker: PickerState,
    pub form: FormState,
    pub permissions: PermissionsState,
}

/// `registrar` replaces Rune's shortcuts in tests; `None` uses them.
pub(crate) fn parts(
    kudzu: &Arc<Kudzu>,
    launch: Launch,
    registrar: Option<Arc<dyn HotkeyRegistrar>>,
) -> Parts {
    let show_main = opens_at_launch(launch.hidden, kudzu.first_run());
    let apps = (
        main::app(launch.identity, show_main),
        palette::app(),
        picker::app(),
        form::app(),
        permissions::app(),
    );
    let handles = Handles {
        main: apps.0.handle(),
        palette: apps.1.handle(),
        picker: apps.2.handle(),
        form: apps.3.handle(),
        permissions: apps.4.handle(),
    };
    let registrar = registrar.unwrap_or_else(|| {
        Arc::new(Registrar::new(
            handles.palette.clone(),
            handles.picker.clone(),
        ))
    });
    let shell = Shell(Arc::new(Inner {
        registrar,
        handles,
        identity: launch.identity,
        native: launch.native,
        forms: kudzu.forms.clone(),
        expansion: kudzu.expansion.clone(),
        updates: kudzu.updates.clone(),
        selection: kudzu.selection.clone(),
        shutdown: kudzu.clone(),
        windows: WindowList::new(kudzu.windows.clone()),
        settle: Settle::default(),
        wakers: Mutex::new(Vec::new()),
        main_focused: std::sync::atomic::AtomicBool::new(false),
    }));
    Parts {
        main: MainState::new(MainServices::from(kudzu), shell.clone(), show_main),
        palette: PaletteState::new(PaletteServices::from(kudzu), shell.clone()),
        picker: PickerState::new(PickerServices::from(kudzu), shell.clone()),
        form: FormState::new(FormServices::from(kudzu), shell.clone()),
        permissions: PermissionsState::new(PermissionsServices::from(kudzu), shell.clone()),
        shell,
        apps,
    }
}

/// Build the five windows over the composed services. Nothing opens until Rune runs.
pub fn build(kudzu: &Arc<Kudzu>, launch: Launch) -> Windows {
    let p = parts(kudzu, launch, None);
    let specs = vec![
        main::spec(p.apps.0, p.main),
        palette::spec(p.apps.1, p.palette),
        picker::spec(p.apps.2, p.picker),
        form::spec(p.apps.3, p.form),
        permissions::spec(p.apps.4, p.permissions),
    ];
    // The first poll installs menus and hotkeys, also for a windowless `main`.
    p.shell.handles().main.wake();
    Windows {
        specs,
        shell: p.shell,
    }
}

/// Every window: CSS sRGB compositing, the shared font smoothing, and the engine's window list.
fn finish<S: 'static>(
    key: &'static str,
    app: rune::App<S>,
    shell: &Shell,
    state: S,
) -> rune::WindowSpec<S> {
    let tracked = shell.clone();
    let app = app.on_window(move |handle| tracked.track_window(handle));
    crate::theme::window::spec(key, app).state(state)
}

/// `AppSpec::init` is never used: compose supplies each state.
fn unreachable_init<S>() -> S {
    unreachable!("window state is supplied by compose")
}
