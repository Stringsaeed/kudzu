//! `chrome compose`: crops the sidebar from the raw captures and from the browser reference,
//! lays them side by side with labels, and measures a patch of the sidebar in each.

use std::path::{Path, PathBuf};

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use serde_json::{json, Value};

/// How much of the window's left edge a panel shows, in points, and the downscale applied.
const SIDE_PT: u32 = 280;
const SCALE: f32 = 0.5;
const GAP: u32 = 8;
const LABEL_H: u32 = 26;
const BG: Rgba<u8> = Rgba([22, 22, 24, 255]);
const INK: Rgba<u8> = Rgba([240, 240, 240, 255]);
/// A strip in the sidebar's left padding, in points, where neither the reference nor the
/// spike draws text or icons: x, y, width, height.
const PATCH: [u32; 4] = [4, 100, 10, 600];

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

pub fn run(args: &[String]) {
    let raw = PathBuf::from(flag(args, "--raw").expect("--raw DIR"));
    let reference = PathBuf::from(flag(args, "--reference").expect("--reference DIR"));
    let out = PathBuf::from(flag(args, "--out").expect("--out DIR"));
    std::fs::create_dir_all(&out).expect("create the output directory");
    let mut stats = serde_json::Map::new();
    for theme in ["light", "dark"] {
        let refer = load(&reference.join("main").join(format!("library.{theme}.png")));
        let pop = |s: &str| load(&raw.join(format!("{theme}-popover-{s}.png")));
        let side = |s: &str| load(&raw.join(format!("{theme}-sidebar-{s}.png")));
        let (pa, pi, sa, si) = (
            pop("active"),
            pop("inactive"),
            side("active"),
            side("inactive"),
        );
        let name = theme.to_uppercase();
        let top = [
            panel(&refer, "REFERENCE (BROWSER, OPAQUE)"),
            panel(&pa, "POPOVER + .72/.55 TINT"),
            panel(&sa, "NATIVE SIDEBAR MATERIAL + TINT"),
        ];
        let bottom = [
            blank(&refer, "NO INACTIVE STATE IN BROWSER"),
            panel(&pi, "POPOVER, INACTIVE"),
            panel(&si, "NATIVE SIDEBAR, INACTIVE"),
        ];
        let sheet = stack(&[
            row(&top, &format!("{name} - ACTIVE WINDOW")),
            row(&bottom, &format!("{name} - INACTIVE WINDOW (NOT KEY)")),
        ]);
        save(&sheet, &out.join(format!("sidebar-{theme}.png")));
        let mut t = serde_json::Map::new();
        for (key, img) in [
            ("reference", &refer),
            ("popover_active", &pa),
            ("sidebar_active", &sa),
            ("popover_inactive", &pi),
            ("sidebar_inactive", &si),
        ] {
            t.insert(key.to_string(), patch_stats(img));
        }
        stats.insert(theme.to_string(), Value::Object(t));
    }
    let dark_pop = load(&raw.join("dark-popover-active.png"));
    let dark_side = load(&raw.join("dark-sidebar-active.png"));
    let full = row(
        &[
            window(&dark_pop, "POPOVER, DARK"),
            window(&dark_side, "NATIVE SIDEBAR MATERIAL, DARK"),
        ],
        "WHOLE WINDOW, ACTIVE",
    );
    save(&full, &out.join("window-dark.png"));
    let collapsed = load(&raw.join("dark-popover-collapsed.png"));
    save(
        &row(
            &[window(
                &collapsed,
                "SPLITVIEW SIDEBAR COLLAPSED, POPOVER, DARK",
            )],
            "COLLAPSE",
        ),
        &out.join("collapse-dark.png"),
    );
    let lights = load(&raw.join("light-popover-active.png"));
    stats.insert(
        "traffic_lights_light_popover_active".to_string(),
        traffic_lights(&lights),
    );
    std::fs::write(
        out.join("stats.json"),
        serde_json::to_string_pretty(&Value::Object(stats)).expect("serialise"),
    )
    .expect("write stats");
}

fn load(path: &Path) -> RgbaImage {
    image::open(path)
        .unwrap_or_else(|e| panic!("open {}: {e}", path.display()))
        .to_rgba8()
}

fn save(img: &RgbaImage, path: &Path) {
    img.save(path).expect("save the image");
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    println!(
        "{} {}x{} {} bytes",
        path.display(),
        img.width(),
        img.height(),
        size
    );
}

/// Pixels per point of a capture of the 1180-point-wide window.
fn per_pt(img: &RgbaImage) -> f32 {
    img.width() as f32 / 1180.0
}

fn scaled(img: RgbaImage, factor: f32) -> RgbaImage {
    let (w, h) = (
        ((img.width() as f32) * factor) as u32,
        ((img.height() as f32) * factor) as u32,
    );
    imageops::resize(&img, w.max(1), h.max(1), FilterType::Lanczos3)
}

fn panel(img: &RgbaImage, label: &str) -> RgbaImage {
    let p = per_pt(img);
    let w = (SIDE_PT as f32 * p) as u32;
    let crop = imageops::crop_imm(img, 0, 0, w.min(img.width()), img.height()).to_image();
    labelled(scaled(crop, SCALE * 2.0 / p.max(1.0)), label)
}

fn window(img: &RgbaImage, label: &str) -> RgbaImage {
    let p = per_pt(img);
    labelled(scaled(img.clone(), 0.5 * 2.0 / p.max(1.0) * 0.5), label)
}

fn blank(like: &RgbaImage, label: &str) -> RgbaImage {
    let p = panel(like, "");
    let mut img = RgbaImage::from_pixel(p.width(), p.height(), Rgba([48, 48, 52, 255]));
    text(&mut img, 6, LABEL_H + 10, label, 1, INK);
    img
}

fn labelled(body: RgbaImage, label: &str) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(body.width(), body.height() + LABEL_H, BG);
    imageops::overlay(&mut out, &body, 0, LABEL_H as i64);
    text(&mut out, 6, 9, label, 1, INK);
    out
}

fn row(panels: &[RgbaImage], title: &str) -> RgbaImage {
    let w = panels.iter().map(|p| p.width()).sum::<u32>() + GAP * (panels.len() as u32 + 1);
    let h = panels.iter().map(|p| p.height()).max().unwrap_or(0) + 2 * GAP + LABEL_H;
    let mut out = RgbaImage::from_pixel(w, h, BG);
    text(&mut out, GAP, 9, title, 2, INK);
    let mut x = GAP;
    for p in panels {
        imageops::overlay(&mut out, p, x as i64, (GAP + LABEL_H) as i64);
        x += p.width() + GAP;
    }
    out
}

fn stack(rows: &[RgbaImage]) -> RgbaImage {
    let w = rows.iter().map(|r| r.width()).max().unwrap_or(0);
    let h = rows.iter().map(|r| r.height()).sum();
    let mut out = RgbaImage::from_pixel(w, h, BG);
    let mut y = 0;
    for r in rows {
        imageops::overlay(&mut out, r, 0, y as i64);
        y += r.height();
    }
    out
}

/// Mean and spread of the sidebar patch, in 8-bit sRGB.
fn patch_stats(img: &RgbaImage) -> Value {
    let p = per_pt(img);
    let [x, y, w, h] = PATCH.map(|v| (v as f32 * p) as u32);
    let (mut sum, mut sq, mut n) = ([0f64; 3], [0f64; 3], 0f64);
    for py in y..(y + h).min(img.height()) {
        for px in x..(x + w).min(img.width()) {
            let c = img.get_pixel(px, py).0;
            for i in 0..3 {
                sum[i] += f64::from(c[i]);
                sq[i] += f64::from(c[i]) * f64::from(c[i]);
            }
            n += 1.0;
        }
    }
    let mean: Vec<f64> = sum.iter().map(|s| (s / n * 10.0).round() / 10.0).collect();
    let sd: Vec<f64> = (0..3)
        .map(|i| ((sq[i] / n - (sum[i] / n).powi(2)).max(0.0).sqrt() * 10.0).round() / 10.0)
        .collect();
    json!({ "mean_rgb": mean, "stddev_rgb": sd })
}

/// The close button's red disc, found by colour in the window's top-left corner, in points
/// from the window's left and top edges.
fn traffic_lights(img: &RgbaImage) -> Value {
    let p = per_pt(img);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..(80.0 * p) as u32 {
        for x in 0..(80.0 * p) as u32 {
            let [r, g, b, _] = img.get_pixel(x, y).0;
            if r > 215 && g < 120 && b < 110 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    if x0 == u32::MAX {
        return Value::Null;
    }
    let pt = |v: u32| (f64::from(v) / f64::from(p) * 10.0).round() / 10.0;
    json!({ "close_left_pt": pt(x0), "close_top_pt": pt(y0), "close_diameter_pt": pt(x1 + 1 - x0) })
}

fn glyph(c: char) -> [&'static str; 7] {
    match c.to_ascii_uppercase() {
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'B' => [
            "11110", "10001", "10001", "11110", "10001", "10001", "11110",
        ],
        'C' => [
            "01110", "10001", "10000", "10000", "10000", "10001", "01110",
        ],
        'D' => [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
        'E' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
        'F' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "10000",
        ],
        'G' => [
            "01110", "10001", "10000", "10111", "10001", "10001", "01111",
        ],
        'H' => [
            "10001", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'I' => [
            "01110", "00100", "00100", "00100", "00100", "00100", "01110",
        ],
        'J' => [
            "00111", "00010", "00010", "00010", "00010", "10010", "01100",
        ],
        'K' => [
            "10001", "10010", "10100", "11000", "10100", "10010", "10001",
        ],
        'L' => [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
        'M' => [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
        'N' => [
            "10001", "11001", "10101", "10011", "10001", "10001", "10001",
        ],
        'O' => [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'P' => [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
        'Q' => [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
        'R' => [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
        'S' => [
            "01111", "10000", "10000", "01110", "00001", "00001", "11110",
        ],
        'T' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
        'U' => [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'V' => [
            "10001", "10001", "10001", "10001", "10001", "01010", "00100",
        ],
        'W' => [
            "10001", "10001", "10001", "10101", "10101", "11011", "10001",
        ],
        'X' => [
            "10001", "10001", "01010", "00100", "01010", "10001", "10001",
        ],
        'Y' => [
            "10001", "10001", "01010", "00100", "00100", "00100", "00100",
        ],
        'Z' => [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
        '0' => [
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
        '1' => [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
        '3' => [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        '4' => [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
        '5' => [
            "11111", "10000", "11110", "00001", "00001", "10001", "01110",
        ],
        '6' => [
            "00110", "01000", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => [
            "01110", "10001", "10001", "01111", "00001", "00010", "01100",
        ],
        '.' => [
            "00000", "00000", "00000", "00000", "00000", "01100", "01100",
        ],
        '/' => [
            "00001", "00010", "00010", "00100", "01000", "01000", "10000",
        ],
        '(' => [
            "00010", "00100", "01000", "01000", "01000", "00100", "00010",
        ],
        ')' => [
            "01000", "00100", "00010", "00010", "00010", "00100", "01000",
        ],
        ',' => [
            "00000", "00000", "00000", "00000", "01100", "00100", "01000",
        ],
        '-' => [
            "00000", "00000", "00000", "11111", "00000", "00000", "00000",
        ],
        '+' => [
            "00000", "00100", "00100", "11111", "00100", "00100", "00000",
        ],
        _ => ["00000"; 7],
    }
}

fn text(img: &mut RgbaImage, x: u32, y: u32, s: &str, scale: u32, ink: Rgba<u8>) {
    let mut cx = x;
    for c in s.chars() {
        for (ry, bits) in glyph(c).iter().enumerate() {
            for (rx, b) in bits.bytes().enumerate() {
                if b == b'1' {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let (px, py) =
                                (cx + rx as u32 * scale + dx, y + ry as u32 * scale + dy);
                            if px < img.width() && py < img.height() {
                                img.put_pixel(px, py, ink);
                            }
                        }
                    }
                }
            }
        }
        cx += 6 * scale;
    }
}
