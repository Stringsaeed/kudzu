//! `cargo xtask parity`: render Rune scenes the way S02 captured the Tauri app, then compare.
//!
//! - **Screens.** Every reference state (`docs/migration/reference/STATES.md` and
//!   `metrics.json`, which must agree) is either rendered at its viewport with the system font
//!   and its landmarks compared, or listed in `reference::NOT_YET_BUILT` with the step that builds
//!   it. A state that is neither fails, and so does a mark on a scene that now exists.
//! - **Gallery.** Every `gallery` scene is rendered and its keyed samples measured:
//!   `parity:<window>:<scene>:<role>:<occurrence>:<name>` against that reference landmark,
//!   `spec:<what>:<w>x<h>` against the Tauri classes' size, `info:<what>` reported only (D6). A
//!   gallery scene with no samples fails unless `reference::GALLERY_UNMEASURED` names it.
//! - **Checks** (`compare`): geometry within 2 pt; background and text colour per 8-bit sRGB
//!   channel; padding as the content's inset; child landmarks by offset. Font size and weight,
//!   radius, border, shadow, focus ring and knob shape are not in Rune's `--tree` (or not in
//!   `metrics.json`), so the report lists them as needing side-by-side review.
//! - **Fails closed.** Every command's exit status is checked; an empty window or gallery
//!   listing, a filter that matches nothing, or a run that compares nothing is an error.
//!
//! Writes `report.md`, `report.json` and `sheet.html` (full-screen pairs and crops, from the
//! reference PNGs at `$KUDZU_REFERENCE_DIR`, default `~/.local/share/kudzu-migration/reference/`;
//! skipped with a message when they are absent, as in CI) to `target/parity/` or `--out`.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

mod compare;
mod reference;
mod report;
mod tree;

use compare::{check, review, Kind, Row};
use reference::{Pending, RefState};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const USAGE: &str = "usage: cargo xtask parity [--window W] [--scene S] [--theme light|dark] \
                     [--out DIR] [--no-build] [--report-only]";

#[derive(Debug, PartialEq)]
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
            "--theme" => match val()?.as_str() {
                t @ ("light" | "dark") => o.themes = vec![t.into()],
                t => return Err(format!("--theme is light or dark, not {t}").into()),
            },
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

/// Runs `cmd`, failing on a non-zero exit, and returns its stdout.
fn run_checked(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{cmd:?} exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Every window the binary has, with its scenes.
type Listing = BTreeMap<String, Vec<String>>;

fn list(bin: &Path, out: &Path) -> Result<Listing> {
    let cmd = |args: &[&str]| {
        let mut c = Command::new(bin);
        c.args(args).env("RUNE_DIRS_ROOT", out.join("dirs"));
        c
    };
    let windows = lines(&run_checked(&mut cmd(&["--list-windows"]))?);
    windows
        .into_iter()
        .map(|w| {
            Ok((
                w.clone(),
                lines(&run_checked(&mut cmd(&["--window", &w, "--list-scenes"]))?),
            ))
        })
        .collect()
}

/// One unit of work.
#[derive(Debug, PartialEq)]
enum Job {
    /// Render a reference state's scene and compare its landmarks.
    Screen {
        window: String,
        scene: String,
        theme: String,
    },
    /// Render a gallery scene and measure its samples.
    Gallery { scene: String, theme: String },
    /// A failing or reported row decided without rendering.
    Row(Row),
}

fn plan_row(page: String, theme: &str, item: &str, kind: Kind, check: compare::Check) -> Job {
    Job::Row(Row {
        page,
        theme: theme.into(),
        item: item.into(),
        kind,
        checks: vec![check],
        crop: None,
    })
}

/// Every comparison the options ask for, with nothing silently left out.
fn plan(
    o: &Options,
    refs: &[RefState],
    listing: &Listing,
    pending: &[Pending],
) -> Result<Vec<Job>> {
    let windows: BTreeSet<&str> = listing
        .keys()
        .map(String::as_str)
        .chain(refs.iter().map(|r| r.window.as_str()))
        .collect();
    if listing.is_empty() {
        return Err("the binary lists no windows".into());
    }
    if let Some(w) = &o.window {
        if !windows.contains(w.as_str()) {
            return Err(format!("--window {w}: no such window (have {windows:?})").into());
        }
    }
    let selected = |w: &str| o.window.as_deref().is_none_or(|x| x == w);
    if selected("gallery") && listing.get("gallery").is_none_or(Vec::is_empty) {
        return Err("the gallery window is missing or lists no scenes".into());
    }
    if let Some(s) = &o.scene {
        let known = listing
            .iter()
            .filter(|(w, _)| selected(w))
            .any(|(_, ss)| ss.contains(s))
            || refs.iter().any(|r| selected(&r.window) && &r.scene == s);
        if !known {
            return Err(format!("--scene {s}: no such scene in the selected windows").into());
        }
    }
    let scene_ok = |s: &str| o.scene.as_deref().is_none_or(|x| x == s);
    let listed = |w: &str, s: &str| listing.get(w).is_some_and(|ss| ss.iter().any(|x| x == s));
    let mut jobs = Vec::new();
    for r in refs
        .iter()
        .filter(|r| selected(&r.window) && scene_ok(&r.scene) && o.themes.contains(&r.theme))
    {
        let page = format!("{}/{}", r.window, r.scene);
        match (
            listed(&r.window, &r.scene),
            reference::pending_step(pending, &r.window, &r.scene),
        ) {
            (true, None) => jobs.push(Job::Screen {
                window: r.window.clone(),
                scene: r.scene.clone(),
                theme: r.theme.clone(),
            }),
            (true, Some(step)) => jobs.push(plan_row(
                page,
                &r.theme,
                "scene",
                Kind::Gated,
                check(
                    "not-yet-built mark",
                    format!("removed once built ({step})"),
                    "the scene exists but is still marked".into(),
                    false,
                ),
            )),
            (false, Some(step)) => jobs.push(plan_row(
                page,
                &r.theme,
                "scene",
                Kind::Pending,
                review("built by", step, "not yet built"),
            )),
            (false, None) => jobs.push(plan_row(
                page,
                &r.theme,
                "scene",
                Kind::Gated,
                check(
                    "rendered",
                    "a scene of this name".into(),
                    "missing, and not marked not yet built".into(),
                    false,
                ),
            )),
        }
    }
    for (w, scenes) in listing.iter().filter(|(w, _)| selected(w)) {
        for s in scenes.iter().filter(|s| scene_ok(s)) {
            for theme in &o.themes {
                if w == "gallery" {
                    jobs.push(Job::Gallery {
                        scene: s.clone(),
                        theme: theme.clone(),
                    });
                } else if !refs.iter().any(|r| &r.window == w && &r.scene == s) {
                    jobs.push(plan_row(
                        format!("{w}/{s}"),
                        theme,
                        "scene",
                        Kind::Info,
                        review("reference", "-", "no reference state: not compared"),
                    ));
                }
            }
        }
    }
    if !jobs
        .iter()
        .any(|j| matches!(j, Job::Screen { .. } | Job::Gallery { .. }))
    {
        return Err("nothing to compare: every selected scene is not yet built".into());
    }
    Ok(jobs)
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
    run_checked(&mut cmd)?;
    if !png.is_file() {
        return Err(format!("{window}/{scene} ({theme}) wrote no PNG").into());
    }
    Ok((png, serde_json::from_str(&fs::read_to_string(&tree)?)?))
}

fn viewport(tree: &Value) -> Result<(f32, f32)> {
    let v = tree["viewport"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or("the --tree output has no viewport")?;
    Ok((
        v[0].as_f64().unwrap_or(0.0) as f32,
        v[1].as_f64().unwrap_or(0.0) as f32,
    ))
}

pub fn run(args: &[String]) -> Result<()> {
    let o = options(args)?;
    let root = root();
    if o.build {
        run_checked(
            Command::new("cargo")
                .current_dir(&root)
                .args(["build", "-p", "kudzu"]),
        )?;
    }
    let bin = root.join("target/debug/kudzu");
    let dir = root.join("docs/migration/reference");
    let refs = reference::parse_metrics(&fs::read_to_string(dir.join("metrics.json"))?)?;
    reference::check_consistent(
        &reference::parse_states_md(&fs::read_to_string(dir.join("STATES.md"))?)?,
        &refs,
    )?;
    reference::check_pending(reference::NOT_YET_BUILT, &refs)?;
    fs::create_dir_all(&o.out)?;
    let listing = list(&bin, &o.out)?;
    let (mut rows, mut renders) = (Vec::new(), BTreeMap::new());
    for job in plan(&o, &refs, &listing, reference::NOT_YET_BUILT)? {
        match job {
            Job::Row(r) => rows.push(r),
            Job::Screen {
                window,
                scene,
                theme,
            } => {
                let r = refs
                    .iter()
                    .find(|r| r.window == window && r.scene == scene && r.theme == theme)
                    .expect("planned from refs");
                let (png, tree) = render(&bin, &o.out, &window, &scene, &theme, Some(r.viewport))?;
                rows.extend(compare::screen_rows(r, &tree::elements(&tree)?));
                let reference = Some((r.file.clone(), r.viewport));
                renders.insert(
                    format!("{window}/{scene}.{theme}"),
                    report::Render {
                        png,
                        viewport: viewport(&tree)?,
                        reference,
                    },
                );
            }
            Job::Gallery { scene, theme } => {
                let (png, tree) = render(&bin, &o.out, "gallery", &scene, &theme, None)?;
                rows.extend(compare::gallery_rows(
                    &scene,
                    &theme,
                    &tree::elements(&tree)?,
                    &refs,
                ));
                renders.insert(
                    format!("gallery/{scene}.{theme}"),
                    report::Render {
                        png,
                        viewport: viewport(&tree)?,
                        reference: None,
                    },
                );
            }
        }
    }
    fs::write(o.out.join("report.md"), report::markdown(&rows))?;
    fs::write(
        o.out.join("report.json"),
        serde_json::to_string_pretty(&report::json(&rows))?,
    )?;
    match reference_pngs() {
        Some(pngs) => {
            let viewports = refs.iter().map(|r| (r.file.clone(), r.viewport)).collect();
            fs::write(o.out.join("sheet.html"), report::sheet(&rows, &pngs, &renders, &viewports))?;
            println!("side-by-side sheet: {}", o.out.join("sheet.html").display());
        }
        None => println!(
            "reference PNGs not found (set KUDZU_REFERENCE_DIR, default ~/.local/share/kudzu-migration/reference); \
             skipped the side-by-side sheet. Landmarks and styles were still compared."
        ),
    }
    let failed = rows.iter().filter(|r| r.failed()).count();
    let pending = rows.iter().filter(|r| r.kind == Kind::Pending).count();
    println!(
        "{} rows, {failed} failed, {pending} not yet built; report: {}",
        rows.len(),
        o.out.join("report.md").display()
    );
    if o.gate && failed > 0 {
        return Err(format!("parity: {failed} rows failed").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
