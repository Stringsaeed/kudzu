//! Character references (`&amp;`, `&#x27;`, …) in HTML text.

pub(super) fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let decoded = after.find(';').filter(|&n| n <= 10).and_then(|n| {
            let entity = &after[..n];
            let named = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "trade" => Some('™'),
                "hellip" => Some('…'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "lsquo" => Some('‘'),
                "rsquo" => Some('’'),
                "ldquo" => Some('“'),
                "rdquo" => Some('”'),
                "bull" => Some('•'),
                "middot" => Some('·'),
                "euro" => Some('€'),
                "pound" => Some('£'),
                "yen" => Some('¥'),
                "cent" => Some('¢'),
                "deg" => Some('°'),
                "times" => Some('×'),
                "divide" => Some('÷'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32)
                    .filter(|c| *c != '\0'),
            };
            named.map(|c| (c, n))
        });
        match decoded {
            Some((c, n)) => {
                out.push(c);
                rest = &after[n + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
