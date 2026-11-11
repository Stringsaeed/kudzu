//! The live probe: opens the main window as the plan configures it, drives it through
//! theme, material and active-state combinations, and writes the raw captures plus a JSON
//! record of what AppKit and Rune reported at each step.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use objc2::rc::Retained;
use objc2_app_kit::NSWindow;
use objc2_foundation::MainThreadMarker;
use rune::{App, AppHandle, AppSpec, Chrome, RunOptions, Tx, WindowOptions, WindowSpec};
use serde_json::{json, Value};

use crate::backdrop;
use crate::native;
use crate::scene::{self, Material, State};

static READY: AtomicBool = AtomicBool::new(false);

thread_local! {
    static BACKDROP: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    static HELPER: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
}

type Handle = AppHandle<State>;

fn ui<T: Send + 'static>(
    h: &Handle,
    f: impl FnOnce(&mut State, &mut Tx) -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::sync_channel(1);
    h.update(move |s, t| {
        let _ = tx.send(f(s, t));
    });
    rx.recv_timeout(Duration::from_secs(10)).ok()
}

fn wait(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

pub fn run(args: &[String]) {
    let out = PathBuf::from(flag(args, "--out").expect("--out DIR"));
    let report = PathBuf::from(flag(args, "--report").expect("--report FILE"));
    std::fs::create_dir_all(&out).expect("create the raw capture directory");

    // The plan's main window: full-size content, transparent, traffic lights at (18, 22).
    let chrome = Chrome {
        full_size_content: true,
        traffic_lights: Some((18.0, 22.0)),
        drag_regions: true,
    };
    let spec = AppSpec::new(native::TITLE, (1180.0, 760.0), State::new, scene::view);
    let app = App::new(spec).chrome(chrome).standard_menus();
    let handle = app.handle();
    let window = WindowSpec::from_app("main", app)
        .options(WindowOptions {
            transparent: true,
            min_size: Some((900.0, 560.0)),
            ..WindowOptions::default()
        })
        .on_ready(|_| READY.store(true, Ordering::SeqCst));

    let driver = {
        let handle = handle.clone();
        thread::spawn(move || {
            let result = drive(&handle, &out);
            let _ = ui(&handle, |_, _| {
                native::set_app_appearance(None);
                for slot in [&BACKDROP, &HELPER] {
                    slot.with(|b| {
                        if let Some(w) = b.borrow_mut().take() {
                            w.close();
                        }
                    });
                }
            });
            std::fs::write(
                &report,
                serde_json::to_string_pretty(&result).expect("serialise"),
            )
            .expect("write the report");
            println!("chrome probe wrote {}", report.display());
            handle.quit();
        })
    };
    let _watchdog = thread::spawn(|| {
        thread::sleep(Duration::from_secs(240));
        eprintln!("chrome probe watchdog: exiting");
        std::process::exit(3);
    });
    rune::run_windows_with_args(
        RunOptions { accessory: false },
        std::iter::empty::<String>(),
        move |_| vec![window.boxed()],
    );
    let _ = driver.join();
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn shoot(region: [f64; 4], path: &Path) -> bool {
    let r = format!("{},{},{},{}", region[0], region[1], region[2], region[3]);
    Command::new("screencapture")
        .args(["-x", "-R", &r])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success())
}

fn effect_summary(f: &Value) -> Value {
    f["effect_views"].clone()
}

fn drive(h: &Handle, out: &Path) -> Value {
    let mut notes: Vec<String> = Vec::new();
    for _ in 0..150 {
        if READY.load(Ordering::SeqCst) {
            break;
        }
        wait(100);
    }
    wait(500);
    let setup = ui(h, |_, _| {
        let w = native::window()?;
        native::place(&w);
        let mtm = MainThreadMarker::new()?;
        let b = backdrop::open(mtm, w.frame(), 70.0);
        BACKDROP.with(|slot| *slot.borrow_mut() = Some(b));
        let helper = native::open_helper(&w);
        HELPER.with(|slot| *slot.borrow_mut() = Some(helper));
        native::activate(&w);
        Some(native::facts(&w))
    })
    .flatten();
    let Some(initial) = setup else {
        return json!({ "error": "the spike window was not found" });
    };
    wait(900);
    let chrome_facts = ui(h, |_, _| native::window().map(|w| native::facts(&w)))
        .flatten()
        .unwrap_or(Value::Null);
    let frame = chrome_facts["frame_top_left"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|v| v.as_f64().unwrap_or(0.0))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let region = [frame[0], frame[1], frame[2], frame[3]];

    let mut captures = Vec::new();
    let mut appearance = Vec::new();
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        let builds_before = scene::BUILDS.load(Ordering::Relaxed);
        let _ = ui(h, move |_, _| native::set_app_appearance(Some(dark)));
        wait(900);
        appearance.push(json!({
            "app_appearance_set": theme,
            "system_is_dark": ui(h, |_, _| native::system_is_dark()),
            "cx_system_dark": scene::SAW_DARK.load(Ordering::Relaxed),
            "rebuilt": scene::BUILDS.load(Ordering::Relaxed) > builds_before,
            "facts": ui(h, |_, _| native::window().map(|w| native::facts(&w))).flatten(),
        }));
        for material in [Material::Popover, Material::NativeSidebar] {
            let name = if material == Material::Popover {
                "popover"
            } else {
                "sidebar"
            };
            let _ = ui(h, move |s, _| {
                s.material = material;
                if let Some(w) = native::window() {
                    native::set_sidebar_view(&w, material == Material::NativeSidebar);
                }
            });
            wait(500);
            for active in [true, false] {
                let state = if active { "active" } else { "inactive" };
                let _ = ui(h, move |_, _| {
                    if active {
                        if let Some(w) = native::window() {
                            native::activate(&w);
                        }
                    } else {
                        HELPER.with(|slot| {
                            if let Some(h) = slot.borrow().as_ref() {
                                native::make_key(h);
                            }
                        });
                    }
                });
                wait(1100);
                let f = ui(h, |_, _| native::window().map(|w| native::facts(&w)))
                    .flatten()
                    .unwrap_or(Value::Null);
                let file = out.join(format!("{theme}-{name}-{state}.png"));
                let ok = shoot(region, &file);
                captures.push(json!({
                    "theme": theme, "material": name, "state": state, "file": file.file_name().map(|n| n.to_string_lossy().into_owned()), "captured": ok,
                    "key": f["key"], "app_active": f["app_active"], "effect_views": effect_summary(&f),
                    "cx_system_dark": scene::SAW_DARK.load(Ordering::Relaxed),
                }));
            }
        }
    }

    // SplitView collapse (dark, Popover), and the reopen.
    let _ = ui(h, |s, _| {
        s.material = Material::Popover;
        if let Some(w) = native::window() {
            native::set_sidebar_view(&w, false);
            native::activate(&w);
        }
        s.sidebar.set_open(false);
    });
    wait(1200);
    let collapsed = ui(h, |s, _| {
        (
            s.sidebar.is_open(),
            native::window().map(|w| native::facts(&w)),
        )
    })
    .unwrap_or((true, None));
    let collapsed_file = out.join("dark-popover-collapsed.png");
    shoot(region, &collapsed_file);
    let _ = ui(h, |s, _| s.sidebar.set_open(true));
    wait(1000);

    // NSApp.appearance back to System.
    let before = scene::BUILDS.load(Ordering::Relaxed);
    let _ = ui(h, |_, _| native::set_app_appearance(None));
    wait(900);
    let reset = json!({
        "system_is_dark": ui(h, |_, _| native::system_is_dark()),
        "cx_system_dark": scene::SAW_DARK.load(Ordering::Relaxed),
        "rebuilt": scene::BUILDS.load(Ordering::Relaxed) > before,
    });

    // Toolbar inset in full screen. The window drops to level 0 for it, and a state bump
    // forces a rebuild so the inset is read from a view built in the new state.
    let inset_windowed = scene::SAW_INSET_X10.load(Ordering::Relaxed) as f64 / 10.0;
    let _ = ui(h, |_, _| {
        if let Some(w) = native::window() {
            native::set_level(&w, 0);
            native::activate(&w);
            native::toggle_fullscreen(&w);
        }
    });
    wait(3500);
    let _ = ui(h, |s, _| s.touch += 1);
    wait(500);
    let full = ui(h, |_, _| native::window().map(|w| native::facts(&w)))
        .flatten()
        .unwrap_or(Value::Null);
    let inset_full = scene::SAW_INSET_X10.load(Ordering::Relaxed) as f64 / 10.0;
    let _ = shoot_main(&out.join("fullscreen.png"));
    let _ = ui(h, |_, _| {
        native::window().map(|w| native::toggle_fullscreen(&w))
    });
    wait(3500);
    let _ = ui(h, |s, _| s.touch += 1);
    wait(500);
    let back = ui(h, |_, _| {
        native::window().map(|w| (native::is_fullscreen(&w), native::facts(&w)))
    })
    .flatten();
    let inset_back = scene::SAW_INSET_X10.load(Ordering::Relaxed) as f64 / 10.0;
    if back.as_ref().is_none_or(|b| b.0) {
        notes.push("the window did not leave full screen".to_string());
    }

    json!({
        "rune": "29aacb688883a9712bc27175bdafd99fc6edb421",
        "profile": "dev, workspace opt-level 2",
        "backdrop": "a borderless window under the spike window, painted with a fixed test pattern",
        "region_pts": region,
        "initial": initial,
        "after_settle": chrome_facts,
        "captures": captures,
        "appearance": appearance,
        "appearance_reset": reset,
        "collapse": { "sidebar_open_after_close": collapsed.0, "effect_views": collapsed.1.as_ref().map(effect_summary), "file": collapsed_file.file_name().map(|n| n.to_string_lossy().into_owned()) },
        "fullscreen": {
            "inset_windowed_pts": inset_windowed,
            "inset_full_screen_pts": inset_full,
            "inset_after_exit_pts": inset_back,
            "facts": full,
            "after_exit": back.map(|b| b.1),
        },
        "notes": notes,
    })
}

fn shoot_main(path: &Path) -> bool {
    Command::new("screencapture")
        .args(["-x", "-m"])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success())
}
