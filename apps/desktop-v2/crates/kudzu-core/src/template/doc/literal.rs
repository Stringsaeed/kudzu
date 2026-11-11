//! Repair only the escape steps that no longer decode to their display text.
use super::load::{literal_from_display, literal_map, slice_raw};
use super::Raw;
use crate::template::Node;
use unicode_segmentation::UnicodeSegmentation;

/// Retain whole source steps; encode only the fragments of a step the edit cuts.
pub(super) fn slice_display(raw: &Raw, from: usize, to: usize) -> Raw {
    let mut out = Raw {
        display: String::new(),
        source: String::new(),
        pill: None,
        steps: vec![(0, 0)],
    };
    for pair in raw.steps.windows(2) {
        let (a, b) = (pair[0].0, pair[1].0);
        if b <= from || a >= to {
            continue;
        }
        let part = if from <= a && b <= to {
            slice_raw(raw, pair[0].1, pair[1].1)
        } else {
            literal_from_display(&raw.display[from.max(a)..to.min(b)])
        };
        append(&mut out, &part);
    }
    if literal_map(&out.source).0 != out.display {
        let mut pieces = vec![out];
        repair(&mut pieces);
        return pieces.remove(0);
    }
    out
}

fn append(out: &mut Raw, part: &Raw) {
    let d = out.display.len();
    let s = out.steps.last().map_or(0, |s| s.1);
    out.steps
        .extend(part.steps.iter().skip(1).map(|&(a, b)| (a + d, b + s)));
    out.display.push_str(&part.display);
    out.source.push_str(&part.source);
}

pub(super) fn repair(raws: &mut Vec<Raw>) {
    let mut out = Vec::new();
    let mut i = 0;
    while i < raws.len() {
        if raws[i].pill.is_some() {
            out.push(raws[i].clone());
            i += 1;
            continue;
        }
        let run_start = i;
        let mut run_end = i;
        while run_end < raws.len() && raws[run_end].pill.is_none() {
            run_end += 1;
        }
        let source: String = raws[i..run_end].iter().map(|r| r.source.as_str()).collect();
        let display: String = raws[i..run_end]
            .iter()
            .map(|r| r.display.as_str())
            .collect();
        let needs_tag_escape = run_end < raws.len()
            && raws[run_end].source.starts_with("{{")
            && (source.ends_with('\\') || source.ends_with("\\{"));
        if !needs_tag_escape && literal_ok(&source, &display) {
            let mut merged = Raw {
                display: String::new(),
                source: String::new(),
                pill: None,
                steps: vec![(0, 0)],
            };
            for raw in &raws[run_start..run_end] {
                append(&mut merged, raw);
            }
            if !merged.display.is_empty() {
                out.push(merged);
            }
            i = run_end;
            continue;
        }
        let mut atoms = Vec::new();
        while i < raws.len() && raws[i].pill.is_none() {
            let raw = &raws[i];
            let boundaries: Vec<_> = raw
                .display
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain([raw.display.len()])
                .collect();
            let mut step = 0;
            while step + 1 < raw.steps.len() {
                let mut end = step + 1;
                let mut atom = slice_raw(raw, raw.steps[step].1, raw.steps[end].1);
                // The map exposes both braces, but their original escape needs both of them.
                while end + 1 < raw.steps.len()
                    && (boundaries.binary_search(&raw.steps[end].0).is_err()
                        || literal_map(&atom.source).0 != atom.display)
                {
                    end += 1;
                    atom = slice_raw(raw, raw.steps[step].1, raw.steps[end].1);
                }
                atoms.push(atom);
                step = end;
            }
            i += 1;
        }
        let mut repaired: Vec<Raw> = Vec::new();
        for atom in atoms {
            repaired.push(atom);
            let source: String = repaired.iter().map(|r| r.source.as_str()).collect();
            let display: String = repaired.iter().map(|r| r.display.as_str()).collect();
            if literal_ok(&source, &display) {
                continue;
            }
            // Prefixes already decode correctly. Find the smallest conflicting suffix,
            // including any backslash run whose parity now changes an escape.
            for start in (0..repaired.len()).rev() {
                let tail: String = repaired[start..]
                    .iter()
                    .map(|r| r.display.as_str())
                    .collect();
                let replacement = literal_from_display(&tail);
                let mut candidate: String = repaired[..start]
                    .iter()
                    .map(|r| r.source.as_str())
                    .collect();
                candidate.push_str(&replacement.source);
                if literal_ok(&candidate, &display) {
                    repaired.splice(start.., [replacement]);
                    break;
                }
            }
        }
        let mut atoms = repaired;
        // A raw trailing backslash must not swallow the next tag's opening braces.
        if i < raws.len() && raws[i].source.starts_with("{{") {
            let count = if source.ends_with("\\{") {
                2
            } else if source.ends_with('\\') {
                1
            } else {
                0
            };
            let start = atoms.len().saturating_sub(count);
            for atom in &mut atoms[start..] {
                *atom = literal_from_display(&atom.display);
            }
        }

        let mut merged = Raw {
            display: String::new(),
            source: String::new(),
            pill: None,
            steps: vec![(0, 0)],
        };
        for atom in atoms {
            append(&mut merged, &atom);
        }
        if !merged.display.is_empty() {
            out.push(merged);
        }
    }
    *raws = out;
}

fn literal_ok(source: &str, display: &str) -> bool {
    let (decoded, _) = literal_map(source);
    if decoded != display {
        return false;
    }
    let parsed = crate::template::parse(source);
    parsed.diagnostics.is_empty()
        && match parsed.nodes.as_slice() {
            [] => display.is_empty(),
            [Node::Text { text, .. }] => text == display,
            _ => false,
        }
}
