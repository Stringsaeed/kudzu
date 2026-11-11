//! A small reader for XML property lists, enough for `.textexpander` files.
//!
//! Binary property lists (`bplist00`) are not supported; callers detect them first.

/// A property-list value. Dates are kept as their ISO-8601 text.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Value {
    Dict(Vec<(String, Value)>),
    Array(Vec<Value>),
    String(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    Date(String),
    Data(String),
}

impl Value {
    pub(super) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Dict(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(super) fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub(super) fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            #[allow(clippy::cast_possible_truncation)]
            Value::Real(r) => Some(*r as i64),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(n) => Some(*n != 0),
            _ => None,
        }
    }

    pub(super) fn as_date(&self) -> Option<&str> {
        match self {
            Value::Date(d) => Some(d),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Open(String),
    SelfClosing(String),
    Close(String),
    Text(String),
}

pub(super) fn parse(xml: &str) -> Result<Value, String> {
    let tokens = tokenize(xml)?;
    let mut pos = 0;
    // Skip to the first element inside `<plist>`.
    skip_text(&tokens, &mut pos);
    while let Some(Token::Open(name) | Token::SelfClosing(name)) = tokens.get(pos) {
        if name == "plist" {
            pos += 1;
        } else {
            break;
        }
    }
    let value = parse_value(&tokens, &mut pos, 0)?;
    Ok(value)
}

const MAX_DEPTH: usize = 64;

fn skip_text(tokens: &[Token], pos: &mut usize) {
    while matches!(tokens.get(*pos), Some(Token::Text(t)) if t.trim().is_empty()) {
        *pos += 1;
    }
}

fn parse_value(tokens: &[Token], pos: &mut usize, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("the property list is nested too deeply".into());
    }
    skip_text(tokens, pos);
    let token = tokens.get(*pos).ok_or("the property list ended early")?;
    *pos += 1;
    match token {
        Token::SelfClosing(name) => match name.as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            "string" => Ok(Value::String(String::new())),
            "dict" => Ok(Value::Dict(vec![])),
            "array" => Ok(Value::Array(vec![])),
            "data" => Ok(Value::Data(String::new())),
            other => Err(format!("unexpected <{other}/>")),
        },
        Token::Open(name) => match name.as_str() {
            "dict" => {
                let mut entries = Vec::new();
                loop {
                    skip_text(tokens, pos);
                    match tokens.get(*pos) {
                        Some(Token::Close(n)) if n == "dict" => {
                            *pos += 1;
                            return Ok(Value::Dict(entries));
                        }
                        Some(Token::Open(n)) if n == "key" => {
                            *pos += 1;
                            let key = read_text(tokens, pos, "key")?;
                            let value = parse_value(tokens, pos, depth + 1)?;
                            entries.push((key, value));
                        }
                        Some(Token::SelfClosing(n)) if n == "key" => {
                            *pos += 1;
                            let value = parse_value(tokens, pos, depth + 1)?;
                            entries.push((String::new(), value));
                        }
                        _ => return Err("expected <key> inside <dict>".into()),
                    }
                }
            }
            "array" => {
                let mut items = Vec::new();
                loop {
                    skip_text(tokens, pos);
                    if matches!(tokens.get(*pos), Some(Token::Close(n)) if n == "array") {
                        *pos += 1;
                        return Ok(Value::Array(items));
                    }
                    if *pos >= tokens.len() {
                        return Err("the property list ended inside an <array>".into());
                    }
                    items.push(parse_value(tokens, pos, depth + 1)?);
                }
            }
            "string" => Ok(Value::String(read_text(tokens, pos, "string")?)),
            "integer" => {
                let text = read_text(tokens, pos, "integer")?;
                text.trim()
                    .parse()
                    .map(Value::Int)
                    .map_err(|_| format!("bad integer '{}'", text.trim()))
            }
            "real" => {
                let text = read_text(tokens, pos, "real")?;
                text.trim()
                    .parse()
                    .map(Value::Real)
                    .map_err(|_| format!("bad number '{}'", text.trim()))
            }
            "date" => Ok(Value::Date(
                read_text(tokens, pos, "date")?.trim().to_string(),
            )),
            "data" => Ok(Value::Data(read_text(tokens, pos, "data")?)),
            "true" | "false" => {
                read_text(tokens, pos, name)?;
                Ok(Value::Bool(name == "true"))
            }
            other => Err(format!("unexpected <{other}>")),
        },
        Token::Close(name) => Err(format!("unexpected </{name}>")),
        Token::Text(_) => Err("unexpected text".into()),
    }
}

/// Reads text up to the matching close tag.
fn read_text(tokens: &[Token], pos: &mut usize, name: &str) -> Result<String, String> {
    let mut text = String::new();
    loop {
        match tokens.get(*pos) {
            Some(Token::Text(t)) => {
                text.push_str(t);
                *pos += 1;
            }
            Some(Token::Close(n)) if n == name => {
                *pos += 1;
                return Ok(text);
            }
            _ => return Err(format!("<{name}> was never closed")),
        }
    }
}

fn tokenize(xml: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut rest = xml;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<![CDATA[") {
            let end = after.find("]]>").ok_or("unterminated CDATA section")?;
            tokens.push(Token::Text(normalize_newlines(&after[..end])));
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").ok_or("unterminated comment")?;
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("<?") {
            let end = after.find("?>").ok_or("unterminated declaration")?;
            rest = &after[end + 2..];
        } else if rest.starts_with("<!") {
            // DOCTYPE, possibly with an internal subset in brackets.
            let mut depth = 0;
            let mut end = None;
            for (i, c) in rest.char_indices() {
                match c {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    '>' if depth <= 0 => {
                        end = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            rest = &rest[end.ok_or("unterminated declaration")? + 1..];
        } else if let Some(after) = rest.strip_prefix("</") {
            let end = after.find('>').ok_or("unterminated tag")?;
            tokens.push(Token::Close(after[..end].trim().to_string()));
            rest = &after[end + 1..];
        } else if let Some(after) = rest.strip_prefix('<') {
            let end = after.find('>').ok_or("unterminated tag")?;
            let body = after[..end].trim();
            let (body, self_closing) = match body.strip_suffix('/') {
                Some(b) => (b.trim_end(), true),
                None => (body, false),
            };
            let name = body.split_whitespace().next().unwrap_or("").to_string();
            tokens.push(if self_closing {
                Token::SelfClosing(name)
            } else {
                Token::Open(name)
            });
            rest = &after[end + 1..];
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            tokens.push(Token::Text(decode_entities(&normalize_newlines(
                &rest[..end],
            ))));
            rest = &rest[end..];
        }
    }
    Ok(tokens)
}

/// XML parsers read `\r\n` and a lone `\r` as `\n`; `&#13;` survives because it is decoded later.
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let decoded = after.find(';').filter(|&n| n <= 10).and_then(|n| {
            let entity = &after[..n];
            let ch = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32)
                    .filter(|c| *c != '\0'),
            };
            ch.map(|c| (c, n))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_values_and_entities() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>name</key><string>Tom &amp; Jerry &#233; &#x1F600;</string>
  <key>n</key><integer>-3</integer>
  <key>on</key><true/>
  <key>empty</key><string/>
  <key>when</key><date>2020-01-02T03:04:05Z</date>
  <key>list</key><array><string>a</string><dict><key>k</key><false/></dict></array>
</dict>
</plist>"#;
        let value = parse(xml).unwrap();
        assert_eq!(
            value.get("name").and_then(Value::as_str),
            Some("Tom & Jerry é 😀")
        );
        assert_eq!(value.get("n").and_then(Value::as_int), Some(-3));
        assert_eq!(value.get("on").and_then(Value::as_bool), Some(true));
        assert_eq!(value.get("empty").and_then(Value::as_str), Some(""));
        assert_eq!(
            value.get("when").and_then(Value::as_date),
            Some("2020-01-02T03:04:05Z")
        );
        let list = value.get("list").and_then(Value::as_array).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].get("k").and_then(Value::as_bool), Some(false));
    }

    #[test]
    fn keeps_whitespace_and_cdata_inside_strings() {
        let xml =
            "<plist><dict><key>t</key><string>  a\n  <![CDATA[<b>]]> </string></dict></plist>";
        let value = parse(xml).unwrap();
        assert_eq!(value.get("t").and_then(Value::as_str), Some("  a\n  <b> "));
    }

    #[test]
    fn normalizes_line_endings_but_keeps_character_references() {
        let xml = "<plist><dict><key>t</key><string>a\r\nb\rc&#13;d&#0;e</string></dict></plist>";
        let value = parse(xml).unwrap();
        assert_eq!(
            value.get("t").and_then(Value::as_str),
            Some("a\nb\nc\rd&#0;e")
        );
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(parse("<plist><dict><key>a</key>").is_err());
        assert!(parse("<plist><dict><string>x</string></dict></plist>").is_err());
        assert!(parse("").is_err());
        let deep = format!("{}{}", "<array>".repeat(200), "</array>".repeat(200));
        assert!(parse(&deep).is_err());
    }
}
