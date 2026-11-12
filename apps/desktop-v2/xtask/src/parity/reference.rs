//! The S02 reference: `metrics.json` (landmarks and their computed styles), `STATES.md` (the
//! scene list) and the scenes the new app has not built yet, each with the step that builds it.
//! Everything here is strict: a malformed row is an error, never a default.

use std::collections::BTreeSet;

use serde_json::Value;

use super::Result;

/// sRGB channels 0–255 and alpha 0–1, as `metrics.json` stores colours.
pub(super) type Rgba = [f32; 4];

/// The computed style `metrics.json` records for a landmark (`styleColumns`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Style {
    pub font_size: f32,
    pub font_weight: f32,
    /// Corner radii in points; `None` where the CSS used another unit (a pill's `%`).
    pub radius: [Option<f32>; 4],
    /// Top, right, bottom, left.
    pub padding: [f32; 4],
    /// `None` when transparent.
    pub color: Option<Rgba>,
    pub background: Option<Rgba>,
    pub shadow: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Landmark {
    pub role: String,
    pub name: String,
    pub occurrence: usize,
    pub rect: [f32; 4],
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RefState {
    pub window: String,
    pub scene: String,
    pub theme: String,
    pub file: String,
    pub viewport: (f32, f32),
    pub landmarks: Vec<Landmark>,
}

fn num(v: &Value, what: &str) -> Result<f32> {
    v.as_f64()
        .map(|n| n as f32)
        .ok_or_else(|| format!("metrics.json: {what} is not a number: {v}").into())
}

fn text<'a>(v: &'a Value, what: &str) -> Result<&'a str> {
    v.as_str()
        .ok_or_else(|| format!("metrics.json: {what} is not a string: {v}").into())
}

fn rect(v: &Value, what: &str) -> Result<[f32; 4]> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or(format!("metrics.json: {what} is not [x, y, w, h]"))?;
    Ok([
        num(&a[0], what)?,
        num(&a[1], what)?,
        num(&a[2], what)?,
        num(&a[3], what)?,
    ])
}

fn colour(colors: &Value, id: &Value) -> Result<Option<Rgba>> {
    let i = id
        .as_u64()
        .ok_or("metrics.json: a colour id is not an index")? as usize;
    let c = colors[i]
        .as_array()
        .filter(|c| c.len() == 4)
        .ok_or(format!("metrics.json: colour {i} is missing"))?;
    let c = [
        num(&c[0], "colour")?,
        num(&c[1], "colour")?,
        num(&c[2], "colour")?,
        num(&c[3], "colour")?,
    ];
    Ok((c[3] > 0.0).then_some(c))
}

fn style(tables: &Value, id: &Value) -> Result<Style> {
    let i = id
        .as_u64()
        .ok_or("metrics.json: a style id is not an index")? as usize;
    let s = tables["styles"][i]
        .as_array()
        .filter(|s| s.len() == 8)
        .ok_or(format!("metrics.json: style {i} is missing"))?;
    let sides = |v: &Value, what: &str| -> Result<Vec<Value>> {
        v.as_array()
            .filter(|a| a.len() == 4)
            .cloned()
            .ok_or_else(|| format!("metrics.json: style {i} {what}").into())
    };
    let radius = sides(&s[3], "radius")?;
    let padding = sides(&s[4], "padding")?;
    let shadow = s[7]
        .as_u64()
        .and_then(|n| tables["shadows"][n as usize].as_str())
        .ok_or(format!("metrics.json: style {i} shadow"))?;
    Ok(Style {
        font_size: num(&s[1], "fontSize")?,
        font_weight: num(&s[2], "fontWeight")?,
        // Pills record `calc(infinity * 1px)` as a huge number: no finite radius to compare.
        radius: [0, 1, 2, 3].map(|k| {
            radius[k]
                .as_f64()
                .map(|r| r as f32)
                .filter(|r| *r < 10_000.0)
        }),
        padding: [
            num(&padding[0], "padding")?,
            num(&padding[1], "padding")?,
            num(&padding[2], "padding")?,
            num(&padding[3], "padding")?,
        ],
        color: colour(&tables["colors"], &s[5])?,
        background: colour(&tables["colors"], &s[6])?,
        shadow: shadow.into(),
    })
}

pub(super) fn parse_metrics(json: &str) -> Result<Vec<RefState>> {
    let m: Value = serde_json::from_str(json)?;
    if m["schemaVersion"] != 2 {
        return Err("metrics.json: expected schemaVersion 2".into());
    }
    let tables = &m["tables"];
    let states = m["states"]
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or("metrics.json has no states")?;
    states
        .iter()
        .map(|s| {
            let landmarks = s["landmarks"]
                .as_array()
                .ok_or("metrics.json: a state has no landmarks array")?;
            Ok(RefState {
                window: text(&s["window"], "window")?.into(),
                scene: text(&s["scene"], "scene")?.into(),
                theme: text(&s["theme"], "theme")?.into(),
                file: text(&s["file"], "file")?.into(),
                viewport: (
                    num(&s["viewport"]["width"], "viewport")?,
                    num(&s["viewport"]["height"], "viewport")?,
                ),
                landmarks: landmarks
                    .iter()
                    .map(|l| {
                        Ok(Landmark {
                            role: text(&l[0], "landmark role")?.into(),
                            name: text(&l[1], "landmark name")?.into(),
                            occurrence: l[2].as_u64().ok_or("metrics.json: landmark occurrence")?
                                as usize,
                            rect: rect(&l[3], "landmark rect")?,
                            style: style(tables, &l[4])?,
                        })
                    })
                    .collect::<Result<_>>()?,
            })
        })
        .collect()
}

/// The `(window, scene)` rows of `STATES.md`'s table.
pub(super) fn parse_states_md(md: &str) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = md
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Window"))
        .map(|l| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            (cells[1].to_owned(), cells[2].to_owned())
        })
        .collect();
    if rows.is_empty() {
        return Err("STATES.md has no scene rows".into());
    }
    Ok(rows)
}

/// `STATES.md` and `metrics.json` name the same scenes, each captured in both themes.
pub(super) fn check_consistent(states_md: &[(String, String)], refs: &[RefState]) -> Result<()> {
    let md: BTreeSet<_> = states_md.iter().cloned().collect();
    let json: BTreeSet<_> = refs
        .iter()
        .map(|r| (r.window.clone(), r.scene.clone()))
        .collect();
    if md.len() != states_md.len() {
        return Err("STATES.md lists a scene twice".into());
    }
    if md != json {
        let only_md: Vec<_> = md.difference(&json).collect();
        let only_json: Vec<_> = json.difference(&md).collect();
        return Err(format!("STATES.md and metrics.json disagree: only in STATES.md {only_md:?}, only in metrics.json {only_json:?}").into());
    }
    for (w, s) in &md {
        for theme in ["light", "dark"] {
            if !refs
                .iter()
                .any(|r| &r.window == w && &r.scene == s && r.theme == theme)
            {
                return Err(format!("metrics.json has no {theme} capture of {w}/{s}").into());
            }
        }
    }
    Ok(())
}

/// Reference scenes the new app does not render yet, with the step that will build each. A step
/// that adds a scene deletes its line here; `cargo xtask parity` fails if a listed scene exists
/// (stale mark) or if a reference scene is neither rendered nor listed.
/// `(window, scene, step)`.
pub(super) type Pending = (&'static str, &'static str, &'static str);

pub(super) const NOT_YET_BUILT: &[Pending] = &[
    ("main", "library", "S50"),
    ("main", "library-empty", "S50"),
    ("main", "snippet-bulk", "S50"),
    ("main", "snippet-deleted-toast", "S50"),
    ("main", "snippet-actions", "S51"),
    ("main", "insert-menu", "S52"),
    ("main", "chip-date", "S53"),
    ("main", "chip-time", "S53"),
    ("main", "chip-clipboard", "S53"),
    ("main", "chip-cursor", "S53"),
    ("main", "chip-snippet", "S53"),
    ("main", "chip-field", "S53"),
    ("main", "chip-choice", "S53"),
    ("main", "chip-selection", "S53"),
    ("main", "chip-uuid", "S53"),
    ("main", "chip-random", "S53"),
    ("main", "chip-key", "S53"),
    ("main", "chip-delay", "S53"),
    ("main", "formatting-toolbar", "S54"),
    ("main", "group-context-menu", "S42"),
    ("main", "group-dialog", "S55"),
    ("main", "command-menu", "S43"),
    ("main", "cheat-sheet", "S43"),
    ("main", "clipboard-history", "S58"),
    ("main", "clips-5000", "S58"),
    ("main", "clipboard-empty", "S58"),
    ("main", "clipboard-options-menu", "S58"),
    ("main", "clipboard-bulk", "S58"),
    ("main", "clipboard-no-match", "S58"),
    ("main", "clipboard-image", "S59"),
    ("main", "clipboard-link", "S59"),
    ("main", "clipboard-file", "S59"),
    ("main", "clipboard-code", "S59"),
    ("main", "clipboard-color", "S59"),
    ("main", "settings-general", "S61"),
    ("main", "settings-expansion", "S61"),
    ("main", "settings-apps", "S61"),
    ("main", "settings-clipboard", "S62"),
    ("main", "settings-agents", "S62"),
    ("main", "settings-data", "S62"),
    ("main", "settings-activity", "S63"),
    ("main", "settings-about", "S63"),
    ("main", "activity-empty", "S63"),
    ("main", "update", "S63"),
    ("main", "insights", "S64"),
    ("picker", "history", "S60"),
    ("picker", "clips-5000", "S60"),
    ("picker", "empty", "S60"),
];

/// Gallery scenes measured by their goldens only (no `parity:`, `spec:` or `info:` samples).
/// Any other gallery scene without samples fails: a lost key must not read as a pass.
pub(super) const GALLERY_UNMEASURED: &[(&str, &str)] = &[
    (
        "tokens",
        "token goldens (S04); colours are checked by the theme table tests",
    ),
    ("icons", "icon goldens (S05/S06)"),
];

/// The step that will build `window/scene`, if it is marked not yet built.
pub(super) fn pending_step(pending: &[Pending], window: &str, scene: &str) -> Option<&'static str> {
    pending
        .iter()
        .find(|(w, s, _)| *w == window && *s == scene)
        .map(|(_, _, step)| *step)
}

/// Every not-yet-built mark names a reference scene, once.
pub(super) fn check_pending(pending: &[Pending], refs: &[RefState]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (w, s, step) in pending {
        if !seen.insert((*w, *s)) {
            return Err(format!("{w}/{s} is marked not yet built twice").into());
        }
        if !refs.iter().any(|r| r.window == *w && r.scene == *s) {
            return Err(format!(
                "{w}/{s} is marked not yet built ({step}) but is not a reference scene"
            )
            .into());
        }
    }
    Ok(())
}
