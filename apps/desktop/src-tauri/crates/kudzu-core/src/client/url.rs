//! The slice of the WHATWG URL parser (`new URL(…)`) and `decodeURI` that clipboard link
//! titles need: absolute URLs with a scheme, no base. Covers special schemes (http, https,
//! ftp, ws, wss, file) with userinfo, ports, IDNA (lowercase, NFKC, punycode), IPv4 forms,
//! dot segments and percent-encoding, and opaque paths such as `mailto:`. IPv6 literals are
//! lowercased but not re-serialized.

use unicode_normalization::UnicodeNormalization;

pub(crate) struct Url {
    pub scheme: String,
    /// Host with a non-default port (`URL#host`).
    pub host: String,
    /// `URL#pathname`.
    pub pathname: String,
    /// `URL#search`: `""` or `?query`.
    pub search: String,
}

fn special_port(scheme: &str) -> Option<Option<u16>> {
    match scheme {
        "http" | "ws" => Some(Some(80)),
        "https" | "wss" => Some(Some(443)),
        "ftp" => Some(Some(21)),
        "file" => Some(None),
        _ => None,
    }
}

fn percent_encode(s: &str, extra: &[u8]) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii() && !(c as u8 <= 0x1F || c as u8 == 0x7F || extra.contains(&(c as u8))) {
            out.push(c);
        } else {
            let mut buf = [0; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

const PATH_SET: &[u8] = b" \"#<>?`{}";
const QUERY_SET: &[u8] = b" \"#<>'";
const FORBIDDEN_HOST: &str = "\0\t\n\r #/:<>?@[\\]^|%\u{7f}";

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(0));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fn punycode(label: &str) -> String {
    const BASE: u32 = 36;
    const TMIN: u32 = 1;
    const TMAX: u32 = 26;
    fn digit(d: u32) -> char {
        char::from(if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 })
    }
    fn adapt(mut delta: u32, points: u32, first: bool) -> u32 {
        delta = if first { delta / 700 } else { delta / 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > ((BASE - TMIN) * TMAX) / 2 {
            delta /= BASE - TMIN;
            k += BASE;
        }
        k + (BASE - TMIN + 1) * delta / (delta + 38)
    }
    let input: Vec<u32> = label.chars().map(u32::from).collect();
    let mut out: String = label.chars().filter(char::is_ascii).collect();
    let basic = out.len() as u32;
    let mut handled = basic;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias) = (128u32, 0u32, 72u32);
    while (handled as usize) < input.len() {
        let m = input.iter().copied().filter(|&c| c >= n).min().unwrap_or(n);
        delta = delta.saturating_add((m - n).saturating_mul(handled + 1));
        n = m;
        for &c in &input {
            if c < n {
                delta = delta.saturating_add(1);
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias { TMIN } else if k >= bias + TMAX { TMAX } else { k - bias };
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q));
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    out
}

fn ipv4_number(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    let (digits, radix) = if let Some(h) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
        (h, 16)
    } else if part.len() > 1 && part.starts_with('0') {
        (&part[1..], 8)
    } else {
        (part, 10)
    };
    if digits.is_empty() {
        return Some(0);
    }
    if !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    Some(u64::from_str_radix(digits, radix).unwrap_or(u64::MAX))
}

/// Host parsing for special schemes. `None` is a failure (`new URL` throws).
fn parse_host(input: &str) -> Option<String> {
    if let Some(inner) = input.strip_prefix('[') {
        return inner.ends_with(']').then(|| input.to_ascii_lowercase());
    }
    let decoded = String::from_utf8_lossy(&percent_decode(input)).into_owned();
    let mapped: String = decoded.nfkc().collect::<String>().to_lowercase().replace(['\u{3002}', '\u{FF0E}', '\u{FF61}'], ".");
    let ascii = mapped.split('.').map(|label| if label.is_ascii() { label.to_string() } else { format!("xn--{}", punycode(label)) }).collect::<Vec<_>>().join(".");
    if ascii.is_empty() || ascii.chars().any(|c| FORBIDDEN_HOST.contains(c) || c <= '\u{1f}') {
        return None;
    }
    // Ends in a number: IPv4.
    let mut parts: Vec<&str> = ascii.split('.').collect();
    if parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    let last = parts.last().copied().unwrap_or("");
    let numeric = !last.is_empty() && (last.bytes().all(|b| b.is_ascii_digit()) || ipv4_number(last).is_some() && (last.starts_with("0x") || last.starts_with("0X")));
    if !numeric {
        return Some(ascii);
    }
    if parts.len() > 4 {
        return None;
    }
    let numbers: Vec<u64> = parts.iter().map(|p| ipv4_number(p)).collect::<Option<_>>()?;
    let (init, last) = numbers.split_at(numbers.len() - 1);
    if init.iter().any(|&n| n > 255) || last[0] >= 256u64.pow(5 - numbers.len() as u32) {
        return None;
    }
    let mut ip = last[0];
    for (i, n) in init.iter().enumerate() {
        ip += n * 256u64.pow(3 - i as u32);
    }
    Some(format!("{}.{}.{}.{}", ip >> 24, (ip >> 16) & 255, (ip >> 8) & 255, ip & 255))
}

fn is_single_dot(s: &str) -> bool {
    s == "." || s.eq_ignore_ascii_case("%2e")
}

fn is_double_dot(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), ".." | ".%2e" | "%2e." | "%2e%2e")
}

fn parse_path(path: &str, special: bool) -> String {
    let mut segments: Vec<String> = Vec::new();
    let is_sep = |c: char| c == '/' || (special && c == '\\');
    let body = path.strip_prefix(is_sep).unwrap_or(path);
    let pieces: Vec<&str> = body.split(is_sep).collect();
    for (i, seg) in pieces.iter().enumerate() {
        let at_end = i == pieces.len() - 1;
        if is_double_dot(seg) {
            segments.pop();
            if at_end {
                segments.push(String::new());
            }
        } else if is_single_dot(seg) {
            if at_end {
                segments.push(String::new());
            }
        } else {
            segments.push(percent_encode(seg, PATH_SET));
        }
    }
    format!("/{}", segments.join("/"))
}

pub(crate) fn parse(input: &str) -> Option<Url> {
    let trimmed: String = input.trim_matches(|c: char| c <= ' ').chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
    let colon = trimmed.find(':')?;
    let scheme = &trimmed[..colon];
    let mut chars = scheme.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    let rest = &trimmed[colon + 1..];
    let rest = rest.split_once('#').map_or(rest, |(r, _)| r);
    let (before_query, query) = match rest.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (rest, None),
    };
    let search = |set: &[u8]| query.filter(|q| !q.is_empty()).map(|q| format!("?{}", percent_encode(q, set))).unwrap_or_default();
    if let Some(default_port) = special_port(&scheme) {
        let after = before_query.trim_start_matches(['/', '\\']);
        let end = after.find(['/', '\\']).unwrap_or(after.len());
        let (authority, path) = after.split_at(end);
        let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let (host, port) = if hostport.starts_with('[') {
            match hostport.find(']') {
                Some(i) => (&hostport[..=i], hostport[i + 1..].strip_prefix(':')),
                None => (hostport, None),
            }
        } else {
            match hostport.split_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (hostport, None),
            }
        };
        if host.is_empty() && scheme != "file" {
            return None;
        }
        let mut host = if host.is_empty() { String::new() } else { parse_host(host)? };
        if let Some(p) = port.filter(|p| !p.is_empty()) {
            if !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: u32 = p.trim_start_matches('0').parse().unwrap_or(0);
            if n > 65535 {
                return None;
            }
            if default_port != Some(n as u16) {
                host = format!("{host}:{n}");
            }
        }
        return Some(Url { scheme, host, pathname: parse_path(path, true), search: search(QUERY_SET) });
    }
    if let Some(after) = before_query.strip_prefix("//") {
        let end = after.find('/').unwrap_or(after.len());
        let (authority, path) = after.split_at(end);
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let pathname = if path.is_empty() { String::new() } else { parse_path(path, false) };
        return Some(Url { scheme, host: percent_encode(host, &[]), pathname, search: search(QUERY_SET) });
    }
    if before_query.starts_with('/') {
        return Some(Url { scheme, host: String::new(), pathname: parse_path(before_query, false), search: search(QUERY_SET) });
    }
    Some(Url { scheme, host: String::new(), pathname: percent_encode(before_query, &[]), search: search(QUERY_SET) })
}

/// `decodeURI`: decodes escapes except those of reserved characters; `None` on malformed
/// escapes or invalid UTF-8 (`URIError`).
pub(crate) fn decode_uri(s: &str) -> Option<String> {
    const RESERVED: &str = ";/?:@&=+$,#";
    let b = s.as_bytes();
    let hex = |i: usize| -> Option<u8> {
        if b.get(i) != Some(&b'%') || i + 2 >= b.len() {
            return None;
        }
        let h = s.get(i + 1..i + 3)?;
        u8::from_str_radix(h, 16).ok().filter(|_| h.bytes().all(|c| c.is_ascii_hexdigit()))
    };
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'%' {
            let c = s[i..].chars().next()?;
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        let first = hex(i)?;
        if first < 0x80 {
            let c = char::from(first);
            if RESERVED.contains(c) {
                out.push_str(&s[i..i + 3]);
            } else {
                out.push(c);
            }
            i += 3;
            continue;
        }
        let n = first.leading_ones() as usize;
        if n == 1 || n > 4 {
            return None;
        }
        let mut bytes = vec![first];
        for k in 1..n {
            let byte = hex(i + 3 * k)?;
            if byte & 0xC0 != 0x80 {
                return None;
            }
            bytes.push(byte);
        }
        out.push_str(std::str::from_utf8(&bytes).ok()?);
        i += 3 * n;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(input: &str) -> (String, String, String) {
        let u = parse(input).unwrap();
        (u.host, u.pathname, u.search)
    }

    #[test]
    fn parses_special_urls() {
        assert_eq!(parts("https://www.Example.com/a/b/?x=1#f"), ("www.example.com".into(), "/a/b/".into(), "?x=1".into()));
        assert_eq!(parts("http://user:pw@host:80"), ("host".into(), "/".into(), "".into()));
        assert_eq!(parts("http://host:0081/x?"), ("host:81".into(), "/x".into(), "".into()));
        assert_eq!(parts("https://h/a/./b/../c d"), ("h".into(), "/a/c%20d".into(), "".into()));
        assert_eq!(parts("https://h/a/.."), ("h".into(), "/".into(), "".into()));
        assert_eq!(parts("https://h/é?q=é"), ("h".into(), "/%C3%A9".into(), "?q=%C3%A9".into()));
        assert_eq!(parts("https://bücher.de"), ("xn--bcher-kva.de".into(), "/".into(), "".into()));
        assert_eq!(parts("http://0x7f.1"), ("127.0.0.1".into(), "/".into(), "".into()));
        assert_eq!(parts("http://192.168.0.1:8080/"), ("192.168.0.1:8080".into(), "/".into(), "".into()));
        assert!(parse("http://256.256.256.256").is_none());
        assert!(parse("http://host:99999").is_none());
        assert!(parse("http://ho st").is_none());
        assert!(parse("https://").is_none());
        assert!(parse("example.com").is_none());
        assert_eq!(parts("mailto:Me@Example.com?subject=hi"), ("".into(), "Me@Example.com".into(), "?subject=hi".into()));
    }

    #[test]
    fn decodes_like_decode_uri() {
        assert_eq!(decode_uri("/a%20b%2Fc%C3%A9").as_deref(), Some("/a b%2Fcé"));
        assert_eq!(decode_uri("%"), None);
        assert_eq!(decode_uri("%zz"), None);
        assert_eq!(decode_uri("%C3"), None);
        assert_eq!(decode_uri("%80"), None);
        assert_eq!(decode_uri("%ED%A0%80"), None);
    }
}
