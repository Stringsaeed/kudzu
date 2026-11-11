//! JSON evidence. Screenshots stay out of git.

use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};

use crate::native::{self, Fact};

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

pub fn fact_json(fact: &Fact) -> Value {
    json!({
        "found": fact.found,
        "number": fact.number,
        "visible": fact.visible,
        "on_space": fact.on_space,
        "level": fact.level,
        "behavior": fact.behavior,
        "behavior_names": native::behavior_names(fact.behavior),
        "style": fact.style,
        "alpha": fact.alpha,
        "ignores_mouse": fact.ignores_mouse,
        "shadow": fact.shadow,
        "class": fact.class_name,
        "frame": [fact.x, fact.y, fact.w, fact.h],
        "radius": fact.radius,
    })
}
