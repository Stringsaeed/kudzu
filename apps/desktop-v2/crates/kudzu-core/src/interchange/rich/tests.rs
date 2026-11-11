use super::*;

fn identity(rich: &RichText) -> Converted {
    to_template(rich, &[], |s| {
        crate::template::serialize(&[Node::Text {
            text: s.to_string(),
            range: 0..0,
        }])
    })
}

#[test]
fn spans_snap_out_of_placeholders_and_graphemes() {
    let mut rich = RichText::default();
    rich.push("a {{date}} e\u{301}x");
    // Starts inside the placeholder, ends inside the combining sequence.
    rich.span(4, 13, FormatStyle::Bold);
    let atoms = espanso_atoms(&rich.text);
    assert_eq!(atoms, vec![2..10]);
    let out = to_template(&rich, &atoms, |s| {
        s.replace("{{date}}", "{{date format=\"y\"}}")
    });
    assert_eq!(out.content, "a {{date format=\"y\"}} e\u{301}x");
    assert_eq!(
        out.formatting.spans,
        vec![FormatSpan {
            start: 2,
            end: 24,
            style: FormatStyle::Bold
        }]
    );
}

#[test]
fn literal_braces_are_escaped_and_never_split() {
    let mut rich = RichText::default();
    rich.push("x{{y");
    rich.span(2, 4, FormatStyle::Italic);
    let out = identity(&rich);
    assert_eq!(out.content, "x\\{{y");
    assert_eq!(
        out.formatting.spans,
        vec![FormatSpan {
            start: 1,
            end: 5,
            style: FormatStyle::Italic
        }]
    );
}

#[test]
fn adjacent_runs_merge() {
    let mut rich = RichText::default();
    rich.push("abc");
    rich.span(0, 1, FormatStyle::Bold);
    rich.span(1, 2, FormatStyle::Bold);
    rich.span(2, 3, FormatStyle::Bold);
    assert_eq!(rich.spans.len(), 1);
}

#[test]
fn textexpander_atoms_cover_macros() {
    let text = "a %filltext:name=x% %m/%d/%Y b %@+1D%A %fillpart:n%in%fillpartend% 50% %|";
    let atoms: Vec<&str> = textexpander_atoms(text)
        .into_iter()
        .map(|r| &text[r])
        .collect();
    assert_eq!(
        atoms,
        vec![
            "%filltext:name=x%",
            "%m/%d/%Y",
            "%@+1D%A",
            "%fillpart:n%in%fillpartend%",
            "%|"
        ]
    );
}

#[test]
fn warning_lists_what_was_dropped() {
    let mut rich = RichText::default();
    assert_eq!(rich.warning(), None);
    rich.drop(Dropped::Colors);
    rich.drop(Dropped::Fonts);
    rich.drop(Dropped::Tables);
    assert_eq!(
        rich.warning().unwrap(),
        "Formatting imported, except fonts, colors and table layout (cells are separated by tabs)."
    );
}
