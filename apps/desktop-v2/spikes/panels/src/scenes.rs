//! Placement, blur, material, fullscreen and first-mouse scenes.
//! Each one shows a panel, records AppKit facts, and hides it again.

use std::process::Command;
use std::thread;
use std::time::Duration;

use rune::{AppHandle, DisplayTarget, OverlayPlacement};
use serde_json::{json, Value};

use crate::bus;
use crate::cycle::{self, Apps};
use crate::native;
use crate::panel::{Kind, Panel, HOST_TITLE};
use crate::place::{self, Caret, Frame};
use crate::report;

fn ensure_textedit() -> bool {
    let running = Command::new("pgrep")
        .args(["-x", "TextEdit"])
        .status()
        .is_ok_and(|status| status.success());
    if running {
        return false;
    }
    let _ = Command::new("open").args(["-a", "TextEdit"]).status();
    thread::sleep(Duration::from_millis(600));
    true
}

pub fn keys_and_blur(apps: &Apps, notes: &mut Vec<String>) -> Value {
    let launched = ensure_textedit();
    let activated = cycle::on_ui(&apps.palette, |_, _| {
        native::activate_bundle("com.apple.TextEdit")
    });
    thread::sleep(Duration::from_millis(400));
    let before =
        cycle::on_ui(&apps.palette, |_, _| native::front()).unwrap_or_else(cycle::empty_front);
    let shown = cycle::show_cycle(&apps.palette, Kind::Palette, false);
    let during = cycle::on_ui(&apps.palette, |_, _| {
        native::send_key(Kind::Palette.title(), "k", 40)
    });
    thread::sleep(Duration::from_millis(80));
    let after_key = cycle::on_ui(&apps.palette, |panel, _| {
        (panel.query.text.clone(), panel.query.focused)
    });
    let mut tx_focus = None;
    if after_key.as_ref().is_none_or(|(text, _)| text != "k") {
        let _ = cycle::on_ui(&apps.palette, |panel, tx| {
            panel.query.text.clear();
            panel.query.focused = true;
            tx.focus("query", false);
        });
        thread::sleep(Duration::from_millis(50));
        let sent = cycle::on_ui(&apps.palette, |_, _| {
            native::send_key(Kind::Palette.title(), "k", 40)
        });
        thread::sleep(Duration::from_millis(80));
        let text = cycle::on_ui(&apps.palette, |panel, _| panel.query.text.clone());
        tx_focus = Some(json!({ "sent": sent, "text": text }));
    }
    cycle::hide_wait(&apps.palette, Kind::Palette);
    thread::sleep(Duration::from_millis(200));
    let after_hide = cycle::on_ui(&apps.palette, |_, _| native::front());
    let after_panel = cycle::on_ui(&apps.palette, |_, _| native::fact(Kind::Palette.title()));
    let blur = blur_case(&apps.palette, Kind::Palette, true);
    let form_blur = blur_case(&apps.form, Kind::Form, false);
    if before.bundle != "com.apple.TextEdit" {
        notes.push(format!(
            "TextEdit was not frontmost before the key test ({})",
            before.bundle
        ));
    }
    if launched {
        let _ = Command::new("osascript")
            .args(["-e", "tell application \"TextEdit\" to quit saving no"])
            .status();
    }
    json!({
        "launched_textedit": launched,
        "activated": activated,
        "before": report::front_json(&before),
        "shown": shown,
        "key_sent": during,
        "text_after": after_key,
        "tx_focus": tx_focus,
        "after_hide": after_hide.as_ref().map(report::front_json),
        "after_hide_panel": after_panel.as_ref().map(report::fact_json),
        "palette_blur": blur,
        "form_blur": form_blur,
        "ax": native::ax_trusted(),
    })
}

fn blur_case(handle: &AppHandle<Panel>, kind: Kind, expect_hide: bool) -> Value {
    let dismiss = kind.dismiss();
    let title = kind.title();
    let name = kind.name();
    let _ = cycle::on_ui(handle, move |panel, _| panel.dismiss_on_blur = dismiss);
    let shown = cycle::show_cycle(handle, kind, false);
    let prev = bus::bus().focus_len();
    let activated = cycle::on_ui(handle, |_, _| native::activate_bundle("com.apple.TextEdit"));
    let lost = bus::bus().wait_focus(name, false, prev, Duration::from_millis(1200));
    thread::sleep(Duration::from_millis(200));
    let fact =
        cycle::on_ui(handle, move |_, _| native::fact(title)).unwrap_or_else(native::missing);
    if !expect_hide {
        cycle::hide_wait(handle, kind);
    }
    json!({
        "shown": shown,
        "activated": activated,
        "lost_focus": lost,
        "after": report::fact_json(&fact),
        "expect_hide": expect_hide,
    })
}

pub fn placement(apps: &Apps, notes: &mut Vec<String>) -> Value {
    json!({
        "palette": place_band(&apps.palette, Kind::Palette, place::PALETTE_W, place::PALETTE_H, place::PALETTE_FRACTION),
        "picker": place_band(&apps.picker, Kind::Picker, place::PICKER_W, place::PICKER_H, place::PICKER_FRACTION),
        "form_below": place_form(apps, false, "form-below.png", notes),
        "form_above": place_form(apps, true, "form-above.png", notes),
    })
}

fn place_band(
    handle: &AppHandle<Panel>,
    kind: Kind,
    width: f64,
    height: f64,
    fraction: f64,
) -> Value {
    let title = kind.title();
    let name = kind.name();
    let prev = bus::bus().focus_len();
    let planned = cycle::on_ui(handle, move |panel, _| {
        let screen = native::pointer_screen()?;
        let origin = place::band_origin(screen.frame, width, fraction);
        let local = place::to_local(screen.frame, origin);
        panel.me.place(
            DisplayTarget::UnderPointer,
            OverlayPlacement::Rect {
                x: local.0 as f32,
                y: local.1 as f32,
                width: width as f32,
                height: height as f32,
            },
        );
        panel.me.show();
        Some((origin, screen))
    })
    .flatten();
    let _ = bus::bus().wait_focus(name, true, prev, Duration::from_millis(800));
    thread::sleep(Duration::from_millis(40));
    let fact =
        cycle::on_ui(handle, move |_, _| native::fact(title)).unwrap_or_else(native::missing);
    cycle::hide_wait(handle, kind);
    let Some((origin, screen)) = planned else {
        return json!({ "error": "no screen", "measured": report::fact_json(&fact) });
    };
    let expected = Frame {
        x: origin.0,
        y: origin.1,
        w: width,
        h: height,
    };
    let measured = Frame {
        x: fact.x,
        y: fact.y,
        w: fact.w,
        h: fact.h,
    };
    json!({
        "expected": [origin.0, origin.1],
        "screen": report::frame_json(screen.frame),
        "visible_frame": report::frame_json(screen.visible),
        "measured": report::fact_json(&fact),
        "delta": [fact.x - origin.0, fact.y - origin.1],
        "frame_ok": native::appkit_local_ok(measured, expected, 2.0),
    })
}

fn place_form(apps: &Apps, above: bool, file: &'static str, notes: &mut Vec<String>) -> Value {
    let handle = &apps.form;
    let prev = bus::bus().focus_len();
    let planned = cycle::on_ui(handle, move |panel, _| {
        let screen = native::pointer_screen()?;
        let caret = if above {
            Caret {
                x: screen.frame.x + 220.0,
                y: screen.frame.y + screen.frame.h - 28.0,
                w: 2.0,
                h: 16.0,
            }
        } else {
            Caret {
                x: screen.frame.x + 220.0,
                y: screen.frame.y + 140.0,
                w: 2.0,
                h: 16.0,
            }
        };
        let origin = place::form_origin(screen.frame, Some(caret), 220.0);
        let local = place::to_local(screen.frame, origin);
        panel.me.place(
            DisplayTarget::UnderPointer,
            OverlayPlacement::Rect {
                x: local.0 as f32,
                y: local.1 as f32,
                width: place::FORM_W as f32,
                height: 220.0,
            },
        );
        panel.me.show();
        Some((origin, caret, screen.frame))
    })
    .flatten();
    let _ = bus::bus().wait_focus(Kind::Form.name(), true, prev, Duration::from_millis(800));
    thread::sleep(Duration::from_millis(50));
    let fact = cycle::on_ui(handle, |_, _| native::fact(Kind::Form.title()))
        .unwrap_or_else(native::missing);
    let shot = cycle::capture(handle, Kind::Form.title(), file);
    if shot.is_none() {
        notes.push(format!("{file} screenshot failed"));
    }
    cycle::hide_wait(handle, Kind::Form);
    let (origin, caret, screen) = planned.unzip3();
    json!({
        "above": above,
        "caret": caret.map(|c| json!([c.x, c.y, c.w, c.h])),
        "screen": screen.map(report::frame_json),
        "expected": origin.map(|p| json!([p.0, p.1])),
        "measured": [fact.x, fact.y, fact.w, fact.h],
        "delta": origin.map(|p| json!([fact.x - p.0, fact.y - p.1])),
        "screenshot": shot,
    })
}

trait Unzip3<A, B, C> {
    fn unzip3(self) -> (Option<A>, Option<B>, Option<C>);
}

impl<A, B, C> Unzip3<A, B, C> for Option<(A, B, C)> {
    fn unzip3(self) -> (Option<A>, Option<B>, Option<C>) {
        match self {
            Some((a, b, c)) => (Some(a), Some(b), Some(c)),
            None => (None, None, None),
        }
    }
}

pub fn resize_form(apps: &Apps, _notes: &mut Vec<String>) -> Value {
    let _ = cycle::show_cycle(&apps.form, Kind::Form, false);
    let mut samples = Vec::new();
    for height in [240.0, 500.0, 900.0] {
        let want = place::clamp_form_height(height);
        let before = cycle::on_ui(&apps.form, |_, _| native::fact(Kind::Form.title()))
            .unwrap_or_else(native::missing);
        let _ = cycle::on_ui(&apps.form, move |_, tx| {
            tx.resize_window_instant((place::FORM_W as f32, want as f32))
        });
        let after = cycle::poll_height(&apps.form, want);
        let top_held = (after.y - before.y).abs() < 2.0;
        samples.push(json!({
            "asked": height,
            "clamped": want,
            "before_y": before.y,
            "after_y": after.y,
            "top_held": top_held,
            "after": report::fact_json(&after),
        }));
    }
    cycle::hide_wait(&apps.form, Kind::Form);
    json!(samples)
}

pub fn material_and_shadow(apps: &Apps, notes: &mut Vec<String>) -> Value {
    let _ = cycle::on_ui(&apps.picker, |panel, _| panel.material = true);
    let _ = cycle::show_cycle(&apps.picker, Kind::Picker, false);
    thread::sleep(Duration::from_millis(160));
    let before = cycle::on_ui(&apps.picker, |_, _| native::fact(Kind::Picker.title()))
        .unwrap_or_else(native::missing);
    let mut forced = false;
    if !before.shadow {
        forced = cycle::on_ui(&apps.picker, |_, _| {
            native::set_shadow(Kind::Picker.title(), true)
        })
        .unwrap_or(false);
        notes.push("launcher hasShadow was false after adopt; the spike set it".to_string());
    }
    thread::sleep(Duration::from_millis(80));
    let after = cycle::on_ui(&apps.picker, |_, _| native::fact(Kind::Picker.title()))
        .unwrap_or_else(native::missing);
    let shot = cycle::capture(&apps.picker, Kind::Picker.title(), "picker-material.png");
    if shot.is_none() {
        notes.push("picker material screenshot failed".to_string());
    }
    cycle::hide_wait(&apps.picker, Kind::Picker);
    let _ = cycle::show_cycle(&apps.palette, Kind::Palette, false);
    thread::sleep(Duration::from_millis(80));
    let palette = cycle::on_ui(&apps.palette, |_, _| native::fact(Kind::Palette.title()))
        .unwrap_or_else(native::missing);
    let palette_shot = cycle::capture(&apps.palette, Kind::Palette.title(), "palette-shadow.png");
    cycle::hide_wait(&apps.palette, Kind::Palette);
    json!({
        "before": report::fact_json(&before),
        "shadow_forced": forced,
        "after": report::fact_json(&after),
        "screenshot": shot,
        "palette": report::fact_json(&palette),
        "palette_screenshot": palette_shot,
    })
}

pub fn fullscreen(apps: &Apps, notes: &mut Vec<String>) -> Value {
    apps.host.show();
    apps.host.focus();
    if !cycle::poll_visible(
        &apps.palette,
        HOST_TITLE,
        |fact| fact.found && fact.visible,
        2000,
    ) {
        notes.push("host window did not show".to_string());
    }
    let entered =
        cycle::on_ui(&apps.palette, |_, _| native::toggle_fullscreen(HOST_TITLE)).unwrap_or(false);
    let full = cycle::poll_visible(&apps.palette, HOST_TITLE, |fact| fact.fullscreen, 5000);
    let _ = cycle::show_cycle(&apps.palette, Kind::Palette, false);
    thread::sleep(Duration::from_millis(200));
    let panel = cycle::on_ui(&apps.palette, |_, _| native::fact(Kind::Palette.title()))
        .unwrap_or_else(native::missing);
    let host = cycle::on_ui(&apps.palette, |_, _| native::fact(HOST_TITLE))
        .unwrap_or_else(native::missing);
    let shot = cycle::capture(
        &apps.palette,
        Kind::Palette.title(),
        "fullscreen-palette.png",
    );
    cycle::hide_wait(&apps.palette, Kind::Palette);
    let _ = cycle::on_ui(&apps.palette, |_, _| native::toggle_fullscreen(HOST_TITLE));
    let left = cycle::poll_visible(
        &apps.palette,
        HOST_TITLE,
        |fact| fact.found && !fact.fullscreen,
        3000,
    );
    apps.host.close();
    if !full {
        notes.push(
            "in-process fullscreen did not engage; Space coverage is the collection mask only"
                .to_string(),
        );
    }
    json!({
        "entered": entered,
        "became_fullscreen": full,
        "panel": report::fact_json(&panel),
        "host": report::fact_json(&host),
        "left": left,
        "screenshot": shot,
        "same_app": "the host is this process, weaker than another app's fullscreen space",
    })
}

pub fn first_mouse(apps: &Apps, _notes: &mut Vec<String>) -> Value {
    let _ = cycle::on_ui(&apps.form, |panel, _| panel.dismiss_on_blur = false);
    let _ = cycle::show_cycle(&apps.form, Kind::Form, false);
    let resigned =
        cycle::on_ui(&apps.form, |_, _| native::resign_key(Kind::Form.title())).unwrap_or(false);
    thread::sleep(Duration::from_millis(40));
    let before = cycle::on_ui(&apps.form, |_, _| native::fact(Kind::Form.title()))
        .unwrap_or_else(native::missing);
    let clicks_before = bus::bus().clicks();
    let accepts = cycle::on_ui(&apps.form, |_, _| native::send_click(Kind::Form.title())).flatten();
    thread::sleep(Duration::from_millis(80));
    let after = cycle::on_ui(&apps.form, |_, _| native::fact(Kind::Form.title()))
        .unwrap_or_else(native::missing);
    let clicks = bus::bus().clicks().saturating_sub(clicks_before);
    cycle::hide_wait(&apps.form, Kind::Form);
    json!({
        "resigned": resigned,
        "key_before_click": before.key,
        "accepts_first_mouse": before.accepts_first_mouse,
        "accepts_with_event": accepts,
        "clicks_delivered": clicks,
        "key_after_click": after.key,
        "ax": native::ax_trusted(),
        "window_server_click": "not posted; a global click could land on the owner's desktop",
    })
}
