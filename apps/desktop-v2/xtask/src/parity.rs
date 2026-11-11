//! `cargo xtask parity`: render Rune scenes the way S02 captured the Tauri app, then compare.
//!
//! - **Screens.** Every window scene that has a reference state of the same name in
//!   `docs/migration/reference/metrics.json` is rendered at the reference viewport with the
//!   system font, and its landmarks (role, accessible name, occurrence) are matched with the
//!   reference's. Gates: each landmark rect within 2 pt, each opaque background equal.
//! - **Gallery.** Every `gallery` scene is rendered and its samples are measured. A sample is
//!   an element keyed `parity:<window>:<scene>:<role>:<occurrence>:<name>` (measured against
//!   that reference landmark: size within 2 pt, background equal), `spec:<what>:<w>x<h>` (against
//!   the size the Tauri component's classes give; `_` for a side the content decides) or
//!   `info:<what>` (reported, not gated: rune-kit's own metrics, PLAN §6 D6).
//!
//! Writes `report.md`, `report.json` and side-by-side HTML sheets to `target/parity/` (or
//! `--out`). The sheets need the reference PNGs, which live outside git at
//! `$KUDZU_REFERENCE_DIR` (default `~/.local/share/kudzu-migration/reference/`); without them
//! the comparison still runs and the sheets are skipped with a message.

use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};

mod report;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The landmark tolerance in points (PLAN §1.7).
const TOLERANCE: f32 = 2.0;
/// Colour channel tolerance: the CSS and Rune OKLCH → sRGB conversions round independently.
const CHANNEL: i32 = 2;

const USAGE: &str = "usage: cargo xtask parity [--window W] [--scene S] [--theme light|dark] \
                     [--out DIR] [--no-build] [--report-only]";

struct Options {
    window: Option<String>,
    scene: Option<String>,
    themes: Vec<String>,
    out: PathBuf,
    build: bool,
    gate: bool,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn options(args: &[String]) -> Result<Options> {
    let mut o = Options {
        window: None,
        scene: None,
        themes: vec!["light".into(), "dark".into()],
        out: root().join("target/parity"),
        build: true,
        gate: true,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{a} needs a value\n{USAGE}"))
        };
        match a.as_str() {
            "--window" => o.window = Some(val()?),
            "--scene" => o.scene = Some(val()?),
            "--theme" => o.themes = vec![val()?],
            "--out" => o.out = PathBuf::from(val()?),
            "--no-build" => o.build = false,
            "--report-only" => o.gate = false,
            _ => return Err(format!("unknown argument {a}\n{USAGE}").into()),
        }
    }
    Ok(o)
}

/// The reference PNG folder (D12), when present.
fn reference_pngs() -> Option<PathBuf> {
    let dir = std::env::var_os("KUDZU_REFERENCE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".local/share/kudzu-migration/reference"))
        })?;
    dir.is_dir().then_some(dir)
}

/// One measured element: a reference landmark or a Rune element.
#[derive(Clone, Debug, PartialEq)]
struct Landmark {
    role: String,
    name: String,
    occurrence: usize,
    rect: [f32; 4],
    /// Background as 8-bit sRGB plus alpha, when it paints anything.
    fill: Option<[f32; 4]>,
}

struct RefState {
    window: String,
    scene: String,
    theme: String,
    file: String,
    viewport: (f32, f32),
    landmarks: Vec<Landmark>,
}

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn rgba(v: &Value) -> Option<[f32; 4]> {
    let c = v.as_array()?;
    let c = [f(&c[0]), f(&c[1]), f(&c[2]), f(&c[3])];
    (c[3] > 0.0).then_some(c)
}

fn load_reference(path: &Path) -> Result<Vec<RefState>> {
    let m: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let (styles, colors) = (&m["tables"]["styles"], &m["tables"]["colors"]);
    let states = m["states"].as_array().ok_or("metrics.json has no states")?;
    Ok(states
        .iter()
        .map(|s| RefState {
            window: s["window"].as_str().unwrap_or_default().into(),
            scene: s["scene"].as_str().unwrap_or_default().into(),
            theme: s["theme"].as_str().unwrap_or_default().into(),
            file: s["file"].as_str().unwrap_or_default().into(),
            viewport: (f(&s["viewport"]["width"]), f(&s["viewport"]["height"])),
            landmarks: s["landmarks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| {
                    let r = &l[3];
                    let style = &styles[l[4].as_u64().unwrap_or(0) as usize];
                    Landmark {
                        role: l[0].as_str().unwrap_or_default().into(),
                        name: l[1].as_str().unwrap_or_default().into(),
                        occurrence: l[2].as_u64().unwrap_or(1) as usize,
                        rect: [f(&r[0]), f(&r[1]), f(&r[2]), f(&r[3])],
                        fill: rgba(&colors[style[6].as_u64().unwrap_or(0) as usize]),
                    }
                })
                .collect(),
        })
        .collect())
}

/// `#rrggbbaa` → 8-bit channels and alpha; transparent → None.
fn hex(v: &Value) -> Option<[f32; 4]> {
    let s = v.as_str()?.strip_prefix('#')?;
    let n = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok().map(f32::from);
    let c = [n(0)?, n(2)?, n(4)?, n(6)? / 255.0];
    (c[3] > 0.0).then_some(c)
}

fn rect(v: &Value) -> [f32; 4] {
    [f(&v[0]), f(&v[1]), f(&v[2]), f(&v[3])]
}

/// An unnamed textbox (a field's box): its path, rect and fill.
type FieldBox<'a> = (&'a str, [f32; 4], Option<[f32; 4]>);

/// The tree's named controls in tree order, numbered per role and name. A named textbox inside
/// an unnamed one is the editor inside a field's box: it measures as the box, as the web's
/// `<input>` does.
fn landmarks(tree: &Value) -> Vec<Landmark> {
    let els = tree["elements"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut boxes: Vec<FieldBox> = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut out = Vec::new();
    for e in els {
        let (role, path) = (
            e["role"].as_str().unwrap_or(""),
            e["path"].as_str().unwrap_or(""),
        );
        let label = e["label"].as_str().filter(|l| !l.is_empty());
        if role == "textbox" && label.is_none() {
            boxes.push((path, rect(&e["rect"]), hex(&e["fill"])));
            continue;
        }
        let (Some(name), true) = (label, ROLES.contains(&role)) else {
            continue;
        };
        let (mut r, mut fill) = (rect(&e["rect"]), hex(&e["fill"]));
        if role == "textbox" {
            if let Some(b) = boxes
                .iter()
                .rev()
                .find(|b| path.starts_with(&format!("{}/", b.0)))
            {
                (r, fill) = (b.1, b.2);
            }
        }
        let n = seen.entry((role.into(), name.into())).or_default();
        *n += 1;
        out.push(Landmark {
            role: role.into(),
            name: name.into(),
            occurrence: *n,
            rect: r,
            fill,
        });
    }
    out
}

/// The roles `metrics.json` records.
const ROLES: &[&str] = &[
    "button",
    "region",
    "radio",
    "heading",
    "textbox",
    "navigation",
    "complementary",
    "switch",
    "template-chip",
    "contentinfo",
    "dialog",
    "combobox",
    "banner",
];

/// Equal after premultiplying: the reference reads colours back through an 8-bit premultiplied
/// canvas, so a faint fill's channels are lost (`foreground/[0.025]` reads as black) while what
/// it paints is kept.
fn colour_eq(a: Option<[f32; 4]>, b: Option<[f32; 4]>) -> bool {
    let pre =
        |c: Option<[f32; 4]>| c.map_or([0.0; 4], |c| [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]);
    let (a, b) = (pre(a), pre(b));
    (0..3).all(|i| (a[i] - b[i]).abs() <= CHANNEL as f32 + 0.5) && (a[3] - b[3]).abs() <= 0.012
}

fn delta(a: [f32; 4], b: [f32; 4], size_only: bool) -> f32 {
    let from = if size_only { 2 } else { 0 };
    (from..4).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max)
}

fn fmt_fill(c: Option<[f32; 4]>) -> String {
    match c {
        None => "none".into(),
        Some(c) => format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], c[3]),
    }
}

/// One compared item for the report.
struct Row {
    page: String,
    theme: String,
    item: String,
    expected: String,
    got: String,
    delta: Option<f32>,
    fill: (String, String),
    gated: bool,
    pass: bool,
    crops: Option<(String, [f32; 4], [f32; 4])>,
}

fn screen_rows(r: &RefState, tree: &Value) -> Vec<Row> {
    let got = landmarks(tree);
    let mut rows = Vec::new();
    for want in &r.landmarks {
        let found = got.iter().find(|g| {
            g.role == want.role && g.name == want.name && g.occurrence == want.occurrence
        });
        let item = format!("{} “{}” #{}", want.role, want.name, want.occurrence);
        let (got_s, d, gf, pass) = match found {
            Some(g) => {
                let d = delta(want.rect, g.rect, false);
                let pass = d <= TOLERANCE && colour_eq(want.fill, g.fill);
                (format!("{:?}", g.rect), Some(d), fmt_fill(g.fill), pass)
            }
            None => ("missing".into(), None, "-".into(), false),
        };
        rows.push(Row {
            page: format!("{}/{}", r.window, r.scene),
            theme: r.theme.clone(),
            item,
            expected: format!("{:?}", want.rect),
            got: got_s,
            delta: d,
            fill: (fmt_fill(want.fill), gf),
            gated: true,
            pass,
            crops: None,
        });
    }
    rows
}

/// What a gallery sample's key asks for.
#[derive(Debug, PartialEq)]
enum Expect {
    Reference {
        window: String,
        scene: String,
        role: String,
        occurrence: usize,
        name: String,
    },
    Spec {
        what: String,
        w: Option<f32>,
        h: Option<f32>,
    },
    Info(String),
}

fn parse_key(key: &str) -> Option<Expect> {
    if let Some(rest) = key.strip_prefix("parity:") {
        let p: Vec<&str> = rest.splitn(5, ':').collect();
        let [window, scene, role, occ, name] = p.as_slice() else {
            return None;
        };
        return Some(Expect::Reference {
            window: (*window).into(),
            scene: (*scene).into(),
            role: (*role).into(),
            occurrence: occ.parse().ok()?,
            name: (*name).into(),
        });
    }
    if let Some(rest) = key.strip_prefix("spec:") {
        let (what, size) = rest.rsplit_once(':')?;
        let (w, h) = size.split_once('x')?;
        let side = |s: &str| {
            if s == "_" {
                Some(None)
            } else {
                s.parse().ok().map(Some)
            }
        };
        return Some(Expect::Spec {
            what: what.into(),
            w: side(w)?,
            h: side(h)?,
        });
    }
    key.strip_prefix("info:").map(|w| Expect::Info(w.into()))
}

fn gallery_rows(scene: &str, theme: &str, tree: &Value, refs: &[RefState]) -> Vec<Row> {
    let els = tree["elements"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut rows = Vec::new();
    for e in els {
        let Some(expect) = e["key"].as_str().and_then(parse_key) else {
            continue;
        };
        let (got, fill) = (rect(&e["rect"]), hex(&e["fill"]));
        let size = format!("{}×{}", got[2], got[3]);
        let base = |item: String, expected: String, d: Option<f32>, gated: bool, pass: bool| Row {
            page: format!("gallery/{scene}"),
            theme: theme.into(),
            item,
            expected,
            got: size.clone(),
            delta: d,
            fill: ("-".into(), fmt_fill(fill)),
            gated,
            pass,
            crops: None,
        };
        rows.push(match expect {
            Expect::Reference {
                window,
                scene: s,
                role,
                occurrence,
                name,
            } => {
                let state = refs
                    .iter()
                    .find(|r| r.window == window && r.scene == s && r.theme == theme);
                let want = state.and_then(|r| {
                    r.landmarks
                        .iter()
                        .find(|l| l.role == role && l.name == name && l.occurrence == occurrence)
                });
                let item = format!("{window}/{s} {role} “{name}” #{occurrence}");
                match (state, want) {
                    (Some(st), Some(w)) => {
                        let d = delta(w.rect, got, true);
                        let pass = d <= TOLERANCE && colour_eq(w.fill, fill);
                        let mut row = base(
                            item,
                            format!("{}×{}", w.rect[2], w.rect[3]),
                            Some(d),
                            true,
                            pass,
                        );
                        row.fill.0 = fmt_fill(w.fill);
                        row.crops = Some((st.file.clone(), w.rect, got));
                        row
                    }
                    _ => base(item, "no such reference landmark".into(), None, true, false),
                }
            }
            Expect::Spec { what, w, h } => {
                let side = |v: Option<f32>| v.map_or("_".to_string(), |v| v.to_string());
                let d = [(w, got[2]), (h, got[3])]
                    .iter()
                    .filter_map(|(want, g)| want.map(|w| (w - g).abs()))
                    .fold(0.0, f32::max);
                base(
                    what,
                    format!("{}×{}", side(w), side(h)),
                    Some(d),
                    true,
                    d <= TOLERANCE,
                )
            }
            Expect::Info(what) => base(what, "rune-kit's own".into(), None, false, true),
        });
    }
    rows
}

fn render(
    bin: &Path,
    out: &Path,
    window: &str,
    scene: &str,
    theme: &str,
    size: Option<(f32, f32)>,
) -> Result<(PathBuf, Value)> {
    let stem = out.join(format!("render/{window}-{scene}.{theme}"));
    fs::create_dir_all(stem.parent().unwrap())?;
    let (png, tree) = (
        stem.with_extension(format!("{theme}.png")),
        stem.with_extension(format!("{theme}.json")),
    );
    let mut cmd = Command::new(bin);
    cmd.args([
        "--window", window, "--scene", scene, "--theme", theme, "--snap",
    ])
    .arg(&png)
    .arg("--tree")
    .arg(&tree)
    .env("RUNE_DIRS_ROOT", out.join("dirs"))
    .env_remove("KUDZU_FONTS");
    if let Some((w, h)) = size {
        cmd.args(["--size", &format!("{w}x{h}")]);
    }
    let result = cmd.output()?;
    if !result.status.success() {
        return Err(format!(
            "{window}/{scene} ({theme}) failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok((png, serde_json::from_str(&fs::read_to_string(&tree)?)?))
}

fn list(bin: &Path, args: &[&str]) -> Result<Vec<String>> {
    let out = Command::new(bin)
        .args(args)
        .env("RUNE_DIRS_ROOT", std::env::temp_dir())
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

pub fn run(args: &[String]) -> Result<()> {
    let o = options(args)?;
    let root = root();
    if o.build {
        let status = Command::new("cargo")
            .current_dir(&root)
            .args(["build", "-p", "kudzu"])
            .status()?;
        if !status.success() {
            return Err("cargo build -p kudzu failed".into());
        }
    }
    let bin = root.join("target/debug/kudzu");
    let refs = load_reference(&root.join("docs/migration/reference/metrics.json"))?;
    fs::create_dir_all(&o.out)?;
    let (mut rows, mut renders) = (Vec::new(), HashMap::new());
    for window in list(&bin, &["--list-windows"])? {
        if o.window.as_ref().is_some_and(|w| *w != window) {
            continue;
        }
        for scene in list(&bin, &["--window", &window, "--list-scenes"])? {
            if o.scene.as_ref().is_some_and(|s| *s != scene) {
                continue;
            }
            for theme in &o.themes {
                let reference = refs
                    .iter()
                    .find(|r| r.window == window && r.scene == scene && &r.theme == theme);
                if window != "gallery" && reference.is_none() {
                    println!("{window}/{scene}: no reference state; skipped");
                    continue;
                }
                let size = reference.map(|r| r.viewport);
                let (png, tree) = render(&bin, &o.out, &window, &scene, theme, size)?;
                let vp = &tree["viewport"];
                renders.insert(
                    format!("{window}/{scene}.{theme}"),
                    (png, (f(&vp[0]), f(&vp[1]))),
                );
                rows.extend(match reference {
                    Some(r) if window != "gallery" => screen_rows(r, &tree),
                    _ => gallery_rows(&scene, theme, &tree, &refs),
                });
            }
        }
    }
    fs::write(o.out.join("report.md"), report::report(&rows))?;
    let json_rows: Vec<Value> = rows
        .iter()
        .map(|r| json!({"page": r.page, "theme": r.theme, "item": r.item, "expected": r.expected,
            "got": r.got, "delta": r.delta, "fill": [r.fill.0, r.fill.1], "gated": r.gated, "pass": r.pass}))
        .collect();
    fs::write(
        o.out.join("report.json"),
        serde_json::to_string_pretty(&json_rows)?,
    )?;
    match reference_pngs() {
        Some(pngs) => {
            let html = report::sheet(&rows, &refs, &pngs, &renders, "Kudzu parity: reference | Rune");
            fs::write(o.out.join("sheet.html"), html)?;
            println!("side-by-side sheet: {}", o.out.join("sheet.html").display());
        }
        None => println!(
            "reference PNGs not found (set KUDZU_REFERENCE_DIR, default ~/.local/share/kudzu-migration/reference); \
             skipped the side-by-side sheet. Landmarks were still compared."
        ),
    }
    let failed: Vec<&Row> = rows.iter().filter(|r| r.gated && !r.pass).collect();
    println!(
        "{} compared, {} gated failures; report: {}",
        rows.len(),
        failed.len(),
        o.out.join("report.md").display()
    );
    if o.gate && !failed.is_empty() {
        return Err(format!("parity: {} gated rows failed", failed.len()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
