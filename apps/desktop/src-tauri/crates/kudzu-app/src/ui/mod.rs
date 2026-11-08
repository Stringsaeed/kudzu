//! The Rune UI: five windows over one backend, as the Tauri app had five webviews.
//!
//! | Window | Web route | State |
//! | --- | --- | --- |
//! | main (library, editor, clipboard, insights, settings, onboarding) | `/` | [`main::MainUi`] |
//! | quick search | `/palette` | [`palette::PaletteUi`] |
//! | clipboard picker | `/clipboard` | [`clipboard::picker::PickerUi`] |
//! | fill-in form | `/form` | [`form::FormUi`] |
//! | permission companion | `/permissions` | [`permissions::PermissionsUi`] |
//!
//! Each window's state mirrors what it shows from the backend and applies [`Event`]s from the
//! core's bus (what the webviews did with `listen`). Handlers call `crate::backend` directly
//! (what they did with `invoke`); slow calls go to a worker and land with `AppHandle::update`.

pub mod clipboard;
pub mod editor;
pub mod form;
#[allow(dead_code)] // generated: every icon the web UI imports
pub mod icons;
pub mod kit;
pub mod main;
pub mod native;
pub mod palette;
pub mod permissions;
pub mod theme;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use parking_lot::Mutex;
use rune::{
    Align, App, AppHandle, FontSmoothing, AppSpec, Chrome, Cx, DisplayTarget, MacWindowOptions, OverlayPlacement, RunOptions, ScreenEdge, ShortcutId,
    WindowOptions, WindowSpec,
};

use crate::backend::core::{core, Event, Shell, Slot, TrayUpdate, Win};
use crate::backend::{self, windows};
use crate::settings::Theme as Appearance;

/// The user's appearance setting, for every window's view.
static APPEARANCE: AtomicU8 = AtomicU8::new(0);
/// Which windows have keyboard focus, from each window's `on_focus`.
static FOCUSED: [AtomicBool; 5] = [const { AtomicBool::new(false) }; 5];

fn win_index(win: Win) -> usize {
    match win {
        Win::Main => 0,
        Win::Palette => 1,
        Win::Clipboard => 2,
        Win::Form => 3,
        Win::Permissions => 4,
    }
}

pub fn set_focused(win: Win, focused: bool) {
    FOCUSED[win_index(win)].store(focused, Ordering::Relaxed);
}

/// Whether a view should draw dark: the user's choice, or macOS's for System.
pub fn is_dark<S>(cx: &Cx<S>) -> bool {
    match APPEARANCE.load(Ordering::Relaxed) {
        1 => false,
        2 => cx.system_dark(),
        _ => true,
    }
}

/// The theme for this view.
pub fn theme_for<S>(cx: &Cx<S>) -> &'static theme::Theme {
    theme::theme(is_dark(cx))
}

fn appearance_code(t: Appearance) -> u8 {
    match t {
        Appearance::Dark => 0,
        Appearance::Light => 1,
        Appearance::System => 2,
    }
}

/// One window, as the shell drives it.
struct Entry {
    open: Box<dyn Fn() + Send + Sync>,
    hide: Box<dyn Fn() + Send + Sync>,
    visible: Box<dyn Fn() -> bool + Send + Sync>,
    place: Box<dyn Fn(DisplayTarget, OverlayPlacement) + Send + Sync>,
    /// Re-runs the view (an appearance change), with no animation.
    refresh: Box<dyn Fn() + Send + Sync>,
}

fn entry<S: 'static>(h: &AppHandle<S>) -> Entry {
    let (a, b, c, d, e) = (h.clone(), h.clone(), h.clone(), h.clone(), h.clone());
    Entry {
        open: Box::new(move || a.open()),
        hide: Box::new(move || b.hide()),
        visible: Box::new(move || c.is_visible()),
        place: Box::new(move |t, p| d.place(t, p)),
        refresh: Box::new(move || {
            e.update(|_, tx| tx.instant());
        }),
    }
}

/// The backend's window manager, over Rune handles.
struct RuneShell {
    windows: HashMap<Win, Entry>,
    main: AppHandle<main::MainUi>,
    shortcuts: Mutex<HashMap<SlotKey, ShortcutId>>,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
enum SlotKey {
    Palette,
    Clipboard,
}

impl From<Slot> for SlotKey {
    fn from(s: Slot) -> Self {
        match s {
            Slot::Palette => SlotKey::Palette,
            Slot::Clipboard => SlotKey::Clipboard,
        }
    }
}

impl Shell for RuneShell {
    fn show(&self, win: Win) {
        if let Some(e) = self.windows.get(&win) {
            (e.open)();
        }
    }

    fn hide(&self, win: Win) {
        if let Some(e) = self.windows.get(&win) {
            (e.hide)();
        }
        set_focused(win, false);
    }

    fn is_visible(&self, win: Win) -> bool {
        self.windows.get(&win).is_some_and(|e| (e.visible)())
    }

    fn is_focused(&self, win: Win) -> bool {
        FOCUSED[win_index(win)].load(Ordering::Relaxed)
    }

    fn place_under_pointer(&self, win: Win, top: f32) {
        let Some(e) = self.windows.get(&win) else { return };
        let height = native::pointer_display().map_or(900.0, |d| d.3) as f32;
        (e.place)(DisplayTarget::UnderPointer, OverlayPlacement::Edge { edge: ScreenEdge::Top, margin: height * top, align: Align::Center, inset: 0.0 });
    }

    fn place_rect(&self, win: Win, x: f64, y: f64, width: f64, height: f64) {
        if let Some(e) = self.windows.get(&win) {
            (e.place)(DisplayTarget::Primary, OverlayPlacement::Rect { x: x as f32, y: y as f32, width: width as f32, height: height as f32 });
        }
    }

    fn pointer_display(&self) -> Option<(f64, f64, f64, f64)> {
        native::pointer_display()
    }

    fn set_dock_icon(&self, visible: bool) {
        self.main.set_dock_icon(visible);
    }

    fn register_hotkey(&self, slot: Slot, accelerator: &str) -> Result<(), String> {
        let shortcut: rune::Shortcut = accelerator.parse().map_err(|e| format!("{e:?}"))?;
        let mut ids = self.shortcuts.lock();
        if let Some(old) = ids.remove(&slot.into()) {
            let _ = self.main.unregister_shortcut(old);
        }
        let id = self
            .main
            .register_shortcut(shortcut, move |_, _| match slot {
                Slot::Palette => windows::toggle_palette(),
                Slot::Clipboard => windows::toggle_clipboard(),
            })
            .map_err(|e| match e {
                rune::ShortcutError::Conflict => "it's already in use".to_string(),
                other => format!("{other:?}"),
            })?;
        ids.insert(slot.into(), id);
        Ok(())
    }

    fn unregister_hotkey(&self, slot: Slot) {
        if let Some(id) = self.shortcuts.lock().remove(&slot.into()) {
            let _ = self.main.unregister_shortcut(id);
        }
    }

    fn tray(&self, update: TrayUpdate) {
        // AppKit wants the main thread; window updates run there.
        self.main.update(move |_, _| match &update {
            TrayUpdate::Status(s) => native::tray::update(s),
            TrayUpdate::ClipboardEnabled(on) => native::tray::set_clipboard_enabled(*on),
            TrayUpdate::PaletteHint(a) => native::tray::set_hotkey_hint(a),
            TrayUpdate::ClipboardHint(a) => native::tray::set_clipboard_hint(a),
            TrayUpdate::Update(u) => native::tray::set_update(u),
        });
    }

    fn apply_theme(&self, theme: Appearance) {
        APPEARANCE.store(appearance_code(theme), Ordering::Relaxed);
        self.main.update(move |_, _| native::set_appearance(theme));
        for e in self.windows.values() {
            (e.refresh)();
        }
    }

    fn quit(&self) {
        self.main.quit();
    }
}

/// Every window's bus subscription: apply the event to its state on the UI thread.
fn subscribe<S: 'static>(h: &AppHandle<S>, apply: fn(&mut S, &Event, &mut rune::Tx) -> bool) {
    let h = h.clone();
    core().bus.subscribe(move |ev| {
        let ev = ev.clone();
        h.update_if(move |s, tx| apply(s, &ev, tx));
    });
}

const CORNER_PALETTE: f32 = windows::PALETTE_RADIUS;
const CORNER_FORM: f32 = windows::FORM_RADIUS;

/// Builds the windows and runs the app. Returns when the app quits.
pub fn run(first_run: bool) {
    let started_hidden = std::env::args().any(|a| a == "--hidden");
    let settings = core().store.lock().settings.clone();
    APPEARANCE.store(appearance_code(settings.theme), Ordering::Relaxed);

    // Rune's own flags (--snap, --tree, --script, --window …) pass through; Kudzu's don't.
    let rune_args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--hidden").collect();
    rune::run_windows_with_args(RunOptions { accessory: true }, rune_args, move |_displays| {
        // Main window: 1180×760, min 900×560, transparent over the sidebar vibrancy, traffic
        // lights at (18, 22), hidden until asked for.
        let main_app = App::new(AppSpec {
            fonts: theme::FONTS,
            families: (theme::SANS, theme::MONO),
            on_key: Some(main::on_key),
            min_size: Some((900.0, 560.0)),
            ..AppSpec::new("Kudzu", (1180.0, 760.0), main::MainUi::init, main::view)
        })
        .keep_running()
        // `-webkit-font-smoothing: antialiased`, as the web UI rendered text.
        .font_smoothing(FontSmoothing::Off)
        .chrome(Chrome { full_size_content: true, traffic_lights: Some((18.0, 14.0)), drag_regions: true })
        .on_focus(|s, focused, tx| {
            set_focused(Win::Main, focused);
            main::on_focus(s, focused, tx);
        })
        .on_close(|_, _| windows::hide_main())
        .on_reopen(|_, _, _| {
            windows::show_main();
            true
        })
        .on_window(native::install_sidebar_vibrancy);
        let main_handle = main_app.handle();
        let main_app = main_app.init({
            let h = main_handle.clone();
            move || main::MainUi::new(Some(h))
        });

        let palette_app = App::new(AppSpec { fonts: theme::FONTS, families: (theme::SANS, theme::MONO), ..AppSpec::new("Quick Search", (windows::PALETTE_W as f32, windows::PALETTE_H as f32), palette::PaletteUi::init, palette::view) })
            .windowless()
            .font_smoothing(FontSmoothing::Off)
            // Holding ⌘ alone shows the ⌘1–9 hints.
            .modifier_keys()
            .on_focus(|s, focused, tx| {
                set_focused(Win::Palette, focused);
                palette::on_focus(s, focused, tx);
            });
        let picker_app = App::new(AppSpec { fonts: theme::FONTS, families: (theme::SANS, theme::MONO), ..AppSpec::new("Clipboard History", (windows::CLIP_W as f32, windows::CLIP_H as f32), clipboard::picker::PickerUi::init, clipboard::picker::view) })
            .windowless()
            .font_smoothing(FontSmoothing::Off)
            // Holding ⌘ alone shows the ⌘1–9 hints.
            .modifier_keys()
            .on_focus(|s, focused, tx| {
                set_focused(Win::Clipboard, focused);
                clipboard::picker::on_focus(s, focused, tx);
            });
        let form_app = App::new(AppSpec { fonts: theme::FONTS, families: (theme::SANS, theme::MONO), ..AppSpec::new("Kudzu", (windows::FORM_W as f32, 360.0), form::FormUi::init, form::view) })
            .windowless()
            .font_smoothing(FontSmoothing::Off)
            .on_focus(|_, focused, _| set_focused(Win::Form, focused))
            // Closed some other way than Esc or ↵ (⌘W): cancel, so the engine isn't left suppressed.
            .on_close(|_, _| {
                let pending = core().pending_form.lock().as_ref().map(|p| p.token);
                if let Some(token) = pending {
                    crate::backend::commands::cancel_form(token);
                }
            });
        let permissions_app = App::new(AppSpec { fonts: theme::FONTS, families: (theme::SANS, theme::MONO), ..AppSpec::new("Kudzu permissions", (420.0, 660.0), permissions::PermissionsUi::init, permissions::view) })
            .windowless()
            .font_smoothing(FontSmoothing::Off)
            .keep_running()
            .on_focus(|_, focused, _| set_focused(Win::Permissions, focused))
            .on_close(permissions::on_close);

        let shell = RuneShell {
            windows: HashMap::from([
                (Win::Main, entry(&main_handle)),
                (Win::Palette, entry(&palette_app.handle())),
                (Win::Clipboard, entry(&picker_app.handle())),
                (Win::Form, entry(&form_app.handle())),
                (Win::Permissions, entry(&permissions_app.handle())),
            ]),
            main: main_handle.clone(),
            shortcuts: Mutex::new(HashMap::new()),
        };
        subscribe(&main_handle, main::apply);
        subscribe(&palette_app.handle(), palette::apply);
        subscribe(&picker_app.handle(), clipboard::picker::apply);
        subscribe(&form_app.handle(), form::apply);
        subscribe(&permissions_app.handle(), permissions::apply);
        permissions::set_handle(permissions_app.handle());
        clipboard::picker::set_handle(picker_app.handle());
        core().set_shell(Box::new(shell));

        // After the main window's first frame: the engine, shortcuts, tray and services.
        let tray_settings = settings.clone();
        let main_app = main_app.on_ready(move |_| {
            native::set_appearance(tray_settings.theme);
            native::tray::create(
                native::tray::TrayInit {
                    dev: backend::identity::get().dev,
                    paused: tray_settings.paused,
                    palette_hotkey: tray_settings.palette_hotkey.clone(),
                    clipboard_hotkey: tray_settings.clipboard_hotkey.clone(),
                    clipboard_enabled: tray_settings.clipboard_enabled,
                },
                main::tray_action,
            );
            native::adopt_app_menu(windows::hide_all, || main::tray_action(native::tray::Action::Quit));
            backend::start();
            if !started_hidden || first_run {
                windows::show_main();
            }
            if backend::permissions::take_resume_after_restart() {
                permissions::show(backend::permissions::PermissionKind::Accessibility);
            }
        });

        // Tauri's `visible_on_all_workspaces(true)`: the panels open on whichever Space (or
        // full-screen app) the user is in. NSWindowCollectionBehaviorCanJoinAllSpaces.
        const ALL_SPACES: usize = 1 << 0;
        let panel = |radius: f32| {
            WindowOptions::launcher_panel().macos(move |native: MacWindowOptions| native.corner_radius(radius).collection_behavior(ALL_SPACES))
        };
        let hidden_main = WindowOptions { transparent: true, visible: false, min_size: Some((900.0, 560.0)), ..Default::default() };
        let companion = WindowOptions { transparent: true, borderless: true, always_on_top: true, visible: false, resizable: false, ..Default::default() }
            .macos(MacWindowOptions::default().corner_radius(18.0).collection_behavior(ALL_SPACES));
        vec![
            WindowSpec::from_app("main", main_app).options(hidden_main).into(),
            WindowSpec::from_app("palette", palette_app).configured_options(panel(CORNER_PALETTE)).into(),
            WindowSpec::from_app("clipboard", picker_app).configured_options(panel(CORNER_PALETTE)).into(),
            WindowSpec::from_app("form", form_app).configured_options(panel(CORNER_FORM)).into(),
            WindowSpec::from_app("permissions", permissions_app).configured_options(companion).into(),
        ] as Vec<rune::AnyWindowSpec>
    });
    backend::on_exit();
}
