//! List markers as text operations.
//!
//! Markers are real characters (`• `, `1. `), so plain-text apps and exports keep the list.
//! The line range is the editor's selection: from `lineAt(from)` through `lineAt(to - 1)` when
//! the selection is non-empty. Lines break on `\n`. A line that already has either marker loses
//! that prefix before the new one is written. When every selected line already has the requested
//! marker, the markers come off instead.

/// One replacement, in display **byte** offsets. Apply from last to first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEdit {
    pub start: usize,
    pub end: usize,
    pub insert: String,
}

/// The edits that toggle a list on `text`. `from` and `to` are display byte offsets.
pub fn list_edits(text: &str, from: usize, to: usize, ordered: bool) -> Vec<ListEdit> {
    let lines = lines_of(text);
    let from = from.min(text.len());
    let to = to.min(text.len());
    let probe = if to > from { to - 1 } else { to };
    let first = line_index(&lines, text.len(), from);
    let last = line_index(&lines, text.len(), probe);
    let selected = &lines[first..=last];
    let remove = selected.iter().all(|(_, line)| {
        if ordered {
            ordered_marker(line)
        } else {
            line.starts_with("\u{2022} ")
        }
    });
    selected
        .iter()
        .enumerate()
        .map(|(i, (start, line))| {
            let existing = marker_bytes(line);
            let insert = if remove {
                String::new()
            } else if ordered {
                format!("{}. ", i + 1)
            } else {
                "\u{2022} ".to_string()
            };
            ListEdit {
                start: *start,
                end: *start + existing,
                insert,
            }
        })
        .collect()
}

fn lines_of(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, _) in text.match_indices('\n') {
        out.push((start, &text[start..i]));
        start = i + 1;
    }
    out.push((start, &text[start..]));
    out
}

fn line_index(lines: &[(usize, &str)], len: usize, pos: usize) -> usize {
    lines
        .iter()
        .rposition(|(start, text)| {
            *start <= pos
                && (pos < *start + text.len() || *start + text.len() == len || pos == *start)
        })
        .unwrap_or(0)
}

fn ordered_marker(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return false;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    bytes.get(i) == Some(&b'.') && bytes.get(i + 1) == Some(&b' ')
}

fn marker_bytes(line: &str) -> usize {
    if let Some(rest) = line.strip_prefix("\u{2022} ") {
        return line.len() - rest.len();
    }
    if ordered_marker(line) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while bytes[i].is_ascii_digit() {
            i += 1;
        }
        return i + 2;
    }
    0
}
