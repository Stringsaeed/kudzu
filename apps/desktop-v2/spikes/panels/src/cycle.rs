//! Show and hide clock for one launcher panel.
//!
//! `t0` is taken inside the update that calls `show`, before AppKit orders the panel in.
//! A cached show may encode nothing new; that encode sample is absent, not a failure.
//! Visible time uses `on_focus` when alpha is already at least 0.95, otherwise an 8 ms poll.

use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use rune::{AppHandle, Tx};
use serde_json::{json, Value};

use crate::bus::{self, FrameNote};
use crate::native::{self, Fact};
use crate::panel::{Host, Kind, Panel};
use crate::report;

pub const VISIBLE_POLL: Duration = Duration::from_millis(8);

pub struct Apps {
    pub palette: AppHandle<Panel>,
    pub picker: AppHandle<Panel>,
    pub form: AppHandle<Panel>,
    pub host: AppHandle<Host>,
}

pub fn show_cycle(handle: &AppHandle<Panel>, kind: Kind, fresh: bool) -> Value {
    let title = kind.title();
    let name = kind.name();
    let focus_prev = bus::bus().focus_len();
    let frame_prev = bus::bus().frame_len();
    let t0 = on_ui(handle, move |panel, _| {
        if fresh {
            panel.token = 1;
        }
        let t0 = Instant::now();
        panel.me.show();
        t0
    });
    let Some(t0) = t0 else {
        return json!({ "error": "show update timed out" });
    };
    let focused = bus::bus().wait_focus(name, true, focus_prev, Duration::from_secs(2));
    let key = bus::bus().focus_since(name, true, focus_prev);
    let mut visible_at = key
        .as_ref()
        .filter(|note| note.snap.alpha >= 0.95 && note.snap.visible)
        .map(|note| note.at);
    let mut polled = false;
    if visible_at.is_none() {
        polled = true;
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(900) {
            let fact =
                on_ui(handle, move |_, _| native::fact(title)).unwrap_or_else(native::missing);
            if fact.alpha >= 0.95 && fact.visible {
                visible_at = Some(Instant::now());
                break;
            }
            thread::sleep(VISIBLE_POLL);
        }
    }
    let frames = bus::bus().frames_since(name, frame_prev, t0);
    let drawn: Vec<&FrameNote> = frames
        .iter()
        .filter(|frame| frame.outcome == "drawn")
        .collect();
    let encode = drawn.first().map(|frame| ms(t0, frame.at));
    let stale_visible = drawn
        .iter()
        .any(|frame| frame.alpha > 0.05 && frame.token == 0);
    let fresh_frame = drawn.iter().any(|frame| frame.token == 1);
    json!({
        "focused": focused,
        "key_ms": key.as_ref().map(|note| ms(t0, note.at)),
        "visible_ms": visible_at.map(|at| ms(t0, at)),
        "polled_alpha": polled,
        "alpha_at_key": key.as_ref().map(|note| note.snap.alpha),
        "app_active_at_key": key.as_ref().map(|note| note.snap.app_active),
        "front_at_key": key.as_ref().map(|note| note.snap.front.clone()),
        "pid_at_key": key.as_ref().map(|note| note.snap.pid),
        "visible_at_key": key.as_ref().map(|note| note.snap.visible),
        "on_space_at_key": key.as_ref().map(|note| note.snap.on_space),
        "key_window": key.as_ref().map(|note| note.snap.key),
        "encode_ms": encode,
        "drawn": drawn.len(),
        "encode_cost_ms": drawn.first().and_then(|frame| frame.encode_ms),
        "flashed": fresh && (stale_visible || !fresh_frame),
        "token_at_last_drawn": drawn.last().map(|frame| frame.token),
    })
}

pub fn hide_wait(handle: &AppHandle<Panel>, kind: Kind) -> bool {
    let title = kind.title();
    let prev = bus::bus().focus_len();
    handle.hide();
    let lost = bus::bus().wait_focus(kind.name(), false, prev, Duration::from_millis(800));
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(800) {
        let fact = on_ui(handle, move |_, _| native::fact(title));
        if fact
            .as_ref()
            .is_some_and(|fact| fact.alpha < 0.05 || !fact.visible)
        {
            return lost;
        }
        thread::sleep(VISIBLE_POLL);
    }
    lost
}

pub fn poll_height(handle: &AppHandle<Panel>, want: f64) -> Fact {
    let start = Instant::now();
    let mut last = native::missing();
    while start.elapsed() < Duration::from_millis(700) {
        last =
            on_ui(handle, |_, _| native::fact(Kind::Form.title())).unwrap_or_else(native::missing);
        if (last.h - want).abs() < 2.0 {
            break;
        }
        thread::sleep(Duration::from_millis(16));
    }
    last
}

pub fn poll_visible(
    handle: &AppHandle<Panel>,
    title: &'static str,
    pred: impl Fn(&Fact) -> bool,
    ms_limit: u64,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms_limit) {
        let fact = on_ui(handle, move |_, _| native::fact(title)).unwrap_or_else(native::missing);
        if pred(&fact) {
            return true;
        }
        thread::sleep(Duration::from_millis(30));
    }
    false
}

pub fn hide_all(apps: &Apps) {
    apps.palette.hide();
    apps.picker.hide();
    apps.form.hide();
    apps.host.hide();
}

pub fn on_ui<T: Send + 'static>(
    handle: &AppHandle<Panel>,
    f: impl FnOnce(&mut Panel, &mut Tx) -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::sync_channel(1);
    handle.update(move |panel, ui| {
        let _ = tx.send(f(panel, ui));
    });
    rx.recv_timeout(Duration::from_secs(8)).ok()
}

pub fn capture(handle: &AppHandle<Panel>, title: &'static str, file: &str) -> Option<String> {
    let fact = on_ui(handle, move |_, _| native::fact(title))?;
    if fact.number <= 0 {
        return None;
    }
    let dir = report::evidence_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(file);
    let window_ok = shoot(&["-x", "-l", &fact.number.to_string()], &path);
    let blank = window_ok
        && report::pixel_at(&path.display().to_string())
            .is_none_or(|pixel| pixel["rgba"][3].as_u64() == Some(0));
    if !blank && window_ok {
        return Some(path.display().to_string());
    }
    let stem = path.file_stem()?.to_string_lossy();
    let screen = dir.join(format!("{stem}-screen.png"));
    let region = format!(
        "{},{},{},{}",
        (fact.x - 40.0).max(0.0),
        (fact.y - 40.0).max(0.0),
        fact.w + 80.0,
        fact.h + 80.0
    );
    shoot(&["-x", "-R", &region], &screen).then(|| screen.display().to_string())
}

fn shoot(args: &[&str], path: &std::path::Path) -> bool {
    let mut child = match Command::new("screencapture").args(args).arg(path).spawn() {
        Ok(child) => child,
        Err(_) => return false,
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < Duration::from_secs(4) => {
                thread::sleep(Duration::from_millis(40));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

pub fn ms(from: Instant, to: Instant) -> f64 {
    to.saturating_duration_since(from).as_secs_f64() * 1000.0
}

pub fn empty_front() -> native::Front {
    native::Front {
        bundle: String::new(),
        pid: -1,
        app_active: false,
        policy: -1,
        ax: false,
    }
}
