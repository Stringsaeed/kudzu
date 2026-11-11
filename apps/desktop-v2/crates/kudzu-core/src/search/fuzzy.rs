//! The subsequence matcher behind search: a greedy variant of the fzf model with bonuses for
//! word starts and consecutive runs.
//!
//! Everything works on UTF-16 code units, exactly like the JavaScript it comes from, so scores
//! and the reported positions agree with the recorded fixtures for any text: an emoji is two
//! units, and a term cut at 64 units can split one.

/// UTF-16 code units of `text`.
pub(super) fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// JavaScript's `\s`: white space and line terminators.
fn is_js_space(unit: u16) -> bool {
    matches!(
        unit,
        0x09..=0x0d
            | 0x20
            | 0xa0
            | 0x1680
            | 0x2000..=0x200a
            | 0x2028
            | 0x2029
            | 0x202f
            | 0x205f
            | 0x3000
            | 0xfeff
    )
}

/// JavaScript's `\s` for a character (every white space character is in the BMP).
pub(super) fn is_js_space_char(c: char) -> bool {
    u16::try_from(u32::from(c)).is_ok_and(is_js_space)
}

fn is_separator(unit: u16) -> bool {
    is_js_space(unit)
        || matches!(
            u8::try_from(unit),
            Ok(b'-'
                | b'_'
                | b'.'
                | b'/'
                | b':'
                | b';'
                | b','
                | b'('
                | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'\''
                | b'"'
                | b'!'
                | b'?')
        )
}

/// Whether a match starting at `i` begins a word: the start of the text, after a separator, or
/// a digit right after a lowercase letter.
pub(super) fn is_boundary(text: &[u16], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = text[i - 1];
    is_separator(prev)
        || (matches!(u8::try_from(prev), Ok(b'a'..=b'z'))
            && matches!(u8::try_from(text[i]), Ok(b'0'..=b'9')))
}

/// Index of the first occurrence of `needle` in `haystack` (JavaScript's `indexOf`).
pub(super) fn find(haystack: &[u16], needle: &[u16]) -> Option<usize> {
    let (first, rest) = needle.split_first()?;
    let last_start = haystack.len().checked_sub(needle.len())?;
    let mut from = 0;
    while from <= last_start {
        let at = from
            + haystack[from..=last_start]
                .iter()
                .position(|u| u == first)?;
        if haystack[at + 1..at + needle.len()] == *rest {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// Scores `term` against `text`, or `None` when `term` is not a subsequence of it. When
/// `positions` is given, the matched unit indices are appended to it.
///
/// A contiguous occurrence scores 16 per unit plus 8 per joint, +20 at a word start, +20 at the
/// start of the text, +20 when it is the whole text, minus half a point per unit of offset (to
/// 20). Otherwise the earliest match is tightened from the right and scored unit by unit:
/// 16 each, +20 (first unit) or +10 at a word start, +4 for a consecutive unit, and a penalty
/// of 1 per skipped unit (3 straight after a match).
pub(super) fn fuzzy(
    term: &[u16],
    text: &[u16],
    mut positions: Option<&mut Vec<usize>>,
) -> Option<f64> {
    let n = text.len();
    let m = term.len();
    if m == 0 {
        return Some(0.0);
    }
    if m > n {
        return None;
    }
    if let Some(idx) = find(text, term) {
        let mut score = (m * 16 + 8 * (m - 1)) as f64;
        if is_boundary(text, idx) {
            score += 20.0;
        }
        if idx == 0 {
            score += 20.0;
        }
        if m == n {
            score += 20.0;
        }
        score -= idx.min(20) as f64 * 0.5;
        if let Some(out) = positions.as_deref_mut() {
            out.extend(idx..idx + m);
        }
        return Some(score);
    }
    // Forward scan to the end of the earliest match.
    let mut ti = 0;
    let mut end = None;
    for (i, unit) in text.iter().enumerate() {
        if *unit == term[ti] {
            ti += 1;
            if ti == m {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    // Backward scan from `end` to the tightest start.
    let mut back = m;
    let mut start = end;
    for i in (0..=end).rev() {
        if text[i] == term[back - 1] {
            back -= 1;
            if back == 0 {
                start = i;
                break;
            }
        }
    }
    // Score the window forward.
    let mut ti = 0;
    let mut score = 0.0;
    let mut prev_match: Option<usize> = None;
    for i in start..=end {
        if ti >= m {
            break;
        }
        let consecutive = i > 0 && prev_match == Some(i - 1);
        if text[i] == term[ti] {
            let mut s = 16.0;
            if is_boundary(text, i) {
                s += if ti == 0 { 20.0 } else { 10.0 };
            }
            if consecutive {
                s += 4.0;
            }
            score += s;
            prev_match = Some(i);
            if let Some(out) = positions.as_deref_mut() {
                out.push(i);
            }
            ti += 1;
        } else {
            score -= if consecutive { 3.0 } else { 1.0 };
        }
    }
    Some(score)
}
