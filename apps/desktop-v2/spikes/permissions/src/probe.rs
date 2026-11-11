//! Live companion probe. Opens the Accessibility privacy pane, never grants or revokes, never
//! drops the icon into the list. Quits System Settings only if this process opened it.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use rune::{AppHandle, OverlayHandle, OverlayPlacement, RunOptions};
use serde_json::{json, Value};

use crate::companion::{self, Companion};
use crate::drag::{ICON, ICON_X, ICON_Y};
use crate::guide::Guide;
use crate::native::{self, Fact};
use crate::pack;
use crate::place::{self, Rect};
use crate::report;

struct Quitter(AppHandle<Companion>);

impl Drop for Quitter {
    fn drop(&mut self) {
        companion::on_ui(&self.0, |_, _| {
            native::hide_native_guide();
            native::close_titled(native::COMPANION_TITLE);
            native::close_titled(native::GUIDE_TITLE);
            native::close_titled("kudzu-spike-guide-native");
            native::close_titled("permissions");
        });
        self.0.quit();
    }
}

pub fn run() {
    let (handle, spec, overlay) = companion::install();
    let driver_handle = handle.clone();
    let driver_overlay = overlay.clone();
    let driver = thread::spawn(move || {
        let _quit = Quitter(driver_handle.clone());
        let report = drive(&driver_handle, &driver_overlay);
        report::write_report(&report);
        println!(
            "probe wrote evidence/report.json overlay_decision={}",
            report["guide"]["decision"]
        );
    });
    let _watchdog = thread::spawn(|| {
        thread::sleep(Duration::from_secs(120));
        eprintln!("permissions probe watchdog: exiting");
        std::process::exit(3);
    });
    rune::prewarm_gpu();
    rune::run_windows_with_args(
        RunOptions { accessory: true },
        std::iter::empty::<String>(),
        move |_| vec![spec.boxed()],
    );
    let _ = driver.join();
}

fn drive(handle: &AppHandle<Companion>, overlay: &OverlayHandle<Guide>) -> Value {
    let mut notes = Vec::new();
    let (ready, window_error, shown_via) = wait_ready(handle);
    if !ready {
        notes.push("companion did not become ready within 30s".into());
        if let Some(err) = &window_error {
            notes.push(format!("window_error: {err}"));
        }
    }
    let windows_at_ready = companion::on_ui(handle, |_, _| native::inventory()).unwrap_or_default();
    let bundled = pack::already_bundled();
    let bundle = companion::on_ui(handle, |s, _| s.bundle.clone()).flatten();
    let before = companion::on_ui(handle, |_, _| native::fact(native::COMPANION_TITLE))
        .unwrap_or_else(native::missing);
    let settings_was_running =
        companion::on_ui(handle, |_, _| native::settings_running()).unwrap_or(false);
    let opened = companion::on_ui(handle, |_, _| native::open_settings()).unwrap_or(false);
    if !opened {
        notes.push("NSWorkspace failed to open the Accessibility privacy pane".into());
    }
    thread::sleep(Duration::from_millis(450));
    let mut destination = None;
    for _ in 0..12 {
        destination = companion::on_ui(handle, |_, _| native::settings_frame()).flatten();
        if destination.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    let dest = destination.unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    });
    let available = companion::on_ui(handle, move |_, _| {
        native::visible_frame_containing((dest.mid_x(), dest.mid_y()))
    })
    .unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: 1440.0,
        h: 900.0,
    });
    let expected = place::companion_frame(dest, available);
    let origin_set = if dest.is_empty() {
        false
    } else {
        companion::on_ui(handle, move |_, _| {
            native::set_origin(native::COMPANION_TITLE, expected.x, expected.y)
        })
        .unwrap_or(false)
    };
    thread::sleep(Duration::from_millis(80));
    let placed = companion::on_ui(handle, |_, _| native::fact(native::COMPANION_TITLE))
        .unwrap_or_else(native::missing);
    let desktop_top = companion::on_ui(handle, |_, _| native::desktop_top()).unwrap_or(0.0);
    let (ox, oy, ow, oh) = place::canvas_to_overlay_placement(expected, desktop_top);
    handle.place(
        rune::DisplayTarget::Primary,
        OverlayPlacement::Rect {
            x: ox,
            y: oy,
            width: ow,
            height: oh,
        },
    );
    thread::sleep(Duration::from_millis(80));
    let after_place = companion::on_ui(handle, |_, _| native::fact(native::COMPANION_TITLE))
        .unwrap_or_else(native::missing);
    let front = companion::on_ui(handle, |_, _| native::front()).unwrap_or(native::Front {
        bundle: String::new(),
        pid: -1,
        app_active: false,
    });
    let reduce = companion::on_ui(handle, |_, _| native::reduce_motion()).unwrap_or(false);
    let companion_rect = placed.appkit();
    let canvas = if dest.is_empty() {
        companion_rect.inset(place::CANVAS_PAD)
    } else {
        place::canvas(companion_rect, dest)
    };
    let source = (
        companion_rect.x + ICON_X as f64 + ICON as f64 / 2.0,
        companion_rect.max_y() - ICON_Y as f64 - ICON as f64 / 2.0,
    );
    companion::place_overlay(overlay, canvas, desktop_top);
    let animate = place::should_animate(companion_rect, dest, reduce);
    companion::on_ui(handle, move |s, _| {
        s.overlay.update(move |guide, tx| {
            crate::guide::arm(guide, canvas, dest, source, reduce);
            tx.wake_after(Duration::from_secs_f64(1.0 / 30.0));
        });
    });
    if animate {
        overlay.show();
        thread::sleep(Duration::from_millis(200));
    }
    let overlay_fact = companion::on_ui(handle, |_, _| native::fact(native::GUIDE_TITLE))
        .unwrap_or_else(native::missing);
    let mut native_guide = None;
    let mut decision = "overlay";
    if animate && overlay_fact.found && !overlay_fact.on_space {
        decision = "native";
        native_guide = companion::on_ui(handle, move |_, _| native::show_native_guide(canvas));
        notes.push("Rune overlay was not on the active Space; native NSPanel used".into());
    } else if !animate {
        notes.push("guide skipped (overlap, empty Settings, or Reduce Motion)".into());
        decision = "skipped";
    } else if !overlay_fact.found {
        decision = "native";
        native_guide = companion::on_ui(handle, move |_, _| native::show_native_guide(canvas));
        notes.push("Rune overlay window was not listed; native NSPanel used".into());
    }
    thread::sleep(Duration::from_millis(400));
    let overlay_live = companion::on_ui(handle, |_, _| native::fact(native::GUIDE_TITLE))
        .unwrap_or_else(native::missing);
    let drag = drag_proof(handle, &bundle, &placed, &mut notes);
    let presses = companion::on_ui(handle, |s, _| s.presses).unwrap_or(0);
    let drag_end_count = companion::on_ui(handle, |s, _| s.drag_ends.len()).unwrap_or(0);
    overlay.hide();
    companion::on_ui(handle, |_, _| native::hide_native_guide());
    let reveal = reveal_proof(handle, &bundle, &mut notes);
    if opened && !settings_was_running {
        companion::on_ui(handle, |_, _| native::quit_settings());
        notes.push("quit System Settings because this probe opened it".into());
    } else if settings_was_running {
        notes.push("left System Settings running; it was already open".into());
    }
    json!({
        "host": report::host_info(),
        "rune": "29aacb688883a9712bc27175bdafd99fc6edb421",
        "bundle_id": crate::pack::BUNDLE_ID,
        "nsbundle_id": crate::bundle::bundle_id(),
        "bundled": bundled,
        "bundle": bundle.as_ref().map(|p| p.display().to_string()),
        "ready": ready,
        "shown_via": shown_via,
        "window_error": window_error,
        "windows": windows_at_ready,
        "settings_was_running": settings_was_running,
        "opened_settings_url": opened,
        "settings_url": native::SETTINGS_URL,
        "destination": rect_json(dest),
        "available": rect_json(available),
        "expected": rect_json(expected),
        "origin_set": origin_set,
        "before": report::fact_json(&before),
        "placed": report::fact_json(&placed),
        "after_rune_place": report::fact_json(&after_place),
        "place_delta": [
            after_place.x - placed.x,
            after_place.y - placed.y,
        ],
        "front_while_settings": {
            "bundle": front.bundle,
            "pid": front.pid,
            "app_active": front.app_active,
        },
        "companion_visible_with_settings": placed.visible && placed.alpha >= 0.95 && placed.found,
        "companion_on_space": placed.on_space,
        "companion_level": placed.level,
        "companion_collection": native::behavior_names(placed.behavior),
        "reduce_motion": reduce,
        "canvas": rect_json(canvas),
        "guide": {
            "decision": decision,
            "animate": animate,
            "overlay": report::fact_json(&overlay_fact),
            "overlay_live": report::fact_json(&overlay_live),
            "native": native_guide.as_ref().map(report::fact_json),
        },
        "drag": drag,
        "presses": presses,
        "drag_end_count": drag_end_count,
        "reveal": reveal,
        "notes": notes,
    })
}

fn wait_ready(handle: &AppHandle<Companion>) -> (bool, Option<String>, &'static str) {
    // App::windowless defers native creation until show/open. Wait until the
    // event loop answers an update, then ask for the window.
    let loop_up = Instant::now();
    while loop_up.elapsed() < Duration::from_secs(8) {
        if companion::on_ui_for(handle, Duration::from_millis(200), |_, _| ()).is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    handle.show();
    let mut via = "show";
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if companion::READY.load(Ordering::SeqCst) {
            return (true, handle.window_error(), via);
        }
        if start.elapsed() > Duration::from_millis(250) {
            let found = companion::on_ui_for(handle, Duration::from_millis(200), |_, _| {
                native::fact(native::COMPANION_TITLE).found
            })
            .unwrap_or(false);
            if found {
                companion::READY.store(true, Ordering::SeqCst);
                return (true, handle.window_error(), via);
            }
        }
        if via == "show" && start.elapsed() > Duration::from_secs(2) {
            handle.open();
            via = "open";
        }
        thread::sleep(Duration::from_millis(50));
    }
    (false, handle.window_error(), via)
}

fn rect_json(r: Rect) -> Value {
    json!([r.x, r.y, r.w, r.h])
}

fn drag_proof(
    handle: &AppHandle<Companion>,
    bundle: &Option<std::path::PathBuf>,
    placed: &Fact,
    notes: &mut Vec<String>,
) -> Value {
    let Some(bundle) = bundle.clone() else {
        notes.push("no running .app; pasteboard proof used a private board only if packed".into());
        return json!({ "offered": false, "reason": "not a bundle" });
    };
    let private = companion::on_ui(handle, {
        let bundle = bundle.clone();
        move |_, _| native::prove_pasteboard(&bundle)
    });
    let can_post = companion::on_ui(handle, |_, _| native::can_post_events()).unwrap_or(false);
    if !can_post {
        notes.push("CGPreflightPostEventAccess is false; no live drag, no prompt".into());
        return json!({
            "offered": true,
            "bundle": bundle.display().to_string(),
            "can_post_events": false,
            "live_session": false,
            "private_pasteboard": private,
            "note": "owner must drag the icon into the list; this process did not",
        });
    }
    if !placed.found || placed.w < 10.0 {
        notes.push("companion frame missing; skipped live drag".into());
        return json!({
            "offered": true,
            "can_post_events": true,
            "live_session": false,
            "private_pasteboard": private,
        });
    }
    let cx = placed.x + ICON_X as f64 + ICON as f64 / 2.0;
    let cy_top = ICON_Y as f64 + ICON as f64 / 2.0;
    let screen_y = placed.y + placed.h - cy_top;
    companion::on_ui(handle, move |_, _| {
        native::warp(cx, screen_y);
        native::post_mouse(
            native::COMPANION_TITLE,
            &[
                (1, ICON_X as f64 + 10.0, ICON_Y as f64 + 10.0),
                (6, ICON_X as f64 + 12.0, ICON_Y as f64 + 10.0),
                (6, ICON_X as f64 + 30.0, ICON_Y as f64 + 10.0),
            ],
        );
    });
    let mut board = (None, None);
    for _ in 0..40 {
        thread::sleep(Duration::from_millis(50));
        board = companion::on_ui(handle, |_, _| native::drag_pasteboard()).unwrap_or((None, None));
        if board.0.is_some() {
            break;
        }
    }
    companion::on_ui(handle, |_, _| {
        native::post_mouse(
            native::COMPANION_TITLE,
            &[(2, ICON_X as f64 + 30.0, ICON_Y as f64 + 10.0)],
        );
        native::release_real_button();
    });
    thread::sleep(Duration::from_millis(200));
    let ends = companion::last_ends();
    json!({
        "offered": true,
        "bundle": bundle.display().to_string(),
        "can_post_events": true,
        "live_session": true,
        "released_on_companion": true,
        "never_over_settings": true,
        "pasteboard_url": board.0,
        "pasteboard_path": board.1,
        "private_pasteboard": private,
        "on_drag_end": ends,
    })
}

fn reveal_proof(
    handle: &AppHandle<Companion>,
    bundle: &Option<std::path::PathBuf>,
    notes: &mut Vec<String>,
) -> Value {
    let Some(bundle) = bundle.clone() else {
        return json!({ "revealed": false, "reason": "not a bundle" });
    };
    let finder_was = companion::on_ui(handle, |_, _| native::finder_running()).unwrap_or(true);
    let revealed = companion::on_ui(handle, {
        let bundle = bundle.clone();
        move |_, _| native::reveal_app(&bundle)
    })
    .unwrap_or(false);
    thread::sleep(Duration::from_millis(400));
    if finder_was {
        companion::on_ui(handle, |_, _| native::close_finder_front_window());
        notes.push("closed the Finder window this reveal opened".into());
    } else {
        companion::on_ui(handle, |_, _| native::quit_finder());
        notes.push("quit Finder because this probe launched it".into());
    }
    json!({
        "revealed": revealed,
        "bundle": bundle.display().to_string(),
        "finder_was_running": finder_was,
    })
}
