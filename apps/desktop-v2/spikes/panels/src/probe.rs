//! Live launcher-panel probe. An in-process trigger stands in for the global hotkey.

use std::thread;
use std::time::Duration;

use rune::{AppHandle, RunOptions};
use serde_json::{json, Value};

use crate::bus;
use crate::cycle::{self, Apps};
use crate::native;
use crate::panel::{self, Kind, Panel};
use crate::report;
use crate::scenes;

struct Quitter(AppHandle<Panel>);

impl Drop for Quitter {
    fn drop(&mut self) {
        self.0.quit();
    }
}

pub fn run() {
    let (apps, windows) = install();
    let apps_for_driver = Apps {
        palette: apps.palette.clone(),
        picker: apps.picker.clone(),
        form: apps.form.clone(),
        host: apps.host.clone(),
    };
    let driver = thread::spawn(move || {
        let _quit = Quitter(apps_for_driver.palette.clone());
        let report = drive(&apps_for_driver);
        report::write_report(&report);
        let visible = &report["cached"]["p95_visible_median"];
        let encode = &report["cached"]["p95_encode_median"];
        println!(
            "probe wrote evidence/report.json cached visible p95 median {visible} encode {encode}"
        );
    });
    let _watchdog = thread::spawn(|| {
        thread::sleep(Duration::from_secs(480));
        eprintln!("panels probe watchdog: exiting");
        std::process::exit(3);
    });
    rune::prewarm_gpu();
    rune::run_windows_with_args(
        RunOptions { accessory: true },
        std::iter::empty::<String>(),
        move |_| windows,
    );
    let _ = driver.join();
}

fn install() -> (Apps, Vec<rune::AnyWindowSpec>) {
    let (palette, palette_spec) = panel::panel(Kind::Palette);
    let (picker, picker_spec) = panel::panel(Kind::Picker);
    let (form, form_spec) = panel::panel(Kind::Form);
    let (host, host_spec) = panel::host();
    let windows = vec![
        palette_spec.boxed(),
        picker_spec.boxed(),
        form_spec.boxed(),
        host_spec.boxed(),
    ];
    (
        Apps {
            palette,
            picker,
            form,
            host,
        },
        windows,
    )
}

fn drive(apps: &Apps) -> Value {
    let mut notes = Vec::new();
    if !bus::bus().wait_ready(Duration::from_secs(30)) {
        notes.push("panels did not become ready within 30s".to_string());
    }
    let hidden = cycle::on_ui(&apps.palette, |_, _| native::fact(Kind::Palette.title()));
    let footprint = report::footprint_text();
    let screen = cycle::on_ui(&apps.palette, |_, _| native::pointer_screen()).flatten();
    let cached = measure_cached(apps, &mut notes);
    let fresh = measure_fresh(apps, &mut notes);
    let over = report::over_budget(&cached) || report::over_budget(&fresh);
    let fallback = if over {
        Some(measure_fallback(apps, &mut notes))
    } else {
        None
    };
    let keys = scenes::keys_and_blur(apps, &mut notes);
    let placed = scenes::placement(apps, &mut notes);
    let sized = scenes::resize_form(apps, &mut notes);
    let material = scenes::material_and_shadow(apps, &mut notes);
    let full = scenes::fullscreen(apps, &mut notes);
    let mouse = scenes::first_mouse(apps, &mut notes);
    cycle::hide_all(apps);
    json!({
        "host": report::host_info(),
        "trigger": "in-process AppHandle::show, not a Carbon global hotkey",
        "visible_poll_ms": cycle::VISIBLE_POLL.as_millis(),
        "profile": "dev, workspace opt-level 2, not release LTO",
        "rune": "29aacb688883a9712bc27175bdafd99fc6edb421",
        "budgets": { "encode_p95_ms": 16, "visible_ms": 80, "memory_mb": 150, "runs_per_batch": 20, "batches": 3 },
        "footprint": footprint,
        "hidden_palette": hidden.as_ref().map(report::fact_json),
        "screen": screen.as_ref().map(report::screen_json),
        "cached": cached,
        "fresh": fresh,
        "fallback": fallback,
        "keys": keys,
        "placement": placed,
        "resize": sized,
        "material": material,
        "fullscreen": full,
        "first_mouse": mouse,
        "notes": notes,
        "windows": cycle::on_ui(&apps.palette, |_, _| {
            native::inventory().iter().map(report::seen_json).collect::<Vec<_>>()
        }),
    })
}

fn measure_cached(apps: &Apps, notes: &mut Vec<String>) -> Value {
    let first = cycle::show_cycle(&apps.palette, Kind::Palette, false);
    if first.get("error").is_some() {
        notes.push("first show failed; skipping timed batches".to_string());
        return json!({
            "first_open": first,
            "batches": [],
            "p95_visible_median": Value::Null,
            "p95_encode_median": Value::Null,
        });
    }
    let first_chrome = cycle::on_ui(&apps.palette, |_, _| native::fact(Kind::Palette.title()));
    let mut hides = 0;
    let mut hide_losses = 0;
    if cycle::hide_wait(&apps.palette, Kind::Palette) {
        hide_losses += 1;
    }
    hides += 1;
    let _warmup = cycle::show_cycle(&apps.palette, Kind::Palette, false);
    if cycle::hide_wait(&apps.palette, Kind::Palette) {
        hide_losses += 1;
    }
    hides += 1;
    let mut batches = Vec::new();
    for index in 0..3 {
        let mut runs = Vec::new();
        for _ in 0..20 {
            let run = cycle::show_cycle(&apps.palette, Kind::Palette, false);
            let failed = run.get("error").is_some();
            runs.push(run);
            if failed {
                notes.push(format!("cached batch {index} show timed out"));
                break;
            }
            if cycle::hide_wait(&apps.palette, Kind::Palette) {
                hide_losses += 1;
            }
            hides += 1;
        }
        batches.push(report::batch_json(index, &runs));
        if runs.iter().any(|run| run["visible_ms"].is_null()) {
            notes.push(format!("cached batch {index} missed a visible sample"));
        }
    }
    json!({
        "first_open": first,
        "first_chrome": first_chrome.as_ref().map(report::fact_json),
        "batches": batches,
        "p95_visible_median": report::p95_median(&batches, "visible"),
        "p95_encode_median": report::p95_median(&batches, "encode"),
        "hides": hides,
        "hide_focus_losses": hide_losses,
    })
}

fn measure_fresh(apps: &Apps, notes: &mut Vec<String>) -> Value {
    let mut runs = Vec::new();
    let mut shot = None;
    let mut pixel = None;
    for index in 0..20 {
        runs.push(cycle::show_cycle(&apps.palette, Kind::Palette, true));
        if index == 0 {
            shot = cycle::capture(&apps.palette, Kind::Palette.title(), "fresh.png");
            pixel = shot.as_ref().and_then(|path| report::pixel_at(path));
            if shot.is_none() {
                notes.push("fresh screenshot failed".to_string());
            }
        }
        cycle::hide_wait(&apps.palette, Kind::Palette);
        let _ = cycle::on_ui(&apps.palette, |panel, _| panel.token = 0);
    }
    if runs.iter().any(|run| run["flashed"] == true) {
        notes.push("a fresh show put the stale token on screen".to_string());
    }
    json!({ "batch": report::batch_json(0, &runs), "screenshot": shot, "pixel": pixel })
}

fn measure_fallback(apps: &Apps, notes: &mut Vec<String>) -> Value {
    let set = cycle::on_ui(&apps.palette, |_, _| {
        native::set_animation_none(Kind::Palette.title())
    });
    if !set.unwrap_or(false) {
        notes.push("could not set animationBehavior".to_string());
    }
    let mut runs = Vec::new();
    for _ in 0..20 {
        runs.push(cycle::show_cycle(&apps.palette, Kind::Palette, false));
        cycle::hide_wait(&apps.palette, Kind::Palette);
    }
    let behavior = cycle::on_ui(&apps.palette, |_, _| {
        native::fact(Kind::Palette.title()).animation
    });
    json!({ "set": set, "animation": behavior, "batch": report::batch_json(0, &runs) })
}
