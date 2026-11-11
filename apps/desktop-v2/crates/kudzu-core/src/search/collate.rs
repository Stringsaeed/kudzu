//! Locale-aware string ordering, standing in for JavaScript's `a.localeCompare(b)`.
//!
//! The Tauri app breaks ranking ties and orders sibling groups with `localeCompare`, which in
//! practice is ICU's root collation (CLDR default: punctuation is not ignorable). The tie order
//! is visible: in a large library most search results tie on score. No collation crate is an
//! approved dependency, so this is a compact implementation of the same three-level scheme:
//!
//! 1. **Primary**: base letters and symbols, case and accents ignored. Groups, low to high:
//!    whitespace, ASCII punctuation in CLDR order, other symbols (emoji among them, by code
//!    point), currency (`$`), digits, Latin `a`–`z`, then every other letter by code point.
//! 2. **Secondary**: accents (`e` < `é` < `è`), compared left to right.
//! 3. **Tertiary**: case, lowercase before uppercase.
//!
//! It is pinned to Node's ICU by `tests/ts_parity_collation.rs` over a corpus of punctuation,
//! digits, case, Latin accents and expansions, Greek, Cyrillic, Hebrew, Arabic and emoji.
//! Known approximations (the TS behaviour is *not* reproduced): non-ASCII punctuation and symbols
//! sort after ASCII ones by code point, where ICU interleaves some of them; scripts are ordered by
//! code point (ICU orders Kana by reading and puts Hangul before Han); `ß` and non-ASCII
//! currency signs are not specially placed; right-to-left letters follow code-point order.

use std::cmp::Ordering;

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// ASCII punctuation and symbols in CLDR root order (after whitespace, before digits).
const ASCII_SYMBOLS: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~";

const WHITESPACE: u32 = 1 << 24;
const SYMBOL: u32 = 2 << 24;
const OTHER_SYMBOL: u32 = 3 << 24;
const CURRENCY: u32 = 4 << 24;
const DIGIT: u32 = 5 << 24;
const LATIN: u32 = 6 << 24;
const OTHER_LETTER: u32 = 7 << 24;

/// A string's collation weights: compare keys instead of strings when sorting many times.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct CollationKey {
    primary: Vec<u32>,
    secondary: Vec<u16>,
    tertiary: Vec<u8>,
}

impl CollationKey {
    /// Computes the weights of `text`.
    pub fn new(text: &str) -> Self {
        let mut key = CollationKey::default();
        for c in text.nfd() {
            if is_combining_mark(c) {
                key.secondary.push(mark_weight(c));
                key.tertiary.push(0);
            } else if !c.is_control() || c.is_whitespace() {
                push_char(&mut key, c);
            }
        }
        key
    }
}

/// Orders `a` against `b` like `a.localeCompare(b)` in the Tauri app.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    CollationKey::new(a).cmp(&CollationKey::new(b))
}

fn push(key: &mut CollationKey, primary: u32, upper: bool) {
    key.primary.push(primary);
    key.secondary.push(1);
    key.tertiary.push(u8::from(upper));
}

fn push_char(key: &mut CollationKey, c: char) {
    if c.is_whitespace() {
        let weight = match c {
            '\t' => 0,
            '\n' => 1,
            '\u{b}' => 2,
            '\u{c}' => 3,
            '\r' => 4,
            _ => 16,
        };
        return push(key, WHITESPACE | weight, c == '\u{a0}');
    }
    if let Some(i) = ASCII_SYMBOLS.find(c) {
        return push(key, SYMBOL | i as u32, false);
    }
    if c == '$' {
        return push(key, CURRENCY, false);
    }
    if c.is_ascii_digit() {
        return push(key, DIGIT | (c as u32 - '0' as u32), false);
    }
    if c.is_numeric() {
        return push(key, DIGIT | 0x100 | (c as u32 & 0xff_ffff), false);
    }
    if !c.is_alphabetic() {
        return push(key, OTHER_SYMBOL | c as u32, false);
    }
    let upper = c.is_uppercase();
    for lower in c.to_lowercase() {
        push_letter(key, lower, upper);
    }
}

fn push_letter(key: &mut CollationKey, c: char, upper: bool) {
    // Letters that are a base letter plus a stroke, or an expansion, rather than a mark.
    let (base, extra): (&str, Option<char>) = match c {
        'ø' => ("o", Some('\u{338}')),
        'đ' => ("d", Some('\u{335}')),
        'ł' => ("l", Some('\u{337}')),
        'æ' => ("ae", None),
        'œ' => ("oe", None),
        _ => ("", None),
    };
    if !base.is_empty() {
        for b in base.chars() {
            push(key, latin(b), upper);
        }
        if let Some(mark) = extra {
            key.secondary.push(mark_weight(mark));
            key.tertiary.push(0);
        } else {
            // The expansion sorts just after the plain two letters.
            *key.tertiary.last_mut().expect("pushed") = 2;
        }
        return;
    }
    if c.is_ascii_lowercase() {
        return push(key, latin(c), upper);
    }
    if c == 'ı' {
        // Dotless i is its own letter right after i.
        return push(key, latin('i') + 1, upper);
    }
    push(key, OTHER_LETTER | c as u32, upper);
}

fn latin(c: char) -> u32 {
    LATIN | ((c as u32 - 'a' as u32) * 4)
}

/// Secondary weight of an accent: acute before grave, then the common marks, then the rest by
/// code point.
fn mark_weight(mark: char) -> u16 {
    const ORDER: [char; 10] = [
        '\u{301}', '\u{300}', '\u{306}', '\u{302}', '\u{30c}', '\u{30a}', '\u{308}', '\u{30b}',
        '\u{303}', '\u{307}',
    ];
    match ORDER.iter().position(|m| *m == mark) {
        Some(i) => 2 + i as u16,
        None => 16 + (mark as u32 & 0x7fff) as u16,
    }
}
