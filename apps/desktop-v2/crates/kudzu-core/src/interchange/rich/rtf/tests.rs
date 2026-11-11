use super::*;

fn render(source: &str) -> (String, Vec<(String, FormatStyle)>, Vec<Dropped>) {
    let rich = read(source).unwrap();
    let mut spans: Vec<_> = rich
        .spans
        .iter()
        .map(|(r, s)| (rich.text[r.clone()].to_string(), s.clone()))
        .collect();
    spans.sort();
    (rich.text, spans, rich.dropped.into_iter().collect())
}

#[test]
fn characters_and_escapes() {
    let (text, _, _) = render(
        r"{\rtf1\ansi caf\'e9 \'93q\'94 \{x\} a\\b\tab c\line d\~e\u8364?f\uc0\u55357\u56832 }",
    );
    assert_eq!(text, "café “q” {x} a\\b\tc\nd\u{a0}e€f😀");
}

#[test]
fn styles_and_plain() {
    let (text, spans, _) = render(r"{\rtf1 \b bold \i both\plain  none {\ul u}\ul0  x}");
    assert_eq!(text, "bold both none u x");
    assert_eq!(
        spans,
        vec![
            ("bold both".into(), FormatStyle::Bold),
            ("both".into(), FormatStyle::Italic),
            ("u".into(), FormatStyle::Underline),
        ]
    );
}

#[test]
fn unsafe_links_keep_their_text() {
    let (text, spans, dropped) =
        render(r#"{\rtf1 {\field{\*\fldinst{HYPERLINK "file:///etc"}}{\fldrslt here}}}"#);
    assert_eq!(text, "here");
    assert!(spans.is_empty());
    assert_eq!(dropped, vec![Dropped::Links]);
}

#[test]
fn rejects_what_is_not_rtf() {
    assert!(read("hello").is_err());
    assert!(read(r"{\rtf1 {\b x}").is_err());
    assert!(read(r"{\rtf1 x}}").is_ok());
}
