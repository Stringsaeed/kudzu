//! Conversion between Espanso `strftime` patterns / second offsets and Kudzu ICU patterns / offsets.

/// An ICU pattern plus the strftime specifiers that had no ICU equivalent (kept as literals).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversionResult {
    pub icu_format: String,
    pub unsupported_specifiers: Vec<String>,
}

/// Converts an Espanso `strftime` format string into an ICU date pattern.
pub fn to_icu(strftime: &str) -> ConversionResult {
    let mut result = String::new();
    let mut literal = String::new();
    let mut unsupported = Vec::new();

    fn flush(result: &mut String, literal: &mut String) {
        if !literal.is_empty() {
            result.push_str(&escape_icu_literal(literal));
            literal.clear();
        }
    }

    let mut chars = strftime.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            literal.push(ch);
            continue;
        }
        let Some(spec) = chars.next() else {
            literal.push('%');
            break;
        };
        if spec == '%' {
            literal.push('%');
        } else if let Some(token) = map_specifier(spec) {
            flush(&mut result, &mut literal);
            result.push_str(token);
        } else {
            let token = format!("%{spec}");
            literal.push_str(&token);
            unsupported.push(token);
        }
    }
    flush(&mut result, &mut literal);
    ConversionResult {
        icu_format: result,
        unsupported_specifiers: unsupported,
    }
}

fn map_specifier(spec: char) -> Option<&'static str> {
    Some(match spec {
        'Y' => "yyyy",
        'y' => "yy",
        'm' => "MM",
        'd' => "dd",
        'e' => "d",
        'H' => "HH",
        'I' => "hh",
        'M' => "mm",
        'S' => "ss",
        'p' => "a",
        'A' => "EEEE",
        'a' => "EEE",
        'B' => "MMMM",
        'b' => "MMM",
        'j' => "DDD",
        'Z' => "zzz",
        'z' => "xx",
        _ => return None,
    })
}

/// Quotes ASCII letter runs so ICU reads them literally; a single quote becomes `''`.
fn escape_icu_literal(text: &str) -> String {
    let mut out = String::new();
    let mut in_quotes = false;
    for ch in text.chars() {
        if ch == '\'' {
            out.push_str("''");
        } else if ch.is_ascii_alphabetic() {
            if !in_quotes {
                out.push('\'');
                in_quotes = true;
            }
            out.push(ch);
        } else {
            if in_quotes {
                out.push('\'');
                in_quotes = false;
            }
            out.push(ch);
        }
    }
    if in_quotes {
        out.push('\'');
    }
    out
}

/// Converts an ICU date pattern back into an Espanso `strftime` format string.
pub fn to_strftime(icu: &str) -> String {
    let chars: Vec<char> = icu.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        out.push('\'');
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    if chars[i] == '%' {
                        out.push_str("%%");
                    } else {
                        out.push(chars[i]);
                    }
                    i += 1;
                }
            }
            continue;
        }
        if ch == '%' {
            out.push_str("%%");
            i += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i] == ch {
                i += 1;
            }
            out.push_str(&map_icu_token(ch, i - start));
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

fn map_icu_token(ch: char, count: usize) -> String {
    let mapped = match (ch, count) {
        ('y', 2) => "%y",
        ('y', _) => "%Y",
        ('M', 4..) => "%B",
        ('M', 3) => "%b",
        ('M', _) => "%m",
        ('d', 2..) => "%d",
        ('d', _) => "%e",
        ('H', _) => "%H",
        ('h', _) => "%I",
        ('m', _) => "%M",
        ('s', _) => "%S",
        ('a', _) => "%p",
        ('E', 4..) => "%A",
        ('E', _) => "%a",
        ('D', _) => "%j",
        ('z', _) => "%Z",
        ('x' | 'X' | 'Z', _) => "%z",
        _ => return ch.to_string().repeat(count),
    };
    mapped.to_string()
}

/// Converts an Espanso offset in seconds into a Kudzu offset (`+1d`, `-4h`, `+1w2d`); `None` for zero.
pub fn seconds_to_kudzu_offset(seconds: i64) -> Option<String> {
    if seconds == 0 {
        return None;
    }
    let sign = if seconds > 0 { '+' } else { '-' };
    let mut rem = seconds.unsigned_abs();
    let mut parts = Vec::new();
    for (unit, size) in [("w", 604_800u64), ("d", 86_400), ("h", 3_600)] {
        let n = rem / size;
        if n > 0 {
            parts.push(format!("{n}{unit}"));
            rem %= size;
        }
    }
    // Leftover seconds below a minute round up to one minute only when nothing else was emitted.
    let minutes = (rem / 60).max(if rem > 0 && parts.is_empty() { 1 } else { 0 });
    if minutes > 0 {
        parts.push(format!("{minutes}min"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("{sign}{}", parts.concat()))
}

/// Converts a Kudzu offset (`+3d`, `-1w2d`, `+30min`) into seconds for Espanso export.
///
/// Months and years are approximated as 30 and 365 days, since Espanso offsets are plain seconds.
pub fn kudzu_offset_to_seconds(offset: &str) -> Option<i64> {
    let trimmed = offset.trim();
    let (sign, body) = match trimmed.strip_prefix('+') {
        Some(rest) => (1, rest),
        None => match trimmed.strip_prefix('-') {
            Some(rest) => (-1, rest),
            None => (1, trimmed),
        },
    };
    let chars: Vec<char> = body.chars().collect();
    let mut total: i64 = 0;
    let mut i = 0;
    let mut parsed_any = false;
    while i < chars.len() {
        let mut number = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            number.push(chars[i]);
            i += 1;
        }
        let mut unit = String::new();
        while i < chars.len() && chars[i].is_alphabetic() {
            unit.push(chars[i]);
            i += 1;
        }
        let value: i64 = number.parse().ok()?;
        let size: i64 = match unit.as_str() {
            "w" => 604_800,
            "d" => 86_400,
            "h" => 3_600,
            "min" => 60,
            "m" => 30 * 86_400,
            "y" => 365 * 86_400,
            _ => return None,
        };
        total = total.checked_add(value.checked_mul(size)?)?;
        parsed_any = true;
    }
    parsed_any.then_some(sign * total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_all_supported_specifiers_both_ways() {
        let pattern = "%Y-%m-%d %H:%M:%S %y %e %I %p %A %a %B %b %j %Z %z %%";
        let converted = to_icu(pattern);
        assert!(converted.unsupported_specifiers.is_empty());
        assert_eq!(
            converted.icu_format,
            "yyyy-MM-dd HH:mm:ss yy d hh a EEEE EEE MMMM MMM DDD zzz xx %"
        );
        assert_eq!(to_strftime(&converted.icu_format), pattern);
    }

    #[test]
    fn quotes_literal_letters_and_reports_unsupported() {
        let converted = to_icu("%Y-%V-%u");
        assert_eq!(converted.icu_format, "yyyy-%'V'-%'u'");
        assert_eq!(converted.unsupported_specifiers, vec!["%V", "%u"]);
        assert_eq!(
            to_icu("%A, %B %e, %Y %I:%M:%S %p (%a %b %d '%y day %j %Z %z) 100%%").icu_format,
            "EEEE, MMMM d, yyyy hh:mm:ss a (EEE MMM dd ''yy 'day' DDD zzz xx) 100%"
        );
        assert_eq!(to_icu("50%").icu_format, "50%");
    }

    #[test]
    fn offsets_convert_both_ways() {
        assert_eq!(seconds_to_kudzu_offset(86_400).as_deref(), Some("+1d"));
        assert_eq!(seconds_to_kudzu_offset(-86_400).as_deref(), Some("-1d"));
        assert_eq!(seconds_to_kudzu_offset(777_600).as_deref(), Some("+1w2d"));
        assert_eq!(seconds_to_kudzu_offset(-14_400).as_deref(), Some("-4h"));
        assert_eq!(seconds_to_kudzu_offset(1_800).as_deref(), Some("+30min"));
        assert_eq!(seconds_to_kudzu_offset(5).as_deref(), Some("+1min"));
        assert_eq!(seconds_to_kudzu_offset(0), None);

        assert_eq!(kudzu_offset_to_seconds("+1d"), Some(86_400));
        assert_eq!(kudzu_offset_to_seconds("-1w2d"), Some(-777_600));
        assert_eq!(kudzu_offset_to_seconds("+30min"), Some(1_800));
        assert_eq!(kudzu_offset_to_seconds("2m"), Some(60 * 86_400));
        assert_eq!(kudzu_offset_to_seconds(""), None);
        assert_eq!(kudzu_offset_to_seconds("+"), None);
        assert_eq!(kudzu_offset_to_seconds("+3x"), None);
        assert_eq!(kudzu_offset_to_seconds("d"), None);
    }
}
