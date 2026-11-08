//! Window behaviour: the main library window, the quick-search palette, the clipboard picker
//! and the caret form. Ported from the Tauri shell's `windows.rs`: the same rules for which app
//! gets focus back, when the engine is suppressed and where each panel opens. The windows
//! themselves are Rune windows behind [`super::core::Shell`].

use kudzu_engine::{system, Rect};

use super::core::{core, Event, Win};

pub const PALETTE_W: f64 = 760.0;
pub const PALETTE_H: f64 = 480.0;
pub const FORM_W: f64 = 440.0;
pub const CLIP_W: f64 = 800.0;
pub const CLIP_H: f64 = 520.0;
pub const PALETTE_RADIUS: f32 = 18.0;
pub const FORM_RADIUS: f32 = 16.0;

fn not_kudzu(app: Option<kudzu_engine::AppInfo>) -> Option<kudzu_engine::AppInfo> {
    app.filter(|a| a.pid != Some(std::process::id() as i32))
}

pub fn toggle_palette() {
    let core = core();
    let shell = core.shell();
    if shell.is_visible(Win::Palette) {
        hide_palette(true);
        return;
    }
    // Remember where the user was, unless it's Kudzu itself.
    let front = not_kudzu(system::frontmost_app());
    *core.palette_target.lock() = front.clone();
    if let Some(engine) = core.engine() {
        engine.set_suppressed(true);
    }
    shell.place_under_pointer(Win::Palette, 0.2);
    core.emit(Event::PaletteShow { target: front });
    shell.show(Win::Palette);
}

/// Hides the palette; with `restore`, hands focus back to the app the user came from.
pub fn hide_palette(restore: bool) {
    let core = core();
    let shell = core.shell();
    if !shell.is_visible(Win::Palette) {
        return;
    }
    shell.hide(Win::Palette);
    if core.pending_form.lock().is_none()
        && let Some(engine) = core.engine() {
            engine.set_suppressed(false);
        }
    if restore && !shell.is_focused(Win::Main)
        && let Some(target) = core.palette_target.lock().clone() {
            system::activate_app(&target);
        }
}

/// Shows the clipboard history picker over the current app (or hides it if open).
pub fn toggle_clipboard() {
    let core = core();
    let shell = core.shell();
    if shell.is_visible(Win::Clipboard) {
        hide_clipboard(true);
        return;
    }
    if !core.store.lock().settings.clipboard_enabled {
        return;
    }
    hide_palette(false);
    let front = not_kudzu(system::frontmost_app());
    *core.clip_target.lock() = front.clone();
    if let Some(engine) = core.engine() {
        engine.set_suppressed(true);
    }
    shell.place_under_pointer(Win::Clipboard, 0.18);
    core.emit(Event::ClipboardShow { target: front, can_paste: kudzu_engine::clipboard::can_paste() });
    shell.show(Win::Clipboard);
}

/// Hides the clipboard picker; with `restore`, hands focus back to the app the user came from.
pub fn hide_clipboard(restore: bool) {
    let core = core();
    let shell = core.shell();
    if !shell.is_visible(Win::Clipboard) {
        return;
    }
    shell.hide(Win::Clipboard);
    if core.pending_form.lock().is_none()
        && let Some(engine) = core.engine() {
            engine.set_suppressed(false);
        }
    if restore && !shell.is_focused(Win::Main)
        && let Some(target) = core.clip_target.lock().clone() {
            system::activate_app(&target);
        }
}

/// Where the form opens: under the caret, flipped above it near the screen's bottom, kept on
/// screen; centred high on the display when the caret is unknown.
pub fn form_origin(caret: Option<Rect>, display: Option<(f64, f64, f64, f64)>, height: f64) -> (f64, f64) {
    match (caret, display) {
        (Some(c), Some((mx, my, mw, mh))) => {
            let mut y = c.y + c.height + 6.0;
            if y + height > my + mh {
                y = (c.y - height - 6.0).max(my);
            }
            let x = (c.x - 24.0).clamp(mx + 8.0, mx + mw - FORM_W - 8.0);
            (x, y)
        }
        (_, Some((mx, my, mw, mh))) => (mx + (mw - FORM_W) / 2.0, my + mh * 0.28),
        _ => (200.0, 200.0),
    }
}

/// The form's last height, so a new request opens at about the right size.
static FORM_HEIGHT: parking_lot::Mutex<f64> = parking_lot::Mutex::new(360.0);
static FORM_CARET: parking_lot::Mutex<Option<Rect>> = parking_lot::Mutex::new(None);

pub fn show_form(caret: Option<Rect>) {
    let core = core();
    let shell = core.shell();
    *FORM_CARET.lock() = caret;
    let h = *FORM_HEIGHT.lock();
    let (x, y) = form_origin(caret, shell.pointer_display(), h);
    shell.place_rect(Win::Form, x, y, FORM_W, h);
    shell.show(Win::Form);
}

pub fn hide_form() {
    core().shell().hide(Win::Form);
}

/// The form measured its content: keep its top-left and take the new height.
pub fn resize_form(height: f64) {
    let height = height.clamp(160.0, 680.0);
    *FORM_HEIGHT.lock() = height;
    let core = core();
    let shell = core.shell();
    let (x, y) = form_origin(*FORM_CARET.lock(), shell.pointer_display(), height);
    shell.place_rect(Win::Form, x, y, FORM_W, height);
}

/// Shows (and focuses) the main window, adding the Dock icon if the user wants it.
pub fn show_main() {
    let core = core();
    let dock = core.store.lock().settings.show_in_dock;
    core.shell().set_dock_icon(dock);
    core.shell().show(Win::Main);
}

/// Hides the main window and drops back to a menu-bar-only app.
pub fn hide_main() {
    let core = core();
    core.shell().hide(Win::Main);
    core.shell().set_dock_icon(false);
}

/// Returns to the menu bar without interrupting background services (⌘Q).
pub fn hide_all() {
    let core = core();
    let pending = core.pending_form.lock().take();
    if let Some(engine) = core.engine() {
        if let Some(pending) = pending {
            engine.cancel_form(pending.token);
        }
        engine.set_suppressed(false);
    }
    hide_palette(false);
    hide_clipboard(false);
    hide_form();
    core.shell().hide(Win::Permissions);
    hide_main();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_opens_under_the_caret_and_flips_above_near_the_bottom() {
        let display = Some((0.0, 0.0, 1440.0, 900.0));
        let caret = |y| Some(Rect { x: 300.0, y, width: 2.0, height: 18.0 });
        assert_eq!(form_origin(caret(100.0), display, 300.0), (276.0, 124.0));
        assert_eq!(form_origin(caret(800.0), display, 300.0), (276.0, 494.0));
        let (x, y) = form_origin(None, display, 300.0);
        assert_eq!(x, 500.0);
        assert!((y - 252.0).abs() < 1e-9);
        let near_right = Some(Rect { x: 1430.0, y: 100.0, width: 2.0, height: 18.0 });
        assert_eq!(form_origin(near_right, display, 300.0).0, 1440.0 - FORM_W - 8.0);
    }
}
