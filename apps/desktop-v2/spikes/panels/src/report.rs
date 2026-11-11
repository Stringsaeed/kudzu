//! JSON evidence for the live probe. Screenshots stay out of git.

use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};

use crate::native::{self, Fact};
use crate::place::Frame;
use crate::stats;

pub fn evidence_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence")
}

pub fn write_report(report: &Value) {
    let dir = evidence_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("report.json"),
        serde_json::to_string_pretty(report).unwrap_or_default(),
    );
}

pub fn footprint_text() -> String {
    let output = Command::new("footprint")
        .args(["-p", &std::process::id().to_string()])
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .lines()
            .take(16)
            .collect::<Vec<_>>()
            .join("\n"),
        Err(err) => err.to_string(),
    }
}

pub fn host_info() -> Value {
    let sw = Command::new("sw_vers").output();
    let text = sw
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).to_string())
        .unwrap_or_default();
    let rustc = Command::new("rustc").arg("--version").output();
    let rustc = rustc
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default();
    json!({ "sw_vers": text, "arch": std::env::consts::ARCH, "rustc": rustc })
}

pub fn pixel_at(path: &str) -> Option<Value> {
    let image = image::open(path).ok()?;
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let p = rgba.get_pixel(w / 2, h / 2);
    Some(json!({ "w": w, "h": h, "rgba": [p[0], p[1], p[2], p[3]] }))
}

pub fn fact_json(fact: &Fact) -> Value {
    json!({
        "found": fact.found,
        "number": fact.number,
        "alpha": fact.alpha,
        "key": fact.key,
        "visible": fact.visible,
        "on_space": fact.on_space,
        "shadow": fact.shadow,
        "level": fact.level,
        "behavior": fact.behavior,
        "behavior_names": native::behavior_names(fact.behavior),
        "animation": fact.animation,
        "style": fact.style,
        "frame": [fact.x, fact.y, fact.w, fact.h],
        "radius": fact.radius,
        "effect_class": fact.effect_class,
        "material": fact.material,
        "blending": fact.blending,
        "effect_state": fact.effect_state,
        "can_key": fact.can_key,
        "ignores_mouse": fact.ignores_mouse,
        "accepts_first_mouse": fact.accepts_first_mouse,
        "fullscreen": fact.fullscreen,
        "class": fact.class_name,
    })
}

pub fn seen_json(seen: &native::WindowSeen) -> Value {
    json!({
        "title": seen.title,
        "class": seen.class_name,
        "level": seen.level,
        "style": seen.style,
        "behavior": seen.behavior,
        "behavior_names": native::behavior_names(seen.behavior),
        "visible": seen.visible,
        "alpha": seen.alpha,
        "number": seen.number,
        "sharing": seen.sharing,
        "has_content": seen.has_content,
        "key": seen.key,
    })
}

pub fn front_json(front: &native::Front) -> Value {
    json!({ "bundle": front.bundle, "pid": front.pid, "app_active": front.app_active, "policy": front.policy, "ax": front.ax })
}

pub fn screen_json(screen: &native::Screen) -> Value {
    json!({
        "frame": frame_json(screen.frame),
        "visible": frame_json(screen.visible),
        "scale": screen.scale,
    })
}

pub fn frame_json(frame: Frame) -> Value {
    json!([frame.x, frame.y, frame.w, frame.h])
}

pub fn batch_json(index: usize, runs: &[Value]) -> Value {
    json!({
        "index": index,
        "visible": spread_json(&nums(runs, "visible_ms")),
        "encode": spread_json(&nums(runs, "encode_ms")),
        "key": spread_json(&nums(runs, "key_ms")),
        "runs": runs,
    })
}

pub fn p95_median(batches: &[Value], key: &str) -> Value {
    let p95s: Vec<f64> = batches
        .iter()
        .filter_map(|batch| batch[key]["p95"].as_f64())
        .collect();
    json!(stats::median_of(&p95s))
}

pub fn over_budget(series: &Value) -> bool {
    let visible = series["p95_visible_median"]
        .as_f64()
        .or_else(|| series["batch"]["visible"]["p95"].as_f64());
    let encode = series["p95_encode_median"]
        .as_f64()
        .or_else(|| series["batch"]["encode"]["p95"].as_f64());
    visible.is_some_and(|ms| ms > 80.0) || encode.is_some_and(|ms| ms > 16.0)
}

fn nums(runs: &[Value], key: &str) -> Vec<f64> {
    runs.iter().filter_map(|run| run[key].as_f64()).collect()
}

fn spread_json(values: &[f64]) -> Value {
    match stats::spread(values) {
        Some(spread) => {
            json!({ "n": spread.n, "min": spread.min, "median": spread.median, "p95": spread.p95, "max": spread.max })
        }
        None => Value::Null,
    }
}
