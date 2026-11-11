use super::*;

fn render(source: &str) -> (String, Vec<(String, FormatStyle)>) {
    let rich = read(source).unwrap();
    let mut spans: Vec<_> = rich
        .spans
        .iter()
        .map(|(r, s)| (rich.text[r.clone()].to_string(), s.clone()))
        .collect();
    spans.sort();
    (rich.text, spans)
}

#[test]
fn emphasis_rules() {
    assert_eq!(render("a * b * c").1, vec![]);
    assert_eq!(render("snake_case_name").1, vec![]);
    assert_eq!(
        render("**a *b* c**").1,
        vec![
            ("a b c".into(), FormatStyle::Bold),
            ("b".into(), FormatStyle::Italic)
        ]
    );
    assert_eq!(
        render("*a **b** c*").1,
        vec![
            ("a b c".into(), FormatStyle::Italic),
            ("b".into(), FormatStyle::Bold)
        ]
    );
    assert_eq!(render("**unclosed").0, "**unclosed");
    assert_eq!(render("2 * 3 = 6 and 4*5").0, "2 * 3 = 6 and 4*5");
    assert_eq!(render("_a_b").0, "_a_b");
    assert_eq!(render(r"\*not\*").0, "*not*");
}

#[test]
fn links_and_inline_html() {
    let (text, spans) = render("[**x**](https://a.b) <u>u</u> <span>s</span><br>end");
    assert_eq!(text, "x u s\nend");
    assert_eq!(
        spans,
        vec![
            ("u".into(), FormatStyle::Underline),
            ("x".into(), FormatStyle::Bold),
            (
                "x".into(),
                FormatStyle::Link {
                    url: "https://a.b".into()
                }
            ),
        ]
    );
    assert_eq!(render("[a] (b) [c](d e)").0, "[a] (b) c");
    assert_eq!(render("a < b > c").0, "a < b > c");
}

#[test]
fn code_is_literal() {
    assert_eq!(render("`**x**` ``a`b``").0, "**x** a`b");
    assert_eq!(render("```\n**x**\n```\nafter").0, "**x**\nafter");
}
