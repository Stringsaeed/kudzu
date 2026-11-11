//! A small RFC 4180 reader: quoted fields, doubled quotes, newlines inside quotes.

/// Splits `text` into rows of fields. Blank lines are dropped; a leading BOM is ignored.
pub(super) fn parse(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let delimiter = detect_delimiter(text);
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut field_started = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if !field_started => {
                quoted = true;
                field_started = true;
            }
            c if c == delimiter => {
                row.push(std::mem::take(&mut field));
                field_started = false;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                finish_row(&mut rows, &mut row, &mut field, &mut field_started);
            }
            _ => {
                field.push(c);
                field_started = true;
            }
        }
    }
    finish_row(&mut rows, &mut row, &mut field, &mut field_started);
    rows
}

fn finish_row(
    rows: &mut Vec<Vec<String>>,
    row: &mut Vec<String>,
    field: &mut String,
    field_started: &mut bool,
) {
    if !row.is_empty() || *field_started || !field.is_empty() {
        row.push(std::mem::take(field));
    }
    *field_started = false;
    let done = std::mem::take(row);
    if !done.is_empty() && !(done.len() == 1 && done[0].is_empty()) {
        rows.push(done);
    }
}

/// Comma unless the first line clearly uses semicolons or tabs (spreadsheet locales).
/// Delimiters inside quoted fields don't count.
fn detect_delimiter(text: &str) -> char {
    let (mut commas, mut semis, mut tabs) = (0, 0, 0);
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' | '\r' if !quoted => break,
            ',' if !quoted => commas += 1,
            ';' if !quoted => semis += 1,
            '\t' if !quoted => tabs += 1,
            _ => {}
        }
    }
    if tabs > commas && tabs >= semis {
        '\t'
    } else if semis > commas {
        ';'
    } else {
        ','
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_quotes_newlines_and_crlf() {
        let rows = parse("\u{feff}a,b\r\n\"x, \"\"y\"\"\",\"line1\nline2\"\r\n\r\nlast,\n");
        assert_eq!(
            rows,
            vec![
                vec!["a", "b"],
                vec!["x, \"y\"", "line1\nline2"],
                vec!["last", ""],
            ]
        );
    }

    #[test]
    fn keeps_empty_quoted_field_and_unterminated_tail() {
        assert_eq!(parse("a,\"\""), vec![vec!["a", ""]]);
        assert_eq!(parse("a,\"open"), vec![vec!["a", "open"]]);
        assert!(parse("").is_empty());
    }

    #[test]
    fn delimiters_inside_quotes_do_not_count() {
        assert_eq!(
            parse("sig,\"Hello; how; are; you\",Signature\nb,plain,Lab"),
            vec![vec!["sig", "Hello; how; are; you", "Signature"], vec!["b", "plain", "Lab"]]
        );
    }

    #[test]
    fn detects_semicolon_delimiter() {
        assert_eq!(parse("a;b\nc;d"), vec![vec!["a", "b"], vec!["c", "d"]]);
    }
}
