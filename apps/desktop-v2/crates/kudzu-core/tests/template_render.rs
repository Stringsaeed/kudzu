//! Rendering, ranges, diagnostics, nesting and date offsets (ported from TemplateTests.swift).

mod template_support;

use std::collections::HashMap;
use std::time::Instant;

use kudzu_core::template::{self, AttributeValue, DateOffset, Node, TemplateTag};
use template_support::*;
use uuid::Uuid;

fn base() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.clipboard = Some(" \nhello \n".into());
    fixture.form_values.insert("who".into(), "Ada".into());
    fixture
}

#[test]
fn rendering() {
    let cases = [
        ("hello", "hello"),
        (r"\{{cursor}}", "{{cursor}}"),
        ("{{clipboard trim=true}}", "hello"),
        ("{{field name=who}}", "Ada"),
        (r#"{{choice name=x options="a|b" default="b"}}"#, "b"),
        ("{{date}} {{time}}", "2024-01-31 00:00"),
        ("{{date offset=+1m}}", "2024-02-29"),
        ("{{date offset=oops}}", "2024-01-31"),
        ("{{unknown}}", "{{unknown}}"),
        ("{{field}}", "{{field}}"),
        (r#"{{field name=x default="a}}b"}}"#, "a}}b"),
    ];
    for (source, expected) in cases {
        assert_eq!(base().render(source).text, expected, "{source}");
    }
}

#[test]
fn ranges_and_diagnostics() {
    let parsed = template::parse("😀{{cursor}}{{cursor}}{{field name=x name=y}}");
    let range = |node: &Node| match node {
        Node::Text { range, .. } => range.clone(),
        Node::Tag(tag) => tag.range.clone(),
    };
    assert_eq!(range(&parsed.nodes[0]), 0..2);
    assert_eq!(range(&parsed.nodes[1]), 2..12);
    assert_eq!(parsed.diagnostics.len(), 2);
    assert_eq!(
        base().render("😀{{cursor}}x{{cursor}}").cursor_offset,
        Some(2)
    );
}

#[test]
fn malformed() {
    for source in [
        "{{",
        "{{date format=}}",
        "{{snippet id=nope}}",
        "{{field name=\"oops}}",
        "{{DATE}}",
    ] {
        let result = base().render(source);
        assert_eq!(result.text, source);
        assert!(!result.diagnostics.is_empty(), "{source}");
    }
}

#[test]
fn analysis() {
    let info = template::analyze(&template::parse(
        r#"{{field name=x multiline=true default="hi"}}{{choice name=y options="a\|b|c"}}{{field name=x}}{{clipboard}}{{cursor}}"#,
    ));
    assert!(!info.form_fields.is_empty() && info.has_cursor && info.uses_clipboard);
    assert_eq!(
        info.form_fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["x", "y"]
    );
    assert_eq!(info.form_fields[0].kind, template::FormFieldKind::Multiline);
    assert_eq!(info.form_fields[1].options, ["a|b", "c"]);
}

#[test]
fn nesting() {
    let id = Uuid::new_v4();
    let tag = format!("{{{{snippet id=\"{id}\"}}}}");
    let mut fixture = base();
    fixture
        .snippets
        .insert(id, "a{{cursor}}{{field name=who}}".into());
    let result = fixture.render(&format!("😀{tag}{{{{cursor}}}}"));
    assert_eq!(result.text, "😀aAda");
    assert_eq!(result.cursor_offset, Some(6));
    fixture.snippets.insert(id, tag.clone());
    assert!(!fixture.render(&tag).diagnostics.is_empty());
    assert!(!base().render(&tag).diagnostics.is_empty());
}

#[test]
fn uppercase_uuid_references_resolve() {
    let id = Uuid::new_v4();
    let mut fixture = base();
    fixture.snippets.insert(id, "ok".into());
    let upper = id.hyphenated().to_string().to_uppercase();
    assert_eq!(
        fixture
            .render(&format!("{{{{snippet id=\"{upper}\"}}}}"))
            .text,
        "ok"
    );
    // Only the hyphenated form is a valid id, as with Swift's `UUID(uuidString:)`.
    let simple = id.simple().to_string();
    assert_eq!(
        fixture
            .render(&format!("{{{{snippet id=\"{simple}\"}}}}"))
            .text,
        format!("{{{{snippet id=\"{simple}\"}}}}")
    );
}

#[test]
fn generated_round_trips() {
    let texts = ["", "abc", "😀é", "{{literal}}", "\\{{", "\\", "a\nb"];
    for i in 0..300 {
        let source = template::serialize(&[
            text(texts[i % texts.len()]),
            tag(
                "field",
                vec![
                    string_attr("name", "x"),
                    string_attr("default", texts[(i / 7) % texts.len()]),
                ],
            ),
            tag(
                "clipboard",
                vec![template::Attribute {
                    name: "trim".into(),
                    value: AttributeValue::Bool(i % 2 == 0),
                }],
            ),
        ]);
        let parsed = template::parse(&source);
        assert!(parsed.diagnostics.is_empty(), "{source}");
        assert_eq!(template::serialize(&parsed.nodes), source);
        assert_eq!(template::parse(&template::serialize(&parsed.nodes)), parsed);
    }
}

#[test]
fn performance_sanity() {
    let source = format!("{}{{{{clipboard}}}}", "x".repeat(40)).repeat(200);
    let _ = base().render(&source);
    let start = Instant::now();
    let _ = template::parse(&source);
    let result = base().render(&source);
    assert!(!result.text.is_empty());
    // Generous for unoptimized debug builds.
    assert!(start.elapsed().as_millis() < 250, "{:?}", start.elapsed());
}

#[test]
fn calendar_offsets() {
    let cases = [
        ("+3d", "2024-02-03 00:00"),
        ("-1w", "2024-01-24 00:00"),
        ("+2m", "2024-03-31 00:00"),
        ("+1y", "2025-01-31 00:00"),
        ("-4h", "2024-01-30 20:00"),
        ("+30min", "2024-01-31 00:30"),
        ("+1w2d", "2024-02-09 00:00"),
        ("-1y2m", "2022-11-30 00:00"),
    ];
    for (offset, expected) in cases {
        let source = format!("{{{{date format=\"yyyy-MM-dd HH:mm\" offset=\"{offset}\"}}}}");
        assert_eq!(base().render(&source).text, expected, "{offset}");
    }
}

#[test]
fn invalid_offsets() {
    for offset in [
        "",
        "+",
        "1d",
        "+1",
        "+1q",
        "+1d-2h",
        "+1.5d",
        "+999999999999999999999d",
    ] {
        assert_eq!(DateOffset::parse(offset), None, "{offset}");
    }
    assert_eq!(
        DateOffset::parse("-1y2m"),
        Some(DateOffset {
            parts: vec![("y".into(), -1), ("m".into(), -2)]
        })
    );
}

#[test]
fn locales_and_empty_values() {
    let mut fixture = base();
    fixture.locale = "de_DE".into();
    fixture.clipboard = None;
    assert_eq!(
        fixture
            .render(r#"{{date format="MMMM"}}/{{date format="MMMM" locale=en_US_POSIX}}"#)
            .text,
        "Januar/January"
    );
    assert_eq!(
        fixture
            .render("{{clipboard}}{{field name=missing}}{{choice name=missing}}")
            .text,
        ""
    );
    fixture.form_values.insert("x".into(), String::new());
    assert_eq!(
        fixture
            .render(r#"{{field name=x default="fallback"}}"#)
            .text,
        ""
    );
}

#[test]
fn depth_and_repeated_references() {
    let ids: Vec<Uuid> = (0..9).map(|_| Uuid::new_v4()).collect();
    let reference = |i: usize| format!("{{{{snippet id=\"{}\"}}}}", ids[i]);
    let mut snippets = HashMap::new();
    for (i, id) in ids.iter().enumerate().take(8) {
        snippets.insert(*id, format!("x{}", reference(i + 1)));
    }
    snippets.insert(ids[8], "too deep".to_string());
    let result = Fixture::with_snippets(snippets.clone()).render(&reference(0));
    assert_eq!(result.text, "x".repeat(8));
    assert_eq!(result.diagnostics.len(), 1);
    snippets.insert(ids[7], "last".into());
    let allowed = Fixture::with_snippets(snippets.clone()).render(&reference(0));
    assert_eq!(allowed.text, format!("{}last", "x".repeat(7)));
    assert!(allowed.diagnostics.is_empty());
    let repeated =
        Fixture::with_snippets(snippets).render(&format!("{}{}", reference(7), reference(7)));
    assert_eq!(repeated.text, "lastlast");
    assert!(repeated.diagnostics.is_empty());
}

#[test]
fn typed_attributes_and_canonicalization() {
    let parsed = template::parse(r#"{{ field name = x custom=42 trim=false default="a\"b\\c" }}"#);
    let Node::Tag(tag) = &parsed.nodes[0] else {
        panic!("expected tag")
    };
    assert_eq!(tag.attributes[1].value, AttributeValue::Number(42.0));
    assert_eq!(tag.attributes[2].value, AttributeValue::Bool(false));
    assert_eq!(
        template::typed(tag),
        TemplateTag::Field {
            name: "x".into(),
            default: Some("a\"b\\c".into()),
            multiline: false
        }
    );
    let reparsed = template::parse(&template::serialize(&parsed.nodes));
    let Node::Tag(again) = &reparsed.nodes[0] else {
        panic!("expected tag")
    };
    assert_eq!(
        (&tag.name, &tag.attributes),
        (&again.name, &again.attributes)
    );
}

#[test]
fn invalid_types_and_duplicate_last_wins() {
    let result = base().render("{{clipboard trim=oops}}");
    assert_eq!(result.text, "{{clipboard trim=oops}}");
    assert_eq!(result.diagnostics.len(), 1);
    let duplicate = base().render(r#"{{field name=x default="first" default="last"}}"#);
    assert_eq!(duplicate.text, "last");
    assert_eq!(duplicate.diagnostics.len(), 1);
}

#[test]
fn invalid_offset_is_ignored() {
    for value in ["3", "true", "\"nonsense\""] {
        let result = base().render(&format!("{{{{date offset={value}}}}}"));
        assert_eq!(result.text, "2024-01-31", "{value}");
        assert_eq!(result.diagnostics.len(), 1, "{value}");
    }
}

#[test]
fn noncanonical_generated_round_trips() {
    for i in 0..250 {
        let source = format!(
            "😀\\{{{{literal}}}} {{{{ field   name = x custom = {i}.5 default=\"value {i}\" }}}} {{{{clipboard trim = {}}}}}",
            i % 2 == 0
        );
        let first = template::parse(&source);
        let canonical = template::serialize(&first.nodes);
        let second = template::parse(&canonical);
        assert!(
            first.diagnostics.is_empty() && second.diagnostics.is_empty(),
            "{source}"
        );
        assert_eq!(first.nodes.len(), second.nodes.len());
        for (a, b) in first.nodes.iter().zip(&second.nodes) {
            match (a, b) {
                (Node::Text { text: x, .. }, Node::Text { text: y, .. }) => assert_eq!(x, y),
                (Node::Tag(x), Node::Tag(y)) => {
                    assert_eq!((&x.name, &x.attributes), (&y.name, &y.attributes))
                }
                _ => panic!("node kind changed"),
            }
        }
        assert_eq!(template::serialize(&second.nodes), canonical);
    }
}

#[test]
fn root_snippet_cannot_include_itself_even_once() {
    let root = Uuid::new_v4();
    let source = format!("before {{{{snippet id=\"{root}\"}}}} after");
    let mut fixture = Fixture::with_snippets([(root, source.clone())].into());
    fixture.root = Some(root);
    let with_root = fixture.render(&source);
    assert_eq!(with_root.text, "before  after");
    assert_eq!(
        with_root
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["Snippet cycle detected"]
    );
    fixture.root = None;
    assert_eq!(fixture.render(&source).text, "before before  after after");
}

#[test]
fn root_id_does_not_affect_other_references() {
    let (root, other) = (Uuid::new_v4(), Uuid::new_v4());
    let source = format!("{{{{snippet id=\"{other}\"}}}}");
    let mut fixture = Fixture::with_snippets([(other, "x".to_string())].into());
    fixture.root = Some(root);
    let result = fixture.render(&source);
    assert_eq!(result.text, "x");
    assert!(result.diagnostics.is_empty());
    let mut indirect = Fixture::with_snippets(
        [
            (other, format!("{{{{snippet id=\"{root}\"}}}}")),
            (root, "never".to_string()),
        ]
        .into(),
    );
    indirect.root = Some(root);
    let result = indirect.render(&source);
    assert_eq!(result.text, "");
    assert_eq!(result.diagnostics.len(), 1);
}

#[test]
fn nested_diagnostics_use_nested_ranges() {
    let id = Uuid::new_v4();
    let fixture = Fixture::with_snippets([(id, "ab{{DATE}}".to_string())].into());
    let result = fixture.render(&format!("0123456789{{{{snippet id=\"{id}\"}}}}"));
    assert_eq!(result.text, "0123456789ab{{DATE}}");
    assert_eq!(result.diagnostics[0].range, 2..10);
}

// Selection, UUID, random, clipboard history

#[test]
fn selection_renders_the_captured_text_or_nothing() {
    let mut fixture = Fixture::new();
    assert_eq!(fixture.render("[{{selection}}]").text, "[]");
    fixture.selection = Some("  picked\n".into());
    assert_eq!(fixture.render("[{{selection}}]").text, "[  picked\n]");
    assert_eq!(fixture.render("[{{selection trim=true}}]").text, "[picked]");
    assert_eq!(
        fixture.render("{{selection trim=1}}").text,
        "{{selection trim=1}}"
    );
}

#[test]
fn uuid_comes_from_the_context() {
    let fixture = Fixture::new();
    let first = fixture.render("{{uuid}}").text;
    assert_eq!(
        first,
        fixture.render("{{uuid}}").text,
        "same seed, same value"
    );
    assert_eq!(first.len(), 36);
    assert_eq!(first, first.to_lowercase());
    assert_eq!(
        fixture.render("{{uuid upper=true}}").text,
        first.to_uppercase()
    );
    // Each tag asks for its own value.
    let two = fixture.render("{{uuid}} {{uuid}}").text;
    let (a, b) = two.split_once(' ').unwrap();
    assert_eq!(a, first);
    assert_ne!(a, b);
    // Without a source the nil UUID keeps rendering deterministic.
    let resolve = |_| None;
    let context = template::RenderContext {
        now: now(),
        locale: "en_US_POSIX".into(),
        clipboard: None,
        form_values: HashMap::new(),
        resolve_snippet: &resolve,
        resolve_formatting: None,
        root_snippet_id: None,
        selection: None,
        clipboard_history: None,
        new_uuid: None,
        pick: None,
    };
    let bare = template::render(
        r#"{{uuid}} {{random options="a|b"}} [{{clipboard history=1}}]"#,
        &context,
    );
    assert_eq!(bare.text, "00000000-0000-0000-0000-000000000000 a []");
}

#[test]
fn random_picks_one_option_with_choice_escaping() {
    let mut fixture = Fixture::new();
    let mut seen = std::collections::HashSet::new();
    for seed in 0..64 {
        fixture.seed = seed;
        let text = fixture.render(r#"{{random options="red|a\|b|"}}"#).text;
        assert_eq!(
            text,
            fixture.render(r#"{{random options="red|a\|b|"}}"#).text
        );
        seen.insert(text);
    }
    assert_eq!(seen, ["red", "a|b", ""].map(String::from).into());
    assert_eq!(fixture.render("[{{random}}]").text, "[]");
    assert_eq!(fixture.render(r#"[{{random options=""}}]"#).text, "[]");
    assert_eq!(fixture.render("{{random options=only}}").text, "only");
    // An index the source gets wrong is wrapped rather than trusted.
    let resolve = |_| None;
    let pick = |n: usize| n + 1;
    let context = template::RenderContext {
        now: now(),
        locale: "en_US_POSIX".into(),
        clipboard: None,
        form_values: HashMap::new(),
        resolve_snippet: &resolve,
        resolve_formatting: None,
        root_snippet_id: None,
        selection: None,
        clipboard_history: None,
        new_uuid: None,
        pick: Some(&pick),
    };
    assert_eq!(
        template::render(r#"{{random options="a|b|c"}}"#, &context).text,
        "b"
    );
}

#[test]
fn clipboard_history_reads_older_items() {
    let mut fixture = Fixture::new();
    fixture.clipboard = Some("now".into());
    fixture.history = vec![" one ".into(), "two".into()];
    assert_eq!(fixture.render("{{clipboard}}").text, "now");
    assert_eq!(fixture.render("{{clipboard history=0}}").text, "now");
    assert_eq!(fixture.render("{{clipboard history=1}}").text, " one ");
    assert_eq!(
        fixture.render("{{clipboard history=1 trim=true}}").text,
        "one"
    );
    assert_eq!(fixture.render("{{clipboard history=2}}").text, "two");
    assert_eq!(fixture.render("[{{clipboard history=3}}]").text, "[]");
    // Out-of-range and fractional numbers are adjusted with a warning; other types are malformed.
    let clamped = fixture.render("{{clipboard history=-1}}");
    assert_eq!(clamped.text, "now");
    assert_eq!(clamped.diagnostics.len(), 1);
    assert_eq!(fixture.render("{{clipboard history=1.9}}").text, " one ");
    assert_eq!(
        fixture.render("{{clipboard history=\"1\"}}").text,
        "{{clipboard history=\"1\"}}"
    );
    assert_eq!(
        template::typed(
            &match &template::parse("{{clipboard history=500}}").nodes[0] {
                Node::Tag(tag) => tag.clone(),
                _ => unreachable!(),
            }
        ),
        TemplateTag::Clipboard {
            trim: false,
            history: template::MAX_CLIPBOARD_HISTORY
        }
    );
}

// Key presses and pauses

use kudzu_core::template::{ActionKind, Key, RenderAction};

fn key(offset: usize, key: Key, count: u32) -> RenderAction {
    RenderAction {
        offset,
        action: ActionKind::Key { key, count },
    }
}

fn pause(offset: usize, ms: u32) -> RenderAction {
    RenderAction {
        offset,
        action: ActionKind::Pause { ms },
    }
}

#[test]
fn keys_and_delays_become_ordered_actions_outside_the_text() {
    let result = Fixture::new().render(
        "😀a{{key name=tab}}b{{delay ms=250}}{{key name=return count=2}}{{key name=enter}}",
    );
    assert_eq!(result.text, "😀ab");
    assert_eq!(
        result.actions,
        [
            key(3, Key::Tab, 1),
            pause(4, 250),
            key(4, Key::Return, 2),
            key(4, Key::Return, 1)
        ]
    );
    assert!(result.diagnostics.is_empty());
    assert!(result.has_key_presses());
    let pause_only = Fixture::new().render("a{{delay ms=10}}b");
    assert!(!pause_only.has_key_presses());
    assert!(Fixture::new().render("plain").actions.is_empty());
}

#[test]
fn every_key_name_parses_and_round_trips() {
    for key in Key::ALL {
        let source = format!("{{{{key name=\"{}\"}}}}", key.name());
        assert_eq!(Key::parse(key.name()), Some(key));
        assert_eq!(canonical(&source), source);
        assert_eq!(
            Fixture::new().render(&source).actions,
            [crate::key(0, key, 1)]
        );
    }
    assert_eq!(Key::parse("enter"), Some(Key::Return));
    assert_eq!(Key::parse("esc"), Some(Key::Escape));
    assert_eq!(Key::parse("Tab"), None);
    // The alias is kept as written: saving never rewrites a tag's attributes.
    assert_eq!(
        canonical("{{key name=esc count=3}}"),
        r#"{{key name="esc" count=3}}"#
    );
}

#[test]
fn malformed_keys_and_delays_stay_verbatim() {
    for source in [
        "{{key}}",
        "{{key name=f5}}",
        "{{key name=tab count=two}}",
        "{{key name=tab count=\"2\"}}",
        "{{key name=tab count=true}}",
        "{{delay}}",
        "{{delay ms=soon}}",
        "{{delay ms=\"100\"}}",
    ] {
        let result = Fixture::new().render(source);
        assert_eq!(result.text, source);
        assert!(result.actions.is_empty(), "{source}");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.severity == template::Severity::Error),
            "{source}"
        );
    }
}

#[test]
fn counts_and_delays_are_clamped_with_a_warning() {
    let cases = [
        ("{{key name=tab count=0}}", key(0, Key::Tab, 1)),
        ("{{key name=tab count=-4}}", key(0, Key::Tab, 1)),
        ("{{key name=tab count=2.7}}", key(0, Key::Tab, 2)),
        (
            "{{key name=tab count=999}}",
            key(0, Key::Tab, template::MAX_KEY_COUNT),
        ),
        ("{{delay ms=0}}", pause(0, 1)),
        ("{{delay ms=60000}}", pause(0, template::MAX_DELAY_MS)),
    ];
    for (source, expected) in cases {
        let result = Fixture::new().render(source);
        assert_eq!(result.actions, [expected], "{source}");
        assert_eq!(result.diagnostics.len(), 1, "{source}");
        assert_eq!(result.diagnostics[0].severity, template::Severity::Warning);
    }
    assert!(Fixture::new()
        .render("{{key name=tab count=50}}{{delay ms=5000}}")
        .diagnostics
        .is_empty());
}

#[test]
fn total_pause_is_capped() {
    let result = Fixture::new()
        .render("{{delay ms=5000}}a{{delay ms=4000}}b{{delay ms=4000}}c{{delay ms=10}}d");
    assert_eq!(result.text, "abcd");
    assert_eq!(
        result.actions,
        [pause(0, 5000), pause(1, 4000), pause(2, 1000)]
    );
    assert_eq!(result.diagnostics.len(), 2);
}

#[test]
fn cursor_counts_only_after_the_last_key_press() {
    let after = Fixture::new().render("a{{key name=tab}}b{{cursor}}c");
    assert_eq!(after.cursor_offset, Some(2));
    assert!(after.diagnostics.is_empty());

    let before = Fixture::new().render("a{{cursor}}b{{key name=tab}}c{{key name=tab}}");
    assert_eq!(before.cursor_offset, None);
    assert_eq!(
        before.diagnostics.len(),
        1,
        "one warning, on the first key after the cursor"
    );
    assert_eq!(before.diagnostics[0].range, 12..28);

    // The first cursor is still the only one that counts; a later one doesn't take over.
    let second = Fixture::new().render("{{cursor}}{{key name=tab}}b{{cursor}}");
    assert_eq!(second.cursor_offset, None);

    // A pause doesn't move focus, so the cursor survives it.
    assert_eq!(
        Fixture::new()
            .render("a{{cursor}}b{{delay ms=5}}c")
            .cursor_offset,
        Some(1)
    );
}

#[test]
fn nested_snippets_contribute_actions_in_place() {
    let id = Uuid::new_v4();
    let fixture = Fixture::with_snippets([(id, "n{{key name=tab}}{{cursor}}m".to_string())].into());
    let result = fixture.render(&format!(
        "a{{{{snippet id=\"{id}\"}}}}b{{{{key name=return}}}}"
    ));
    assert_eq!(result.text, "anmb");
    assert_eq!(
        result.actions,
        [key(2, Key::Tab, 1), key(4, Key::Return, 1)]
    );
    assert_eq!(result.cursor_offset, None);
    assert!(result.diagnostics.is_empty());

    // A key inside a nested snippet cancels a root cursor before it, and says so.
    let result = fixture.render(&format!("{{{{cursor}}}}a{{{{snippet id=\"{id}\"}}}}"));
    assert_eq!(result.cursor_offset, None);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].range,
        1..17,
        "the range is in the nested source"
    );
}

#[test]
fn analysis_reports_selection_and_key_presses() {
    let info = template::analyze(&template::parse("{{selection}}{{key name=tab}}"));
    assert!(info.uses_selection && info.has_key_presses && !info.uses_clipboard);
    let info = template::analyze(&template::parse(
        "{{clipboard history=2}}{{delay ms=5}}{{key name=f5}}",
    ));
    assert!(info.uses_clipboard && !info.uses_selection && !info.has_key_presses);
}

#[test]
fn actions_serialize_flat_and_are_omitted_when_empty() {
    let json =
        serde_json::to_value(Fixture::new().render("a{{key name=tab count=2}}{{delay ms=5}}"))
            .unwrap();
    assert_eq!(
        json["actions"],
        serde_json::json!([
            { "offset": 1, "kind": "key", "key": "tab", "count": 2 },
            { "offset": 1, "kind": "pause", "ms": 5 },
        ])
    );
    let plain = serde_json::to_value(Fixture::new().render("a")).unwrap();
    assert!(plain.get("actions").is_none());
    // A result written by an older build still decodes.
    let old: template::RenderResult =
        serde_json::from_str(r#"{"text":"a","cursorOffset":null,"diagnostics":[]}"#).unwrap();
    assert!(old.actions.is_empty());
}
