use super::*;

fn convert_plain(raw: &str) -> (String, Vec<String>) {
    let mut problems = Vec::new();
    let out = convert(raw, &HashMap::new(), &mut problems);
    (out, problems)
}

#[test]
fn date_specifiers_merge_into_one_pattern() {
    let (out, problems) = convert_plain("%A, %B %1d, %Y at %H:%M");
    assert!(problems.is_empty());
    assert_eq!(
        out,
        "{{date format=\"EEEE, MMMM d, yyyy\"}} at {{date format=\"HH:mm\"}}"
    );
}

#[test]
fn percent_signs_that_are_not_macros_stay_literal() {
    let (out, problems) = convert_plain("100% sure, 50%, %q, %");
    assert!(problems.is_empty());
    assert_eq!(out, "100% sure, 50%, %q, %");
}

#[test]
fn fill_macros_become_form_tags() {
    let (out, problems) = convert_plain(
        "Hi %filltext:name=Customer:default=Jo%, %fillarea:name=Note:width=40%, %fillpopup:name=Tone:Formal:default=Casual%",
    );
    assert!(problems.is_empty());
    assert_eq!(
        out,
        "Hi {{field name=\"Customer\" default=\"Jo\"}}, {{field name=\"Note\" multiline=true}}, {{choice name=\"Tone\" options=\"Casual|Formal\" default=\"Casual\"}}"
    );
}

#[test]
fn unsupported_macros_warn_and_stay_text() {
    let (out, problems) = convert_plain("%key:f13% and %calc:1+1% and %@+1D (no date)");
    assert_eq!(problems.len(), 3);
    assert_eq!(out, "%key:f13% and %calc:1+1% and %@+1D (no date)");
}

#[test]
fn key_macros_become_key_tags() {
    let (out, problems) =
        convert_plain("user%key:tab%pass%key:enter%%key:return%%key:esc%%key:Tab%");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        out,
        "user{{key name=\"tab\"}}pass{{key name=\"return\"}}{{key name=\"return\"}}{{key name=\"escape\"}}{{key name=\"tab\"}}"
    );
}

#[test]
fn date_math_becomes_an_offset() {
    let (out, problems) = convert_plain("%@+1D%A, %@-2M%B %@+3m%H:%M");
    assert!(problems.is_empty());
    assert_eq!(
        out,
        "{{date format=\"EEEE\" offset=\"+1d\"}}, {{date format=\"MMMM\" offset=\"-2m\"}} {{date format=\"HH:mm\" offset=\"+3min\"}}"
    );
}

#[test]
fn optional_sections_are_always_included() {
    let (out, problems) =
        convert_plain("A%fillpart:name=Opt:default=yes% extra %clipboard%fillpartend%B");
    assert_eq!(out, "A extra {{clipboard}}B");
    assert_eq!(problems.len(), 1);
    let (out, _) = convert_plain("A%fillpart:name=Opt%B");
    assert_eq!(out, "AB");
}
