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
fn whitespace_collapses_outside_pre() {
    assert_eq!(render("  a \n  <b> b </b>  c  ").0, "a b c");
    assert_eq!(render("<pre>a\n  b</pre>after").0, "a\n  b\nafter");
    assert_eq!(render("<p>one</p>\n<p>two</p>").0, "one\n\ntwo");
    assert_eq!(render("a<br>b<br/><br>c").0, "a\nb\n\nc");
}

#[test]
fn spans_exclude_surrounding_breaks_and_spaces() {
    let (text, spans) = render("<p>x</p><p><b> bold </b></p>");
    assert_eq!(text, "x\n\nbold");
    assert_eq!(spans, vec![("bold".into(), FormatStyle::Bold)]);
}

#[test]
fn implicit_closes_and_entities() {
    assert_eq!(render("<ul><li>a<li>b</ul>").0, "• a\n• b");
    assert_eq!(render("<p>a<p>b").0, "a\n\nb");
    assert_eq!(
        render("Tom &amp; Jerry &copy; &#x1F600; &bogus; &").0,
        "Tom & Jerry © 😀 &bogus; &"
    );
    assert_eq!(render("<!-- c --><!DOCTYPE html>x<?php ?>y").0, "xy");
    assert_eq!(render("<a href=\"javascript:x\">t</a>").1, vec![]);
}
