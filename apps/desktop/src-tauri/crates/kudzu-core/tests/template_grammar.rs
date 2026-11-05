//! Grammar edge cases and property round trips (ported from TemplateGrammarTests.swift).

mod template_support;

use kudzu_core::template::{self, Attribute, AttributeValue, Node, Severity, Tag, TemplateTag};
use template_support::*;
use uuid::Uuid;

fn first_tag(source: &str) -> Tag {
    match template::parse(source).nodes.into_iter().next() {
        Some(Node::Tag(tag)) => tag,
        other => panic!("expected a tag in {source}, got {other:?}"),
    }
}

// Bare numbers

#[test]
fn bare_number_is_accepted_for_string_attributes() {
    let parsed = template::parse("{{field name=qty default=5}}");
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(
        template::typed(&first_tag("{{field name=qty default=5}}")),
        TemplateTag::Field {
            name: "qty".into(),
            default: Some("5".into()),
            multiline: false
        }
    );
    assert_eq!(render("{{field name=qty default=5}}"), "5");
    assert_eq!(
        canonical("{{field name=qty default=5}}"),
        r#"{{field name="qty" default="5"}}"#
    );
}

#[test]
fn bare_tokens_keep_their_written_text_in_string_attributes() {
    for token in [
        "5", "-3", "+7", "007", "1.50", "-0.25", "10", "true", "false",
    ] {
        assert_eq!(
            render(&format!("{{{{field name=x default={token}}}}}")),
            token
        );
        assert_eq!(
            render(&format!(
                "{{{{choice name=x options=\"a|b\" default={token}}}}}"
            )),
            token
        );
        assert!(
            template::parse(&format!("{{{{field name=x default={token}}}}}"))
                .diagnostics
                .is_empty()
        );
    }
}

#[test]
fn bare_numbers_work_in_every_string_attribute() {
    assert!(template::parse("{{field name=7}}").diagnostics.is_empty());
    assert!(!template::parse("{{choice name=2 options=1|2}}")
        .diagnostics
        .is_empty());
    assert!(template::parse("{{choice name=2 options=12 default=1}}")
        .diagnostics
        .is_empty());
    assert!(template::parse("{{date format=5}}").diagnostics.is_empty());
    assert_eq!(render("{{date format=5}}"), "5");
    assert!(template::parse("{{date locale=5}}").diagnostics.is_empty());
}

#[test]
fn numbers_still_cannot_be_booleans_or_ids() {
    assert_eq!(render("{{clipboard trim=1}}"), "{{clipboard trim=1}}");
    assert_eq!(
        render("{{field name=x multiline=0}}"),
        "{{field name=x multiline=0}}"
    );
    assert_eq!(render("{{snippet id=5}}"), "{{snippet id=5}}");
    assert!(!template::parse("{{clipboard trim=1}}")
        .diagnostics
        .is_empty());
}

#[test]
fn invalid_offset_numbers_still_warn_once_and_are_ignored() {
    let result = Fixture::new().render("{{date offset=3}}");
    assert_eq!(result.text, "2024-01-31");
    assert_eq!(result.diagnostics.len(), 1);
}

#[test]
fn unknown_attributes_keep_typed_numbers() {
    let attribute = |source: &str| first_tag(source).attributes.last().unwrap().value.clone();
    assert_eq!(attribute("{{cursor n=5}}"), AttributeValue::Number(5.0));
    assert_eq!(attribute("{{cursor n=-2.5}}"), AttributeValue::Number(-2.5));
    assert_eq!(attribute("{{cursor n=+3}}"), AttributeValue::Number(3.0));
    for token in ["1e5", ".5", "5.", "0x10", "1_000", "5d", "--5", "+-5"] {
        assert_eq!(
            attribute(&format!("{{{{cursor n={token}}}}}")),
            AttributeValue::String(token.into()),
            "{token}"
        );
    }
}

#[test]
fn numbers_serialize_as_plain_decimals_and_round_trip() {
    for value in [
        0.0,
        42.0,
        -7.0,
        2.5,
        -0.125,
        1e20,
        123_456_789_012_345_678.0,
        1e-7,
        0.1,
        1e300,
    ] {
        let node = Node::Tag(Tag {
            name: "cursor".into(),
            attributes: vec![Attribute {
                name: "n".into(),
                value: AttributeValue::Number(value),
            }],
            range: 0..0,
            verbatim: None,
        });
        let source = template::serialize(&[node]);
        assert!(!source.contains('e'), "{source}");
        let parsed = template::parse(&source);
        assert!(parsed.diagnostics.is_empty());
        assert_eq!(
            first_tag(&source).attributes[0].value,
            AttributeValue::Number(value),
            "{source}"
        );
    }
    assert_eq!(canonical("{{cursor n=42}}"), "{{cursor n=42}}");
}

// Literal brace before a tag

#[test]
fn literal_brace_before_tag() {
    let cases = [
        ("{{{date}}", "{2024-01-31"),
        ("{{{date}}}", "{2024-01-31}"),
        ("{{{{date}}", "{{2024-01-31"),
        ("a{{{clipboard}}b", "a{clipb"),
        ("{{{", "{{{"),
        ("{{{x", "{{{x"),
        ("{{{{", "{{{{"),
    ];
    for (source, expected) in cases {
        assert_eq!(render(source), expected, "{source}");
    }
}

#[test]
fn brace_before_tag_parses_as_text_then_tag() {
    assert_eq!(
        semantic(&template::parse("{{{date}}").nodes),
        vec![text("{"), tag("date", vec![])]
    );
    let ranges: Vec<_> = template::parse("é{{{date}}")
        .nodes
        .iter()
        .map(|n| match n {
            Node::Text { range, .. } => range.clone(),
            Node::Tag(t) => t.range.clone(),
        })
        .collect();
    assert_eq!(ranges, vec![0..2, 2..10]);
}

#[test]
fn serializer_emits_round_tripping_brace_escapes() {
    let date = tag("date", vec![]);
    assert_eq!(template::serialize(&[text("{"), date.clone()]), "{{{date}}");
    assert_eq!(
        template::serialize(&[text("{{"), date.clone()]),
        "\\{{{{date}}"
    );
    assert_eq!(
        template::serialize(&[text("{{{"), date.clone()]),
        "\\{{{{{date}}"
    );
    assert_eq!(template::serialize(&[text("a{{{b")]), "a\\{{{b");
    assert_eq!(template::serialize(&[text("{"), text("{")]), "\\{{");
    assert_eq!(
        template::serialize(&[text("{"), text("{"), date.clone()]),
        "\\{{{{date}}"
    );
    let cases = [
        vec![text("{"), date.clone()],
        vec![text("{{"), date.clone()],
        vec![text("\\{"), date.clone()],
        vec![text("{{{"), date.clone(), text("{")],
    ];
    for nodes in cases {
        let source = template::serialize(&nodes);
        assert_eq!(
            semantic(&template::parse(&source).nodes),
            semantic(&nodes),
            "{source}"
        );
    }
}

#[test]
fn existing_valid_sources_keep_their_meaning() {
    let cases = [
        (r"\{{", "{{"),
        (r"\{{cursor}}", "{{cursor}}"),
        (r"\\", r"\"),
        (r"\\\\", r"\\"),
        (r"\{x", r"\{x"),
        (r"a\{", r"a\{"),
        (r"\{ \}", r"\{ \}"),
        (r"\}", r"\}"),
        (r"\x", r"\x"),
        (r"\\{{cursor}}", r"\"),
        (r"\\\{{cursor}}", r"\{{cursor}}"),
        (r"\{{{date}}", "{{{date}}"),
        (r"\{{{{date}}", "{{2024-01-31"),
        ("a{b", "a{b"),
        ("{ {{cursor}}", "{ "),
        ("{}", "{}"),
        ("}}{{cursor}}{", "}}{"),
        (r#"{{field name="{{{"}}"#, ""),
        (r#"{{field name="{{{" default="x"}}"#, "x"),
    ];
    for (source, expected) in cases {
        assert_eq!(render(source), expected, "{source}");
    }
}

#[test]
fn serializer_output_matches_the_legacy_serializer_for_ordinary_text() {
    for value in [
        "", "abc", "{", "}", "a{b", "{{", "\\", "\\\\", "\\{{", "x\\{{y", "{{{", "{{{{", "😀{{é",
    ] {
        let legacy = value.replace('\\', "\\\\").replace("{{", "\\{{");
        assert_eq!(template::serialize(&[text(value)]), legacy, "{value}");
    }
}

// Unrecognized tags stay verbatim

const UNRECOGNIZED: &[&str] = &[
    "{{DATE}}",
    "{{unknown}}",
    "{{ future  size = 3 mode=\"x\" }}",
    "{{field}}",
    "{{choice}}",
    "{{clipboard trim=oops}}",
    "{{snippet id=nope}}",
    "{{date format=}}",
    "{{ }}",
    "{{}}",
    "{{9}}",
    "{{a-b}}",
    "{{date format=\"y\" format=\"z\" bad}}",
    "{{field name=x multiline=\"yes\"}}",
];

#[test]
fn unrecognized_tags_survive_a_save_verbatim() {
    for tag in UNRECOGNIZED {
        for source in [
            tag.to_string(),
            format!("a{tag}"),
            format!("{tag}b"),
            format!("a{tag}{{{{cursor}}}}{tag}\\\\"),
        ] {
            let parsed = template::parse(&source);
            assert_eq!(template::serialize(&parsed.nodes), source);
            assert!(
                parsed
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == Severity::Error),
                "{source}"
            );
        }
    }
}

#[test]
fn unrecognized_tags_render_exactly_as_they_always_did() {
    for tag in UNRECOGNIZED {
        assert_eq!(render(tag), *tag);
        assert_eq!(
            render(&format!("x{tag}{{{{cursor}}}}y")),
            format!("x{tag}y")
        );
        assert_eq!(render(&canonical(tag)), render(tag));
    }
}

#[test]
fn unrecognized_tag_is_a_tag_node_without_recognized_meaning() {
    let parsed = template::parse("é{{DATE}}");
    assert_eq!(parsed.nodes.len(), 2);
    let Node::Tag(tag) = &parsed.nodes[1] else {
        panic!("expected a tag node")
    };
    assert_eq!(tag.verbatim.as_deref(), Some("{{DATE}}"));
    assert_eq!(template::typed(tag), TemplateTag::Unknown);
    assert_eq!(tag.range, 1..9);
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.range.clone())
            .collect::<Vec<_>>(),
        vec![1..9]
    );
}

#[test]
fn escaped_tag_syntax_is_still_escaped_text() {
    assert_eq!(canonical(r"\{{DATE}}"), r"\{{DATE}}");
    assert_eq!(canonical(r"\{{date}}"), r"\{{date}}");
    assert_eq!(template::serialize(&[text("{{DATE}}")]), r"\{{DATE}}");
    assert_eq!(template::serialize(&[text("{{date}}")]), r"\{{date}}");
}

#[test]
fn unterminated_tag_is_literal_text() {
    let parsed = template::parse("a{{date");
    assert_eq!(semantic(&parsed.nodes), vec![text("a{{date")]);
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["Unterminated tag"]
    );
    assert_eq!(render("a{{date"), "a{{date");
    let quoted = template::parse(r#"{{field name="oops}}"#);
    assert_eq!(
        semantic(&quoted.nodes),
        vec![text(r#"{{field name="oops}}"#)]
    );
    assert_eq!(
        canonical(r#"{{field name="oops}}"#),
        r#"\{{field name="oops}}"#
    );
}

#[test]
fn analysis_ignores_unrecognized_tags() {
    let info = template::analyze(&template::parse("{{field}}{{clipboard trim=oops}}{{DATE}}"));
    assert!(info.form_fields.is_empty() && !info.uses_clipboard && !info.has_cursor);
}

// Property-style round trips

fn random_text(rng: &mut SplitMix64) -> String {
    const ALPHABET: &[&str] = &[
        "a", "{", "}", "\\", "\"", " ", "😀", "\n", "|", "{{", "}}", "\\{", "\\\\",
    ];
    (0..rng.below(7)).map(|_| *rng.pick(ALPHABET)).collect()
}

fn random_tag(rng: &mut SplitMix64) -> Tag {
    let make = |name: &str, attributes: Vec<Attribute>| Tag {
        name: name.into(),
        attributes,
        range: 0..0,
        verbatim: None,
    };
    let string = |rng: &mut SplitMix64| AttributeValue::String(random_text(rng));
    match rng.below(8) {
        0 => make(
            "date",
            vec![Attribute {
                name: "format".into(),
                value: string(rng),
            }],
        ),
        1 => make("cursor", vec![]),
        2 => make(
            "clipboard",
            vec![Attribute {
                name: "trim".into(),
                value: AttributeValue::Bool(rng.bool()),
            }],
        ),
        3 => make(
            "field",
            vec![
                Attribute {
                    name: "name".into(),
                    value: string(rng),
                },
                Attribute {
                    name: "default".into(),
                    value: string(rng),
                },
            ],
        ),
        4 => make(
            "choice",
            vec![
                Attribute {
                    name: "name".into(),
                    value: AttributeValue::String("c".into()),
                },
                Attribute {
                    name: "options".into(),
                    value: string(rng),
                },
            ],
        ),
        5 => make(
            "snippet",
            vec![Attribute {
                name: "id".into(),
                value: AttributeValue::String(
                    Uuid::new_v4().hyphenated().to_string().to_uppercase(),
                ),
            }],
        ),
        6 => make(
            "time",
            vec![Attribute {
                name: "extra".into(),
                value: AttributeValue::Number(rng.below(1999) as f64 - 999.0),
            }],
        ),
        _ => Tag {
            name: String::new(),
            attributes: vec![],
            range: 0..0,
            verbatim: Some(
                rng.pick(&["{{DATE}}", "{{ nope }}", "{{field}}", "{{x y=1}}"])
                    .to_string(),
            ),
        },
    }
}

#[test]
fn generated_nodes_round_trip_through_the_serializer() {
    let mut rng = SplitMix64(20_260_413);
    for _ in 0..4000 {
        let mut nodes = Vec::new();
        for _ in 0..rng.below(7) {
            nodes.push(if rng.bool() {
                text(&random_text(&mut rng))
            } else {
                Node::Tag(random_tag(&mut rng))
            });
        }
        let source = template::serialize(&nodes);
        let parsed = template::parse(&source);
        assert_eq!(
            semantic(&parsed.nodes),
            semantic(&nodes),
            "source: {source}"
        );
        assert_eq!(template::serialize(&parsed.nodes), source);
        assert!(
            !parsed
                .diagnostics
                .iter()
                .any(|d| d.message == "Unterminated tag"),
            "source: {source}"
        );
    }
}

#[test]
fn arbitrary_source_is_stable_after_one_save() {
    let mut rng = SplitMix64(7);
    const PIECES: &[&str] = &[
        "a",
        "{",
        "{{",
        "{{{",
        "}",
        "}}",
        "\\",
        "\\\\",
        "\\{{",
        "\\{",
        "\"",
        " ",
        "=",
        "date",
        "x=1",
        "{{date}}",
        "{{cursor}}",
        "{{DATE}}",
        "{{field name=q default=5}}",
        "{{clipboard trim=true}}",
        "😀",
        "|",
        "{{unknown a=\"b\"}}",
    ];
    for _ in 0..6000 {
        let source: String = (0..rng.below(10)).map(|_| *rng.pick(PIECES)).collect();
        let first = template::parse(&source);
        let saved = template::serialize(&first.nodes);
        let second = template::parse(&saved);
        assert_eq!(
            semantic(&second.nodes),
            semantic(&first.nodes),
            "source: {source} saved: {saved}"
        );
        assert_eq!(
            template::serialize(&second.nodes),
            saved,
            "source: {source}"
        );
        assert_eq!(
            render(&saved),
            render(&source),
            "source: {source} saved: {saved}"
        );
    }
}

#[test]
fn brace_and_backslash_runs_next_to_tags_round_trip() {
    for braces in 0..=5 {
        for slashes in 0..=5 {
            for tail in 0..=3 {
                let prefix = format!("{}{}", "\\".repeat(slashes), "{".repeat(braces));
                let nodes = vec![
                    text(&prefix),
                    tag("cursor", vec![]),
                    text(&"}".repeat(tail)),
                    tag("clipboard", vec![]),
                ];
                let source = template::serialize(&nodes);
                assert_eq!(
                    semantic(&template::parse(&source).nodes),
                    semantic(&nodes),
                    "source: {source}"
                );
                let result = Fixture::new().render(&source);
                assert_eq!(
                    result.text,
                    format!("{prefix}{}clip", "}".repeat(tail)),
                    "source: {source}"
                );
                assert_eq!(result.cursor_offset, Some(prefix.encode_utf16().count()));
            }
        }
    }
}
