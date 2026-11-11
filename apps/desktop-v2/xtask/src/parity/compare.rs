//! Rows of the report: one per reference landmark of a screen, one per gallery sample. A row is a
//! list of property checks, each pass, fail, or "needs side-by-side review" when one side cannot
//! measure the property (Rune's `--tree` has no font, radius or shadow; `metrics.json` no border).

use super::reference::{Landmark, RefState, Rgba, GALLERY_UNMEASURED};
use super::tree::{self, Element, Measured};

/// Geometry tolerance in points (PLAN §1.7).
pub(super) const TOLERANCE: f32 = 2.0;
/// Colour tolerance per 8-bit sRGB channel, after premultiplying.
const CHANNEL: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    Pass,
    Fail,
    Review,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Check {
    pub prop: String,
    pub expected: String,
    pub got: String,
    pub status: Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Its failed checks fail the run.
    Gated,
    /// Reported only: rune-kit's own metrics (D6), scenes with no reference.
    Info,
    /// A reference scene not built yet, with the step that builds it.
    Pending,
}

/// Where the sheet crops a row from: the reference PNG and the Rune render, in points.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Crop {
    pub file: String,
    pub reference: Option<[f32; 4]>,
    pub rune: Option<[f32; 4]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub page: String,
    pub theme: String,
    pub item: String,
    pub kind: Kind,
    pub checks: Vec<Check>,
    pub crop: Option<Crop>,
}

impl Row {
    pub fn failed(&self) -> bool {
        self.kind == Kind::Gated && self.checks.iter().any(|c| c.status == Status::Fail)
    }
}

pub(super) fn check(prop: &str, expected: String, got: String, pass: bool) -> Check {
    let status = if pass { Status::Pass } else { Status::Fail };
    Check {
        prop: prop.into(),
        expected,
        got,
        status,
    }
}

pub(super) fn review(prop: &str, expected: impl Into<String>, why: &str) -> Check {
    Check {
        prop: prop.into(),
        expected: expected.into(),
        got: why.into(),
        status: Status::Review,
    }
}

/// Equal after premultiplying: `metrics.json` reads colours back through an 8-bit premultiplied
/// canvas, so a faint fill's channels are lost (`foreground/[0.025]` reads as black) while what
/// it paints is kept.
pub(super) fn colour_eq(a: Option<Rgba>, b: Option<Rgba>) -> bool {
    let pre =
        |c: Option<Rgba>| c.map_or([0.0; 4], |c| [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]);
    let (a, b) = (pre(a), pre(b));
    (0..3).all(|i| (a[i] - b[i]).abs() <= CHANNEL) && (a[3] - b[3]).abs() <= 0.012
}

pub(super) fn fmt_rgba(c: Option<Rgba>) -> String {
    c.map_or("none".into(), |c| {
        format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], c[3])
    })
}

fn fmt_rect(r: [f32; 4]) -> String {
    format!("{}, {}, {}×{}", r[0], r[1], r[2], r[3])
}

fn max_delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// How far the content sits in from each edge (top, right, bottom, left).
fn insets(r: [f32; 4], c: [f32; 4]) -> [f32; 4] {
    [
        c[1] - r[1],
        (r[0] + r[2]) - (c[0] + c[2]),
        (r[1] + r[3]) - (c[1] + c[3]),
        c[0] - r[0],
    ]
}

/// Padding on side `i` (top, right, bottom, left) against the content's inset there. CSS padding
/// is where content starts on the leading side (top, left) and a floor on the trailing side
/// (right, bottom), where a fixed or minimum size can leave more room. The reference's padding
/// excludes a transparent 1 pt border, which the tolerance covers.
/// - within 2 pt: pass;
/// - content intruding into the padding: fail;
/// - leading side further in: fail, unless the content is centred on that axis (equal insets),
///   which alignment placed: review;
/// - trailing side further in: review (a size the content does not fill).
fn padding_check(prop: &str, padding: [f32; 4], inset: [f32; 4], i: usize) -> Check {
    let (want, got) = (padding[i], inset[i]);
    let got_s = format!("{got} (content inset)");
    if (got - want).abs() <= TOLERANCE {
        return check(prop, want.to_string(), got_s, true);
    }
    if got < want - TOLERANCE {
        return check(prop, want.to_string(), got_s, false);
    }
    let opposite = (i + 2) % 4;
    if i == 0 || i == 3 {
        if (inset[opposite] - got).abs() <= TOLERANCE {
            return review(
                prop,
                want.to_string(),
                &format!("{got_s}: centred on this axis, placed by alignment"),
            );
        }
        return check(prop, want.to_string(), got_s, false);
    }
    review(
        prop,
        want.to_string(),
        &format!("{got_s}: room the content does not fill (a fixed or minimum size)"),
    )
}

/// A control as Rune drew it against the reference landmark: geometry (with position on a
/// screen, size alone in the gallery), background, ink, padding where the reference has any,
/// and what only a side-by-side can judge.
pub(super) fn style_checks(want: &Landmark, got: &Measured, position: bool) -> Vec<Check> {
    let s = &want.style;
    let mut out = Vec::new();
    if position {
        let d = max_delta(&want.rect, &got.rect);
        out.push(check(
            "rect",
            fmt_rect(want.rect),
            format!("{} (Δ {d:.2})", fmt_rect(got.rect)),
            d <= TOLERANCE,
        ));
    } else {
        let d = max_delta(&want.rect[2..], &got.rect[2..]);
        let size = |r: [f32; 4]| format!("{}×{}", r[2], r[3]);
        out.push(check(
            "size",
            size(want.rect),
            format!("{} (Δ {d:.2})", size(got.rect)),
            d <= TOLERANCE,
        ));
    }
    out.push(check(
        "background",
        fmt_rgba(s.background),
        fmt_rgba(got.fill),
        colour_eq(s.background, got.fill),
    ));
    match (s.color, got.ink) {
        (None, _) => {}
        (Some(_), None) => out.push(review(
            "text colour",
            fmt_rgba(s.color),
            "Rune draws no text or icon inside",
        )),
        (Some(_), Some(ink)) => out.push(check(
            "text colour",
            fmt_rgba(s.color),
            fmt_rgba(Some(ink)),
            colour_eq(s.color, Some(ink)),
        )),
    }
    for (i, side) in SIDES
        .iter()
        .enumerate()
        .filter(|(i, _)| s.padding[*i] > 0.0)
    {
        let prop = format!("padding {side}");
        out.push(match got.content {
            None => review(
                &prop,
                s.padding[i].to_string(),
                "Rune draws no text or icon inside",
            ),
            Some(c) => padding_check(&prop, s.padding, insets(got.rect, c), i),
        });
    }
    out.push(review(
        "font size / weight",
        format!("{} / {}", s.font_size, s.font_weight),
        "not in Rune's --tree",
    ));
    let radius: Vec<String> = s
        .radius
        .iter()
        .map(|r| r.map_or("non-px".into(), |r| r.to_string()))
        .collect();
    out.push(review("radius", radius.join(" "), "not in Rune's --tree"));
    out.push(review(
        "border",
        "not recorded",
        "neither metrics.json nor Rune's --tree records borders",
    ));
    let layers = if s.shadow == "none" {
        "none".to_string()
    } else {
        format!("{} layers", s.shadow.matches("px,").count() + 1)
    };
    out.push(review("shadow", layers, "not in Rune's --tree"));
    if ["button", "switch", "textbox", "combobox", "radio"].contains(&want.role.as_str()) {
        out.push(review(
            "focus ring",
            "-",
            "a state look: compare the focus samples side by side",
        ));
    }
    if want.role == "switch" {
        out.push(review("knob", "-", "shape and press stretch: side by side"));
    }
    out
}

struct PermissionRowSpec {
    name: &'static str,
    rect: [f32; 4],
    selected: bool,
    granted: bool,
    badge: &'static str,
}

fn permission_row_specs(r: &RefState) -> Vec<PermissionRowSpec> {
    if r.window != "permissions" {
        return Vec::new();
    }
    let two_line_detail = matches!(
        r.scene.as_str(),
        "missing" | "revoked" | "paused" | "secure"
    );
    let top_y = if two_line_detail { 295.0 } else { 274.0 };
    let acc_granted = matches!(
        r.scene.as_str(),
        "allowed" | "paused" | "secure" | "restart"
    );
    vec![
        PermissionRowSpec {
            name: "Accessibility",
            rect: [41.0, top_y, 338.0, 62.5],
            selected: true,
            granted: acc_granted,
            badge: if acc_granted { "Allowed" } else { "Required" },
        },
        PermissionRowSpec {
            name: "Input Monitoring",
            rect: [41.0, top_y + 70.5, 338.0, 62.5],
            selected: false,
            granted: false,
            badge: "Optional",
        },
    ]
}

fn permission_row_checks(
    r: &RefState,
    spec: &PermissionRowSpec,
    g: &tree::Measured,
    els: &[Element],
) -> Vec<Check> {
    let dark = r.theme == "dark";
    let ink = if dark {
        [250.0, 250.0, 249.0, 1.0]
    } else {
        [12.0, 10.0, 9.0, 1.0]
    };
    let muted = if dark {
        [166.0, 160.0, 155.0, 1.0]
    } else {
        [121.0, 113.0, 107.0, 1.0]
    };
    let success = if dark {
        [84.0, 204.0, 142.0, 1.0]
    } else {
        [0.0, 153.0, 86.0, 1.0]
    };
    let card_bg = match (dark, spec.selected) {
        (false, true) => [20.0, 69.0, 226.0, 0.05],
        (false, false) => [255.0, 255.0, 255.0, 0.5],
        (true, true) => [44.0, 128.0, 255.0, 0.09],
        (true, false) => [28.0, 25.0, 23.0, 0.5],
    };
    let media_bg = if spec.granted {
        [success[0], success[1], success[2], 0.1]
    } else {
        [ink[0], ink[1], ink[2], 0.05]
    };
    let state_fg = if spec.granted { success } else { muted };

    let prefix = format!("{}/", g.path);
    let descendants: Vec<&Element> = els.iter().filter(|e| e.path.starts_with(&prefix)).collect();

    let mut out = vec![
        check(
            "rect",
            fmt_rect(spec.rect),
            fmt_rect(g.rect),
            max_delta(&spec.rect, &g.rect) <= TOLERANCE,
        ),
        check(
            "background",
            fmt_rgba(Some(card_bg)),
            fmt_rgba(g.fill),
            colour_eq(Some(card_bg), g.fill),
        ),
        check(
            "text colour",
            fmt_rgba(Some(ink)),
            fmt_rgba(g.ink),
            colour_eq(Some(ink), g.ink),
        ),
    ];

    let media = descendants.iter().copied().find(|e| {
        e.role != "icon"
            && e.text.is_none()
            && (e.rect[2] - 32.0).abs() <= TOLERANCE
            && (e.rect[3] - 32.0).abs() <= TOLERANCE
    });
    out.push(check(
        "media background",
        fmt_rgba(Some(media_bg)),
        fmt_rgba(media.and_then(|e| e.fill)),
        colour_eq(Some(media_bg), media.and_then(|e| e.fill)),
    ));

    let glyph = descendants
        .iter()
        .copied()
        .find(|e| e.role == "icon" && (e.rect[2] - 16.0).abs() <= 0.5);
    out.push(check(
        "icon colour",
        fmt_rgba(Some(state_fg)),
        fmt_rgba(glyph.and_then(|e| e.color)),
        colour_eq(Some(state_fg), glyph.and_then(|e| e.color)),
    ));

    let check_icon = descendants
        .iter()
        .copied()
        .find(|e| e.role == "icon" && (e.rect[2] - 14.0).abs() <= 0.5);
    out.push(check(
        "check icon",
        if spec.granted { "drawn" } else { "absent" }.into(),
        match check_icon {
            Some(ic) => fmt_rgba(ic.color),
            None => "absent".into(),
        },
        if spec.granted {
            check_icon.is_some_and(|ic| colour_eq(Some(success), ic.color))
        } else {
            check_icon.is_none()
        },
    ));

    let badge = descendants.iter().rev().copied().find(|e| e.text.is_some());
    let got_badge = badge.and_then(|e| e.text.as_deref()).unwrap_or("missing");
    out.push(check(
        "badge",
        spec.badge.into(),
        got_badge.into(),
        got_badge == spec.badge,
    ));
    out.push(check(
        "badge colour",
        fmt_rgba(Some(state_fg)),
        fmt_rgba(badge.and_then(|e| e.color)),
        colour_eq(Some(state_fg), badge.and_then(|e| e.color)),
    ));

    out
}

pub(super) fn screen_rows(r: &RefState, els: &[Element]) -> Vec<Row> {
    let got = tree::landmarks(els);
    let page = format!("{}/{}", r.window, r.scene);
    let mut rows: Vec<Row> = r
        .landmarks
        .iter()
        .map(|want| {
            let found = got.iter().find(|g| {
                g.role == want.role && g.name == want.name && g.occurrence == want.occurrence
            });
            let checks = match found {
                Some(g) => style_checks(want, g, true),
                None => vec![check("present", "drawn".into(), "missing".into(), false)],
            };
            Row {
                page: page.clone(),
                theme: r.theme.clone(),
                item: format!("{} “{}” #{}", want.role, want.name, want.occurrence),
                kind: Kind::Gated,
                checks,
                crop: Some(Crop {
                    file: r.file.clone(),
                    reference: Some(want.rect),
                    rune: found.map(|g| g.rect),
                }),
            }
        })
        .collect();
    let perm_specs = permission_row_specs(r);
    for spec in &perm_specs {
        let found = got
            .iter()
            .find(|g| g.role == "button" && g.name == spec.name && g.occurrence == 1);
        let checks = match found {
            Some(g) => permission_row_checks(r, spec, g, els),
            None => vec![check("present", "drawn".into(), "missing".into(), false)],
        };
        rows.push(Row {
            page: page.clone(),
            theme: r.theme.clone(),
            item: format!("button “{}” #1", spec.name),
            kind: Kind::Gated,
            checks,
            crop: Some(Crop {
                file: r.file.clone(),
                reference: Some(spec.rect),
                rune: found.map(|g| g.rect),
            }),
        });
    }
    for g in got.iter().filter(|g| {
        !r.landmarks
            .iter()
            .any(|w| w.role == g.role && w.name == g.name && w.occurrence == g.occurrence)
            && !perm_specs
                .iter()
                .any(|s| g.role == "button" && g.name == s.name && g.occurrence == 1)
    }) {
        rows.push(Row {
            page: page.clone(),
            theme: r.theme.clone(),
            item: format!("{} “{}” #{}", g.role, g.name, g.occurrence),
            kind: Kind::Info,
            checks: vec![review(
                "present",
                "not in the reference",
                "drawn by Rune only",
            )],
            crop: Some(Crop {
                file: r.file.clone(),
                reference: None,
                rune: Some(g.rect),
            }),
        });
    }
    rows
}

/// What a gallery sample's key asks for.
#[derive(Debug, PartialEq)]
pub(super) enum Expect {
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

pub(super) fn parse_key(key: &str) -> Option<Expect> {
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

/// The sample element as a measured control: a field's box takes its editor as content.
fn measure(els: &[Element], el: &Element, want: &Landmark) -> Measured {
    let editor = (el.role == "textbox")
        .then(|| {
            els.iter().find(|e| {
                e.role == "textbox"
                    && e.label.is_some()
                    && e.path.starts_with(&format!("{}/", el.path))
            })
        })
        .flatten();
    let (content, ink) = match editor {
        Some(e) => (Some(e.rect), tree::content(els, e).1.or(e.color)),
        None => tree::content(els, el),
    };
    Measured {
        role: want.role.clone(),
        name: want.name.clone(),
        occurrence: 1,
        path: el.path.clone(),
        rect: el.rect,
        fill: el.fill,
        content,
        ink,
    }
}

fn within(outer: [f32; 4], r: [f32; 4]) -> bool {
    let s = 0.5;
    r[0] >= outer[0] - s
        && r[1] >= outer[1] - s
        && r[0] + r[2] <= outer[0] + outer[2] + s
        && r[1] + r[3] <= outer[1] + outer[3] + s
}

/// Reference landmarks inside `want` against Rune's inside the sample, by offset from the
/// parent's corner and by size.
fn child_checks(state: &RefState, want: &Landmark, els: &[Element], el: &Element) -> Vec<Check> {
    let rune = tree::landmarks_within(els, &el.path);
    let mut seen = std::collections::HashMap::<(&str, &str), usize>::new();
    let children = state
        .landmarks
        .iter()
        .filter(|l| *l != want && within(want.rect, l.rect));
    children
        .map(|child| {
            let n = seen.entry((&child.role, &child.name)).or_default();
            *n += 1;
            let prop = format!("child {} “{}”", child.role, child.name);
            let rel = |r: [f32; 4], o: [f32; 4]| [r[0] - o[0], r[1] - o[1], r[2], r[3]];
            let expected = rel(child.rect, want.rect);
            match rune
                .iter()
                .find(|m| m.role == child.role && m.name == child.name && m.occurrence == *n)
            {
                None => check(&prop, fmt_rect(expected), "missing".into(), false),
                Some(m) => {
                    let got = rel(m.rect, el.rect);
                    let d = max_delta(&expected, &got);
                    check(
                        &prop,
                        fmt_rect(expected),
                        format!("{} (Δ {d:.2})", fmt_rect(got)),
                        d <= TOLERANCE,
                    )
                }
            }
        })
        .collect()
}

pub(super) fn gallery_rows(
    scene: &str,
    theme: &str,
    els: &[Element],
    refs: &[RefState],
) -> Vec<Row> {
    let page = format!("gallery/{scene}");
    let row = |item: String, kind, checks, crop| Row {
        page: page.clone(),
        theme: theme.into(),
        item,
        kind,
        checks,
        crop,
    };
    let mut rows = Vec::new();
    for el in els {
        let Some(expect) = el.key.as_deref().and_then(parse_key) else {
            continue;
        };
        let size = format!("{}×{}", el.rect[2], el.rect[3]);
        rows.push(match expect {
            Expect::Reference {
                window,
                scene: s,
                role,
                occurrence,
                name,
            } => {
                let item = format!("{window}/{s} {role} “{name}” #{occurrence}");
                let state = refs
                    .iter()
                    .find(|r| r.window == window && r.scene == s && r.theme == theme);
                let want = state.and_then(|r| {
                    r.landmarks
                        .iter()
                        .find(|l| l.role == role && l.name == name && l.occurrence == occurrence)
                });
                match (state, want) {
                    (Some(st), Some(w)) => {
                        let mut checks = style_checks(w, &measure(els, el, w), false);
                        checks.extend(child_checks(st, w, els, el));
                        let crop = Crop {
                            file: st.file.clone(),
                            reference: Some(w.rect),
                            rune: Some(el.rect),
                        };
                        row(item, Kind::Gated, checks, Some(crop))
                    }
                    _ => row(
                        item,
                        Kind::Gated,
                        vec![check(
                            "reference",
                            "a landmark in metrics.json".into(),
                            "no such landmark".into(),
                            false,
                        )],
                        None,
                    ),
                }
            }
            Expect::Spec { what, w, h } => {
                let side = |v: Option<f32>| v.map_or("_".to_string(), |v| v.to_string());
                let d = [(w, el.rect[2]), (h, el.rect[3])]
                    .iter()
                    .filter_map(|(want, g)| want.map(|w| (w - g).abs()))
                    .fold(0.0, f32::max);
                let checks = vec![
                    check(
                        "size",
                        format!("{}×{}", side(w), side(h)),
                        format!("{size} (Δ {d:.2})"),
                        d <= TOLERANCE,
                    ),
                    review(
                        "padding, radius, type, colours",
                        "-",
                        "a spec key gives size only: side by side",
                    ),
                ];
                row(what, Kind::Gated, checks, None)
            }
            Expect::Info(what) => row(
                what,
                Kind::Info,
                vec![review("metrics", size, "rune-kit's own (D6): side by side")],
                None,
            ),
        });
    }
    if rows.is_empty() {
        let item = "keyed samples".to_string();
        rows.push(match GALLERY_UNMEASURED.iter().find(|(s, _)| *s == scene) {
            Some((_, why)) => row(item, Kind::Info, vec![review("samples", "none", why)], None),
            None => row(
                item,
                Kind::Gated,
                vec![check(
                    "samples",
                    "at least one parity:, spec: or info: key".into(),
                    "none".into(),
                    false,
                )],
                None,
            ),
        });
    }
    rows
}
