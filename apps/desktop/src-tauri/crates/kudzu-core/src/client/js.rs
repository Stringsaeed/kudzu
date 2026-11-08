//! The JavaScript semantics the web UI leaned on, so ports give the same answers.
//!
//! - Strings: JS strings are UTF-16, so lengths, offsets and slices here count UTF-16 code units.
//! - Whitespace: `\s`, `trim()` and `split(/\s+/)` use the ECMAScript whitespace set, which is
//!   not Rust's `char::is_whitespace` (it adds U+FEFF and leaves out U+0085).
//! - Numbers: `Math.round`, `Number#toFixed` and `String(number)`.
//! - Collation: `localeCompare`, approximated with a small UCA-style collator (see [`locale_compare`]).
//! - Dates: the en-US `Intl` output the UI showed.

use std::cmp::Ordering;

use chrono::{DateTime, Datelike, TimeZone, Timelike};
use unicode_normalization::UnicodeNormalization;

// MARK: Strings

/// UTF-16 code units of `s`.
pub fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Length of `s` in UTF-16 code units (`string.length`).
pub fn len16(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Decodes UTF-16, replacing unpaired surrogates with U+FFFD (one code unit, so offsets hold).
pub fn from16(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// `s.slice(start, end)` in UTF-16 units, clamped to the string.
pub fn slice16(s: &str, start: usize, end: usize) -> String {
    let units = utf16(s);
    let end = end.min(units.len());
    let start = start.min(end);
    from16(&units[start..end])
}

/// ECMAScript `WhiteSpace` and `LineTerminator`: what `\s` and `trim()` match.
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// [`is_space`] for one UTF-16 code unit (surrogates are never whitespace).
pub fn is_space_unit(u: u16) -> bool {
    char::from_u32(u32::from(u)).is_some_and(is_space)
}

/// `s.trim()`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `s.replace(/\s+/g, " ")`.
pub fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if is_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// `s.split(/\s+/)`, empty pieces included (a leading run yields `""` first).
pub fn split_spaces(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = false;
    for (i, c) in s.char_indices() {
        if is_space(c) {
            if !in_space {
                out.push(&s[start..i]);
                in_space = true;
            }
        } else if in_space {
            start = i;
            in_space = false;
        }
    }
    out.push(if in_space { "" } else { &s[start..] });
    out
}

/// `s.toLowerCase()`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// `toLowerCase()` over UTF-16 units, keeping unpaired surrogates as they are.
pub fn lower16(units: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(units.len());
    let mut run = String::new();
    for r in char::decode_utf16(units.iter().copied()) {
        match r {
            Ok(c) => run.push(c),
            Err(e) => {
                out.extend(run.to_lowercase().encode_utf16());
                run.clear();
                out.push(e.unpaired_surrogate());
            }
        }
    }
    out.extend(run.to_lowercase().encode_utf16());
    out
}

/// `hay.indexOf(needle, from)` over UTF-16 units.
pub fn index_of16(hay: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    let from = from.min(hay.len());
    if needle.is_empty() {
        return Some(from);
    }
    if needle.len() > hay.len() {
        return None;
    }
    let first = needle[0];
    (from..=hay.len() - needle.len()).find(|&i| hay[i] == first && hay[i..i + needle.len()] == *needle)
}

// MARK: Numbers

/// `Math.round`: halves round toward +∞, and small negatives give -0.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x < 0.0 {
        -0.0
    } else {
        r
    }
}

/// `String(x)` / `${x}` for a number.
pub fn number_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x < 0.0 {
        return format!("-{}", number_to_string(-x));
    }
    if x.is_infinite() {
        return "Infinity".into();
    }
    let sci = format!("{x:e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { '+' } else { '-' };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    }
}

/// `x.toFixed(digits)`: rounds the exact binary value, ties away from zero.
pub fn to_fixed(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.abs() >= 1e21 {
        return number_to_string(x);
    }
    // 1,100 places hold every finite double's exact decimal expansion, so nothing is rounded yet.
    let exact = format!("{:.1100}", x.abs());
    let (int, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut kept: Vec<u8> = int.bytes().chain(frac.bytes().take(digits)).collect();
    if frac.as_bytes().get(digits).is_some_and(|&d| d >= b'5') {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let split = kept.len() - digits;
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    out.push_str(std::str::from_utf8(&kept[..split]).unwrap_or("0"));
    if digits > 0 {
        out.push('.');
        out.push_str(std::str::from_utf8(&kept[split..]).unwrap_or(""));
    }
    out
}

/// `n.toLocaleString()` in en-US: `2048` → `"2,048"`.
pub fn group_thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

// MARK: Collation

/// Collation element: primary (base letter), secondary (accents), tertiary (case and variants).
#[derive(Clone, Copy)]
struct Ce {
    p: u64,
    s: u32,
    t: u32,
}

const GROUP_SPACE: u64 = 1 << 32;
const GROUP_PUNCT: u64 = 2 << 32;
const GROUP_SYMBOL: u64 = 3 << 32;
const GROUP_DIGIT: u64 = 4 << 32;
const GROUP_LETTER: u64 = 5 << 32;
const GROUP_LAST: u64 = 6 << 32;
/// ASCII punctuation and symbols in CLDR root order.
const ASCII_PUNCT: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
const COMMON: u32 = 5;
const LOWER: u32 = 5;
const UPPER: u32 = 6;
const VARIANT: u32 = 8;

fn ignorable(c: char) -> bool {
    (c.is_control() && !matches!(c, '\t' | '\n' | '\u{B}' | '\u{C}' | '\r')) || matches!(c, '\u{AD}' | '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}')
}

fn push_base(out: &mut Vec<Ce>, c: char, variant: bool) {
    let bump = if variant { VARIANT } else { 0 };
    // Letters that the root collation treats as a base letter plus a difference.
    let (bases, mark): (&[char], Option<u32>) = match c.to_lowercase().next().unwrap_or(c) {
        'æ' => (&['a', 'e'], None),
        'œ' => (&['o', 'e'], None),
        'ß' => (&['s', 's'], None),
        'ø' => (&['o'], Some(0x338)),
        'đ' => (&['d'], Some(0x335)),
        'ł' => (&['l'], Some(0x337)),
        'ħ' => (&['h'], Some(0x335)),
        _ => (&[], None),
    };
    if !bases.is_empty() {
        let upper = c.is_uppercase();
        let extra = if mark.is_none() { VARIANT / 2 } else { 0 };
        for &b in bases {
            out.push(Ce { p: GROUP_LETTER | u64::from(u32::from(b)), s: COMMON, t: if upper { UPPER } else { LOWER } + bump + extra });
        }
        if let Some(m) = mark {
            out.push(Ce { p: 0, s: 0x100 + m, t: 0 });
        }
        return;
    }
    let ce = if c == '\u{FFFF}' {
        Ce { p: GROUP_LAST, s: COMMON, t: LOWER }
    } else if is_space(c) {
        Ce { p: GROUP_SPACE | u64::from(u32::from(c)), s: COMMON, t: LOWER + bump }
    } else if let Some(i) = ASCII_PUNCT.find(c) {
        let group = if i < ASCII_PUNCT.find('@').unwrap_or(0) { GROUP_PUNCT } else { GROUP_SYMBOL };
        Ce { p: group | i as u64, s: COMMON, t: LOWER + bump }
    } else if c.is_ascii_digit() {
        Ce { p: GROUP_DIGIT | u64::from(u32::from(c) - u32::from('0')), s: COMMON, t: LOWER + bump }
    } else if c.is_alphanumeric() {
        let base = c.to_lowercase().next().unwrap_or(c);
        let group = if c.is_numeric() { GROUP_DIGIT + 0x100 } else { GROUP_LETTER };
        Ce { p: group + u64::from(u32::from(base)), s: COMMON, t: if c.is_uppercase() { UPPER } else { LOWER } + bump }
    } else {
        Ce { p: GROUP_SYMBOL + 0x100 + u64::from(u32::from(c)), s: COMMON, t: LOWER + bump }
    };
    out.push(ce);
}

fn elements(s: &str) -> Vec<Ce> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        if ignorable(c) {
            continue;
        }
        let canonical: String = std::iter::once(c).nfd().collect();
        let compat: String = std::iter::once(c).nfkd().collect();
        let variant = canonical != compat;
        for d in compat.chars() {
            if unicode_normalization::char::is_combining_mark(d) {
                out.push(Ce { p: 0, s: 0x100 + u32::from(d), t: 0 });
            } else if !ignorable(d) {
                push_base(&mut out, d, variant);
            }
        }
    }
    out
}

fn collate(a: &str, b: &str, base_only: bool) -> Ordering {
    let (x, y) = (elements(a), elements(b));
    let primary = |v: &[Ce]| v.iter().filter(|c| c.p != 0).map(|c| c.p).collect::<Vec<_>>();
    let order = primary(&x).cmp(&primary(&y));
    if order != Ordering::Equal || base_only {
        return order;
    }
    let secondary = |v: &[Ce]| v.iter().filter(|c| c.s != 0).map(|c| c.s).collect::<Vec<_>>();
    let order = secondary(&x).cmp(&secondary(&y));
    if order != Ordering::Equal {
        return order;
    }
    let tertiary = |v: &[Ce]| v.iter().filter(|c| c.t != 0).map(|c| c.t).collect::<Vec<_>>();
    tertiary(&x).cmp(&tertiary(&y))
}

/// `a.localeCompare(b)` for en: letters before by base letter, then accents, then case
/// (lowercase first); whitespace, punctuation, symbols and digits sort before letters, in CLDR
/// root order. Exact for ASCII; an approximation of ICU for the rest of Unicode.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    collate(a, b, false)
}

/// `a.localeCompare(b, undefined, { sensitivity: "base" })`: base letters only.
pub fn locale_compare_base(a: &str, b: &str) -> Ordering {
    collate(a, b, true)
}

/// `b - a || tie` for scores, as JS sort comparators write it: a zero or NaN difference falls
/// through to `tie`.
pub fn by_score_desc(a: f64, b: f64, tie: impl FnOnce() -> Ordering) -> Ordering {
    let d = b - a;
    if d > 0.0 {
        Ordering::Greater
    } else if d < 0.0 {
        Ordering::Less
    } else {
        tie()
    }
}

// MARK: Dates (en-US)

const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `{ weekday: "long" }`: "Monday".
pub fn weekday_long<Tz: TimeZone>(d: &DateTime<Tz>) -> &'static str {
    WEEKDAYS[d.weekday().num_days_from_monday() as usize]
}

/// `{ weekday: "short" }`: "Mon".
pub fn weekday_short<Tz: TimeZone>(d: &DateTime<Tz>) -> &'static str {
    &weekday_long(d)[..3]
}

/// `{ month: "short" }`: "Oct".
pub fn month_short<Tz: TimeZone>(d: &DateTime<Tz>) -> &'static str {
    MONTHS[d.month0() as usize]
}

/// `{ month: "short", day: "numeric" }`: "Oct 28".
pub fn month_day<Tz: TimeZone>(d: &DateTime<Tz>) -> String {
    format!("{} {}", month_short(d), d.day())
}

/// `{ month: "short", day: "numeric", year: "numeric" }` and `dateStyle: "medium"`: "Oct 28, 2026".
pub fn date_medium<Tz: TimeZone>(d: &DateTime<Tz>) -> String {
    format!("{}, {}", month_day(d), d.year())
}

/// `{ hour: "numeric", minute: "2-digit" }`: "2:04 PM".
pub fn time_short<Tz: TimeZone>(d: &DateTime<Tz>) -> String {
    let (pm, h) = d.hour12();
    format!("{h}:{:02} {}", d.minute(), if pm { "PM" } else { "AM" })
}

/// `{ hour: "numeric", minute: "2-digit", second: "2-digit" }`: "2:04:11 PM".
pub fn time_seconds<Tz: TimeZone>(d: &DateTime<Tz>) -> String {
    let (pm, h) = d.hour12();
    format!("{h}:{:02}:{:02} {}", d.minute(), d.second(), if pm { "PM" } else { "AM" })
}

/// Milliseconds since the epoch as a local date-time (`new Date(ms)`).
pub fn local<Tz: TimeZone>(ms: i64, tz: &Tz) -> DateTime<Tz> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default().with_timezone(tz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_split_matches_js() {
        assert_eq!(split_spaces("a  b"), ["a", "b"]);
        assert_eq!(split_spaces(" a"), ["", "a"]);
        assert_eq!(split_spaces("a "), ["a", ""]);
        assert_eq!(split_spaces(""), [""]);
        assert_eq!(trim("\u{FEFF}\u{A0} x \u{3000}"), "x");
        assert_eq!(trim("\u{85}x"), "\u{85}x");
        assert_eq!(collapse_spaces("a \n\t b"), "a b");
    }

    #[test]
    fn numbers_match_js() {
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(-2.5), -2.0);
        assert!(round(-0.4).is_sign_negative());
        assert_eq!(number_to_string(1.0), "1");
        assert_eq!(number_to_string(1.5), "1.5");
        assert_eq!(number_to_string(-0.0), "0");
        assert_eq!(number_to_string(1e21), "1e+21");
        assert_eq!(number_to_string(1.5e-7), "1.5e-7");
        assert_eq!(number_to_string(0.000001), "0.000001");
        assert_eq!(number_to_string(123456789012345680000.0), "123456789012345680000");
        assert_eq!(to_fixed(0.15, 1), "0.1");
        assert_eq!(to_fixed(0.25, 1), "0.3");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(-0.0001, 1), "-0.0");
        assert_eq!(to_fixed(9.96, 1), "10.0");
        assert_eq!(to_fixed(999.5, 0), "1000");
        assert_eq!(group_thousands(2048), "2,048");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
        assert_eq!(group_thousands(999), "999");
    }

    fn sorted(words: &[&str], base: bool) -> Vec<String> {
        let mut v: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        v.sort_by(|a, b| if base { locale_compare_base(a, b) } else { locale_compare(a, b) });
        v
    }

    // Expected orders come from Node 24 (full ICU): `[...w].sort((a, b) => a.localeCompare(b))`.
    const WORDS: [&str; 43] = [
        "a", "A", "á", "Á", "ab", "Ab", "a-b", "a_b", "a.b", "a1", "a9", "a10", "résumé", "resume", "Resume", "naïve", "naive", "Zebra", "apple", "Apple", "éclair", "eclair", "Ωmega",
        "омега", "日本", "#tag", "@me", "$x", "~t", "0", "9", "10", "a\tb", "!", "?", "æ", "ae", "ß", "ss", "st", "Ø", "o", "p",
    ];

    #[test]
    fn locale_compare_matches_icu() {
        let expected = [
            "!", "?", "@me", "#tag", "~t", "$x", "0", "10", "9", "a", "A", "á", "Á", "a\tb", "a_b", "a-b", "a.b", "a1", "a10", "a9", "ab", "Ab", "ae", "æ", "apple", "Apple", "eclair", "éclair",
            "naive", "naïve", "o", "Ø", "p", "resume", "Resume", "résumé", "ss", "ß", "st", "Zebra", "Ωmega", "омега", "日本",
        ];
        assert_eq!(sorted(&WORDS, false), expected);
        let base = [
            "!", "?", "@me", "#tag", "~t", "$x", "0", "10", "9", "a", "A", "á", "Á", "a\tb", "a_b", "a-b", "a.b", "a1", "a10", "a9", "ab", "Ab", "æ", "ae", "apple", "Apple", "éclair", "eclair",
            "naïve", "naive", "Ø", "o", "p", "résumé", "resume", "Resume", "ß", "ss", "st", "Zebra", "Ωmega", "омега", "日本",
        ];
        assert_eq!(sorted(&WORDS, true), base);
        let mut punct: Vec<char> = (32u8..127).map(char::from).filter(|c| !c.is_ascii_alphanumeric()).collect();
        punct.sort_by(|a, b| locale_compare(&a.to_string(), &b.to_string()));
        assert_eq!(punct.into_iter().collect::<String>(), " _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$");
        assert_eq!(sorted(&["a\tb", "a b", "a\nb", "a\u{a0}b", "ab"], false), ["a\tb", "a\nb", "a b", "a\u{a0}b", "ab"]);
        assert_eq!(locale_compare("\u{FFFF}", "zzz"), Ordering::Greater);
        assert_eq!(locale_compare("x", "x\u{0}"), Ordering::Equal);
        assert_eq!(locale_compare("ﬁ", "fi"), Ordering::Greater);
        assert_eq!(locale_compare("²", "2"), Ordering::Greater);
    }

    #[test]
    fn utf16_helpers() {
        assert_eq!(len16("😀a"), 3);
        assert_eq!(slice16("😀a", 2, 3), "a");
        assert_eq!(slice16("😀a", 0, 1), "\u{FFFD}");
        assert_eq!(index_of16(&utf16("abcabc"), &utf16("c"), 3), Some(5));
        assert_eq!(index_of16(&utf16("abc"), &[], 9), Some(3));
        assert_eq!(from16(&lower16(&utf16("ÄB"))), "äb");
    }
}
