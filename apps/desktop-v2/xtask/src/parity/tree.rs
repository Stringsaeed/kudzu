//! What a Rune `--tree` measures: every element's rect, fill, text and text colour, and from
//! those a landmark's content box (its text and icons) and ink. Rune's tree carries no font
//! size or weight, corner radius, border or shadow, so those are left to side-by-side review.

use std::collections::HashMap;

use serde_json::Value;

use super::reference::Rgba;
use super::Result;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Element {
    pub path: String,
    pub role: String,
    pub key: Option<String>,
    pub label: Option<String>,
    pub rect: [f32; 4],
    pub fill: Option<Rgba>,
    pub color: Option<Rgba>,
    pub text: Option<String>,
}

/// One named control as Rune drew it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Measured {
    pub role: String,
    pub name: String,
    pub occurrence: usize,
    pub path: String,
    pub rect: [f32; 4],
    pub fill: Option<Rgba>,
    /// The union of the text and icons inside: what padding places.
    pub content: Option<[f32; 4]>,
    /// The first text's colour, else the first icon's.
    pub ink: Option<Rgba>,
    /// The colour of every text drawn inside, in tree order. A container (a preview pane)
    /// has no colour of its own to draw; the web reports the colour its unstyled text
    /// inherits, and `style_checks` looks for that among these.
    pub inks: Vec<Rgba>,
}

/// The roles `metrics.json` records.
pub(super) const ROLES: &[&str] = &[
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

/// `#rrggbbaa` → 8-bit channels and alpha; transparent → None.
fn hex(v: &Value) -> Option<Rgba> {
    let s = v.as_str()?.strip_prefix('#')?;
    let n = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok().map(f32::from);
    let c = [n(0)?, n(2)?, n(4)?, n(6)? / 255.0];
    (c[3] > 0.0).then_some(c)
}

pub(super) fn elements(tree: &Value) -> Result<Vec<Element>> {
    let els = tree["elements"]
        .as_array()
        .ok_or("the --tree output has no elements")?;
    els.iter()
        .map(|e| {
            let r = e["rect"]
                .as_array()
                .filter(|r| r.len() == 4)
                .ok_or("an element has no rect")?;
            let n = |i: usize| {
                r[i].as_f64()
                    .map(|v| v as f32)
                    .ok_or("a rect is not numeric")
            };
            let s = |k: &str| e[k].as_str().map(String::from);
            Ok(Element {
                path: s("path").ok_or("an element has no path")?,
                role: s("role").unwrap_or_default(),
                key: s("key"),
                label: s("label").filter(|l| !l.is_empty()),
                rect: [n(0)?, n(1)?, n(2)?, n(3)?],
                fill: hex(&e["fill"]),
                color: hex(&e["color"]),
                text: s("text").filter(|t| !t.trim().is_empty()),
            })
        })
        .collect()
}

fn inside(path: &str, ancestor: &str) -> bool {
    path.len() > ancestor.len()
        && path.starts_with(ancestor)
        && path.as_bytes()[ancestor.len()] == b'/'
}

fn union(a: Option<[f32; 4]>, b: [f32; 4]) -> [f32; 4] {
    match a {
        None => b,
        Some(a) => {
            let (x, y) = (a[0].min(b[0]), a[1].min(b[1]));
            let (r, bo) = (
                (a[0] + a[2]).max(b[0] + b[2]),
                (a[1] + a[3]).max(b[1] + b[3]),
            );
            [x, y, r - x, bo - y]
        }
    }
}

/// The text and icons drawn inside `el` (and `el` itself when it is one): their union, and the
/// ink of the first text (else the first icon).
pub(super) fn content(els: &[Element], el: &Element) -> (Option<[f32; 4]>, Option<Rgba>) {
    let (mut rect, mut text_ink, mut icon_ink) = (None, None, None);
    for e in els
        .iter()
        .filter(|e| e.path == el.path || inside(&e.path, &el.path))
    {
        let drawn = e.rect[2] > 0.0 && e.rect[3] > 0.0;
        if e.text.is_some() && drawn {
            rect = Some(union(rect, e.rect));
            text_ink = text_ink.or(e.color);
        } else if e.role == "icon" && drawn {
            rect = Some(union(rect, e.rect));
            icon_ink = icon_ink.or(e.color);
        }
    }
    (rect, text_ink.or(icon_ink))
}

/// The colour of every drawn text inside `el`, in tree order.
pub(super) fn text_inks(els: &[Element], el: &Element) -> Vec<Rgba> {
    els.iter()
        .filter(|e| e.path == el.path || inside(&e.path, &el.path))
        .filter(|e| e.text.is_some() && e.rect[2] > 0.0 && e.rect[3] > 0.0)
        .filter_map(|e| e.color)
        .collect()
}

/// The named controls in tree order, numbered per role and name. A named textbox inside an
/// unnamed one is the editor inside a field's box: it measures as the box (rect and fill), and
/// the editor is its content, as the web's `<input>` is one element.
pub(super) fn landmarks(els: &[Element]) -> Vec<Measured> {
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut out = Vec::new();
    for e in els {
        let Some(name) = e
            .label
            .as_ref()
            .filter(|_| ROLES.contains(&e.role.as_str()))
        else {
            continue;
        };
        let field_box = (e.role == "textbox")
            .then(|| {
                els.iter()
                    .rev()
                    .find(|b| b.role == "textbox" && b.label.is_none() && inside(&e.path, &b.path))
            })
            .flatten();
        let (content, ink) = content(els, e);
        let (rect, fill, content, ink) = match field_box {
            Some(b) => (b.rect, b.fill, Some(e.rect), ink.or(e.color)),
            None => (e.rect, e.fill, content, ink),
        };
        let n = seen.entry((e.role.clone(), name.clone())).or_default();
        *n += 1;
        out.push(Measured {
            role: e.role.clone(),
            name: name.clone(),
            occurrence: *n,
            path: field_box.map_or(e.path.clone(), |b| b.path.clone()),
            rect,
            fill,
            content,
            ink,
            inks: text_inks(els, e),
        });
    }
    out
}

/// The landmarks inside the element at `path`, numbered again from 1 within it.
pub(super) fn landmarks_within(els: &[Element], path: &str) -> Vec<Measured> {
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    landmarks(els)
        .into_iter()
        .filter(|m| inside(&m.path, path))
        .map(|mut m| {
            let n = seen.entry((m.role.clone(), m.name.clone())).or_default();
            *n += 1;
            m.occurrence = *n;
            m
        })
        .collect()
}
