//! `main`'s lifecycle (PLAN §1.3): close to hidden without the Dock icon, reopen to `main`,
//! ⌘Q closes windows, and quit runs drafts → `Kudzu::shutdown` → exit, however it starts.
//! The first poll installs the app menu, the status item, the appearance and the hotkeys.
use super::routes::{NewSnippet, Route};
use super::theme_sync;
use super::tray::Tray;
use super::windows::main::HotkeyInputs;
use super::windows::MainState;
use rune::{KeyEvent, Tx};

impl MainState {
    /// Show `main` at its route with the Dock icon the user chose, then patch the app menu
    /// on the next turn, after Rune has applied the Dock change (never by timer).
    pub fn show(&mut self) {
        let handles = self.shell.handles();
        let dock = self.svc.settings.watch().get().show_in_dock;
        handles.main.set_dock_icon(dock);
        handles.main.open();
        handles.main.update(|s, _| s.patch_menu());
    }

    /// Every way `main` hides commits the draft first: hiding runs no close hook.
    pub fn hide(&mut self) {
        self.commit_drafts();
        let main = &self.shell.handles().main;
        main.hide();
        main.set_dock_icon(false);
        self.shell.push_windows();
    }

    /// Drafts, then the ordered shutdown. Idempotent: `on_quit` and the tray both call it.
    pub fn quit(&mut self) {
        self.commit_drafts();
        self.shell.shutdown();
    }

    /// Commit the open draft through the library. Accepted at once; the writer persists it,
    /// and shutdown drains the writer.
    pub fn commit_drafts(&mut self) {
        if let Some(draft) = self.model.draft.take() {
            if let Err(error) = self.svc.library.create_snippet(draft) {
                log::warn!("the open draft could not be saved: {error}");
            }
        }
    }

    /// "New Snippet from Selection": capture on a worker; `sync` routes when it lands.
    pub fn begin_capture(&mut self) {
        match self.shell.selection().capture() {
            Ok(task) => self.model.capture = Some(task),
            Err(error) => {
                log::warn!("selection capture failed to start: {error}");
                self.model.route = Route::NewSnippet(NewSnippet::default());
                self.show();
            }
        }
    }

    /// The close button: Rune hides the window (`keep_running`); drafts and the Dock follow.
    pub fn closed(&mut self) {
        self.commit_drafts();
        self.shell.handles().main.set_dock_icon(false);
        self.shell.push_windows();
    }

    fn start(&mut self) {
        self.model.started = true;
        if self.shell.native() {
            self.install_native();
        }
        self.apply_hotkeys();
        if self.model.open_at_launch {
            self.show();
        }
    }

    fn install_native(&mut self) {
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
            return;
        };
        let shell = self.shell.clone();
        let close = std::rc::Rc::new(move || shell.close_windows());
        match kudzu_appkit::app_menu::AppMenu::install(mtm, self.shell.identity().app_name, close) {
            Ok(menu) => self.model.menu = Some(menu),
            Err(error) => log::warn!("the app menu was not patched: {error}"),
        }
        let model = self.tray_model();
        match Tray::install(mtm, &model, &self.shell) {
            Ok(tray) => self.model.tray = Some(tray),
            Err(error) => log::error!("the menu-bar item could not be installed: {error}"),
        }
        self.apply_appearance();
    }

    /// Close Windows ⌘Q goes in as soon as Rune's menu bar exists, even before any Dock
    /// promotion (S10 review). Until then each poll tries again.
    fn install_menu(&mut self) {
        if self.model.menu.is_some() || !self.shell.native() {
            return;
        }
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
            return;
        };
        let shell = self.shell.clone();
        let close = std::rc::Rc::new(move || shell.close_windows());
        let name = self.shell.identity().app_name;
        if let Ok(menu) = kudzu_appkit::app_menu::AppMenu::install(mtm, name, close) {
            self.model.menu = Some(menu);
        }
    }

    fn patch_menu(&mut self) {
        if let Some(menu) = &self.model.menu {
            if let Err(error) = menu.patch() {
                log::warn!("the app menu was not patched: {error}");
            }
        }
    }

    fn apply_appearance(&self) {
        if !self.shell.native() {
            return;
        }
        if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
            let preference = theme_sync::native(self.model.theme.appearance());
            if let Err(error) = kudzu_appkit::appearance::apply(mtm, preference) {
                log::warn!("the app appearance was not set: {error}");
            }
        }
    }

    /// Main thread only (the registrar). Re-run whenever a chord or recording changes.
    fn apply_hotkeys(&mut self) {
        let settings = self.svc.settings.watch().get();
        let inputs = HotkeyInputs {
            palette: settings.palette_hotkey.clone(),
            clipboard: settings.clipboard_hotkey.clone(),
            recording: settings.clipboard_enabled,
        };
        if self.model.hotkeys_seen.as_ref() == Some(&inputs) {
            return;
        }
        self.model.hotkeys_seen = Some(inputs);
        self.model.hotkey_error = self.svc.hotkeys.apply(self.shell.registrar()).err();
        if let Some(error) = &self.model.hotkey_error {
            log::warn!("a global shortcut was not registered: {error}");
        }
    }

    fn tray_model(&self) -> super::tray::Model {
        let settings = self.svc.settings.watch().get();
        let status = self.svc.expansion.status().get();
        let update = self.svc.updates.state().get();
        let identity = self.shell.identity();
        super::tray::model(&super::tray::Input {
            app_name: identity.app_name,
            dev: identity.dev,
            status: Some(&status),
            clipboard_enabled: settings.clipboard_enabled,
            palette_hotkey: &settings.palette_hotkey,
            clipboard_hotkey: &settings.clipboard_hotkey,
            update: &update,
        })
    }

    /// One pull of everything `main` shows or owns. True when the view must rebuild.
    pub fn sync(&mut self) -> bool {
        let mut changed = false;
        if !self.model.started {
            self.start();
            changed = true;
        }
        self.install_menu();
        if self.model.theme.sync(&self.svc.settings.watch()) {
            self.apply_appearance();
            changed = true;
        }
        self.apply_hotkeys();
        if self.model.tray.is_some() {
            let model = self.tray_model();
            if let Some(tray) = &mut self.model.tray {
                tray.refresh(model);
            }
        }
        if let Some(result) = self.model.capture.as_ref().and_then(|task| task.take()) {
            self.model.capture = None;
            let content = result.unwrap_or_default();
            self.model.route = Route::NewSnippet(NewSnippet { content });
            self.show();
            changed = true;
        }
        self.shell.drain_settled();
        changed
    }
}

pub(super) fn poll(s: &mut MainState, _: f64, _: &mut Tx) -> bool {
    s.sync()
}

/// ⌘Q reaches the app menu's Close Windows natively; this covers a run without that menu.
pub(super) fn on_key(s: &mut MainState, key: &KeyEvent, _: &mut Tx) -> bool {
    if closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    false
}

/// The close button (and ⌘W): `keep_running` hides the window; drafts and the Dock follow.
pub(super) fn on_close(s: &mut MainState, _: &mut Tx) {
    s.closed();
}

/// A system quit (Dock, logout, the app menu's Quit): Rune runs this before windows close.
pub(super) fn on_quit(s: &mut MainState, _: &mut Tx) {
    s.quit();
}

/// A Dock click or a second launch shows `main`, whatever else is open.
pub(super) fn on_reopen(s: &mut MainState, _: bool, _: &mut Tx) -> bool {
    s.show();
    true
}

/// ⌘Q exactly: no other modifier.
pub(crate) fn closes_windows(key: &KeyEvent) -> bool {
    key.is_command('q') && !key.mods.shift && !key.mods.alt && !key.mods.ctrl
}
