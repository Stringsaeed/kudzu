//! URL presentation only; never used for opening or validation.
// URL display only, never used for opening/validation. Preserve decodeURI's reserved escapes.
fn decode_uri(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let pair = s.get(i + 1..i + 3)?;
            let n = u8::from_str_radix(pair, 16).ok()?;
            if b";/?:@&=+$,#".contains(&n) {
                out.extend_from_slice(&b[i..i + 3]);
            } else {
                out.push(n);
            }
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
pub(super) fn link_parts(text: &str) -> (String, String) {
    let cleaned = text.replace(['\t', '\n', '\r'], "");
    let url = if cleaned
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("www."))
    {
        format!("https://{cleaned}")
    } else {
        cleaned
    };
    let fallback = || (text.into(), text.into());
    // Deliberate deviation: display keeps Unicode hostnames. This is not a full WHATWG
    // parser and does not implement IDNA/punycode; never use it to validate/open a URL.
    let special = url.split_once(':').is_some_and(|(scheme, _)| {
        matches!(
            scheme.to_ascii_lowercase().as_str(),
            "http" | "https" | "ftp" | "ws" | "wss" | "file"
        )
    });
    let url = if special {
        let end = url.find(['?', '#']).unwrap_or(url.len());
        format!("{}{}", url[..end].replace('\\', "/"), &url[end..])
    } else {
        url
    };
    let Some((scheme, body)) = url.split_once(':') else {
        return fallback();
    };
    if scheme.eq_ignore_ascii_case("mailto") {
        return (
            body.split(['?', '#']).next().unwrap_or("").into(),
            "Email address".into(),
        );
    }
    let body = if special {
        body.trim_start_matches('/')
    } else {
        let Some(body) = body.strip_prefix("//") else {
            return fallback();
        };
        body
    };
    let end = body.find(['/', '?', '#']).unwrap_or(body.len());
    let host = body[..end].rsplit('@').next().unwrap_or("").to_lowercase();
    if host.is_empty() || host.contains(char::is_whitespace) {
        return fallback();
    }
    let mut host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let port = match scheme.to_ascii_lowercase().as_str() {
        "https" => ":443",
        "http" => ":80",
        "ftp" => ":21",
        _ => "",
    };
    if !port.is_empty() && host.ends_with(port) {
        host.truncate(host.len() - port.len());
    }
    let rest = body[end..].split('#').next().unwrap_or("");
    // URL.search omits a bare trailing question mark.
    let rest = if rest.split_once('?').is_some_and(|(_, q)| q.is_empty()) {
        rest.strip_suffix('?').unwrap_or(rest)
    } else {
        rest
    };
    let rest = if rest.starts_with('?') {
        format!("/{rest}")
    } else {
        rest.into()
    };
    let Some(mut rest) = decode_uri(&rest) else {
        return fallback();
    };
    let (path, query) = rest.split_once('?').unwrap_or((&rest, ""));
    if let Some(path) = path.strip_prefix('/') {
        let mut segments = Vec::new();
        let mut parts = path.split('/').peekable();
        while let Some(part) = parts.next() {
            match part {
                "." => {}
                ".." => {
                    segments.pop();
                }
                _ => {
                    segments.push(part);
                    continue;
                }
            }
            if parts.peek().is_none() {
                segments.push("");
            }
        }
        let normalized = format!("/{}", segments.join("/"));
        rest = if rest.contains('?') {
            format!("{normalized}?{query}")
        } else {
            normalized
        };
    }
    if rest.ends_with('/') {
        rest.pop();
    }
    (host, if rest.is_empty() { text.into() } else { rest })
}
