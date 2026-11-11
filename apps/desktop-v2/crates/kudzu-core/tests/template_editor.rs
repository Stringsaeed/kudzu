//! The template editor document: load, edit, clipboard, and the recorded fixtures.
//!
//! Public offsets are UTF-8 display bytes. Saved spans are template UTF-16. A painted mark may
//! cover a whole chip when the stored span covers only part of it.

mod template_support;

use std::fs;
use std::path::PathBuf;

use kudzu_core::formatting::{safe_link, FormatSpan, FormatStyle, Formatting};
use kudzu_core::template::chips::{self, PillKind, Tone};
use kudzu_core::template::doc::{EditorDoc, FormatChange};
use kudzu_core::template::images::{self, ImageError};
use kudzu_core::template::presets::date_presets;
use kudzu_core::template::summary::{plain_summary, LIST_UNITS, SEARCH_UNITS};
use kudzu_core::template::{self, Node, Tag};
use serde::Deserialize;
use template_support::{random_tag, random_text, render, semantic, text, SplitMix64};

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6x8AAAAASUVORK5CYII=";
const SIGNATURE: &str = "c5af65ac-e426-4046-802b-11ccdd3a62ba";

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn names(id: &str) -> Option<&str> {
    if id == SIGNATURE {
        Some("Email signature")
    } else {
        None
    }
}

fn spans_of(doc: &EditorDoc) -> Vec<FormatSpan> {
    doc.to_template()
        .1
        .map(|formatting| formatting.spans)
        .unwrap_or_default()
}

fn bold_span(start: usize, end: usize) -> FormatSpan {
    FormatSpan {
        start,
        end,
        style: FormatStyle::Bold,
    }
}

#[derive(Deserialize)]
struct TemplateFixture {
    presets: Vec<PresetFixture>,
    cases: Vec<CaseFixture>,
}

#[derive(Deserialize)]
struct PresetFixture {
    format: String,
    label: String,
}

#[derive(Deserialize)]
struct CaseFixture {
    source: String,
    chips: Vec<ChipFixture>,
    summary: String,
}

#[derive(Deserialize)]
struct ChipFixture {
    label: String,
    tone: String,
}

#[derive(Deserialize)]
struct FormattingFixture {
    transactions: Vec<serde_json::Value>,
    safety: Vec<SafetyFixture>,
    utf16: Vec<FormatSpan>,
}

#[derive(Deserialize)]
struct SafetyFixture {
    input: String,
    result: bool,
}

#[test]
fn presets_and_chip_fixtures_match() {
    let fixture: TemplateFixture = serde_json::from_str(&fixture("template.json")).unwrap();
    assert_eq!(date_presets().len(), 11);
    assert_eq!(fixture.presets.len(), 11);
    for (got, want) in date_presets().iter().zip(&fixture.presets) {
        assert_eq!(got.format, want.format);
        assert_eq!(got.label, want.label);
    }

    assert_eq!(fixture.cases.len(), 17);
    for case in &fixture.cases {
        let mut doc = EditorDoc::from_template_named(&case.source, None, &names);
        assert_eq!(doc.to_template().0, case.source, "bytes {}", case.source);
        assert_eq!(
            plain_summary(&case.source, Some(&names), LIST_UNITS),
            case.summary,
            "summary {}",
            case.source
        );
        assert_eq!(doc.pills().len(), case.chips.len(), "{}", case.source);
        for (pill, chip) in doc.pills().iter().zip(&case.chips) {
            assert_eq!(pill.label, chip.label, "{}", case.source);
            assert_eq!(pill.tone.name(), chip.tone, "{}", case.source);
        }
        assert!(!doc.can_undo(), "loading is not an edit");
        assert!(!doc.undo());
        assert_eq!(doc.to_template().0, case.source);
    }
}

#[test]
fn verbatim_tags_keep_their_bytes_and_unknown_without_verbatim_is_a_form_chip() {
    for source in [r#"{{unknown a="x"}}"#, "{{field}}"] {
        let doc = EditorDoc::from_template(source, None);
        assert_eq!(doc.pills().len(), 1, "{source}");
        assert_eq!(doc.pills()[0].label, source);
        assert_eq!(doc.pills()[0].tone, Tone::Error);
        assert_eq!(doc.pills()[0].source, source);
        assert_eq!(doc.to_template().0, source);
    }
    let tag = Tag {
        name: "madeup".into(),
        attributes: vec![],
        range: 0..0,
        verbatim: None,
    };
    assert_eq!(chips::chip_tone(&tag), Tone::Form);
    assert_eq!(
        chips::chip_label(&tag, "{{madeup}}", &|_| None),
        "{{madeup}}"
    );
}

#[test]
fn named_snippet_and_missing_snippet() {
    let source = format!(r#"{{{{snippet id="{SIGNATURE}"}}}}"#);
    let missing = EditorDoc::from_template(&source, None);
    assert_eq!(missing.pills()[0].label, "↳ Missing snippet");
    let named = EditorDoc::from_template_named(&source, None, &names);
    assert_eq!(named.pills()[0].label, "↳ Email signature");
    assert_eq!(named.pills()[0].tone, Tone::Snippet);
}

#[test]
fn summaries_match_recorded_calls_and_the_search_map() {
    let examples = [
        (
            "Best regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}",
            "Best regards, Saeed ‹Date · Long date›",
        ),
        ("{{clipboard history=1}}", "‹Previous copy›"),
        (
            "{{clipboard trim=true history=1}}",
            "‹Previous copy (trimmed)›",
        ),
        ("{{key name=\"tab\"}}", "‹⇥ Tab›"),
        ("{{key name=\"tab\" count=2}}", "‹⇥ Tab ×2›"),
        ("{{key name=\"return\"}}", "‹↵ Return›"),
        ("{{uuid}}", "‹UUID›"),
        (
            "{{date format=\"EEEE, MMMM d\" offset=\"+1w\"}}",
            "‹Date · Weekday + date +1w›",
        ),
        ("{{delay ms=1500}}", "‹Pause 1.5 s›"),
        ("{{delay ms=1000}}", "‹Pause 1 s›"),
    ];
    for (source, summary) in examples {
        assert_eq!(plain_summary(source, None, LIST_UNITS), summary, "{source}");
    }
    let xs = "x".repeat(150);
    assert_eq!(plain_summary(&xs, None, LIST_UNITS), "x".repeat(140));

    let recorded: std::collections::HashMap<String, String> =
        serde_json::from_str(&fixture("plain-summaries.json")).unwrap();
    assert_eq!(recorded.len(), 18);
    for (source, summary) in &recorded {
        assert_eq!(
            plain_summary(source, None, SEARCH_UNITS),
            *summary,
            "{source}"
        );
    }
}

#[test]
fn every_key_has_its_label() {
    let want = [
        ("tab", "⇥ Tab"),
        ("return", "↵ Return"),
        ("escape", "Escape"),
        ("space", "Space"),
        ("backspace", "⌫ Backspace"),
        ("delete", "⌦ Forward delete"),
        ("left", "← Left"),
        ("right", "→ Right"),
        ("up", "↑ Up"),
        ("down", "↓ Down"),
    ];
    assert_eq!(want.len(), template::Key::ALL.len());
    for (name, label) in want {
        let source = format!("{{{{key name=\"{name}\"}}}}");
        let doc = EditorDoc::from_template(&source, None);
        assert_eq!(doc.pills()[0].label, label, "{name}");
        assert_eq!(doc.pills()[0].tone, Tone::Key);
    }
}

#[test]
fn formatting_fixture_replays_including_the_last_undo() {
    let fixture: FormattingFixture =
        serde_json::from_str(&fixture("editor-formatting.json")).unwrap();
    assert_eq!(fixture.transactions.len(), 30);
    for entry in &fixture.safety {
        assert_eq!(safe_link(&entry.input), entry.result, "{}", entry.input);
    }
    assert_eq!(
        fixture.utf16,
        vec![FormatSpan {
            start: 0,
            end: 2,
            style: FormatStyle::Italic
        }]
    );

    let mut doc = EditorDoc::from_template("", None);
    for action in &fixture.transactions {
        let name = action["action"].as_str().unwrap();
        match name {
            "enable" => doc = EditorDoc::from_template(action["text"].as_str().unwrap(), None),
            "bold" => {
                doc.change_format(0, doc.text().len(), FormatChange::Bold);
            }
            "italic" => {
                doc.change_format(0, doc.text().len(), FormatChange::Italic);
            }
            "underline" => {
                doc.change_format(0, doc.text().len(), FormatChange::Underline);
            }
            "link" => {
                doc.change_format(
                    0,
                    doc.text().len(),
                    FormatChange::Link("https://example.com/first".into()),
                );
            }
            "edit-link" => {
                doc.change_format(
                    0,
                    doc.text().len(),
                    FormatChange::Link("https://example.com/edited".into()),
                );
            }
            "clear" => {
                doc.change_format(0, doc.text().len(), FormatChange::Clear);
            }
            "insert-image" => {
                doc.insert_image(0, 0, PNG, 32, "Dot").unwrap();
            }
            "remove-image-formatting" => {
                let range = doc
                    .pills()
                    .iter()
                    .find(|pill| pill.kind == PillKind::Image)
                    .unwrap()
                    .range
                    .clone();
                doc.change_format(range.start, range.end, FormatChange::Clear);
            }
            "insert" => {
                doc.replace(0, 0, "Hi ");
            }
            "bullets" | "numbers" | "remove-numbers" => {
                let start = doc.text().find("[Dot]").unwrap();
                doc.toggle_list(start, start + "[Dot]".len(), name != "bullets");
            }
            "undo" => assert!(doc.undo(), "undo"),
            "redo" => assert!(doc.redo(), "redo"),
            other => panic!("unknown action {other}"),
        }
        let (source, formatting) = doc.to_template();
        assert_eq!(source, action["doc"].as_str().unwrap(), "{name}");
        let want: Vec<FormatSpan> = serde_json::from_value(action["spans"].clone()).unwrap();
        assert_eq!(
            formatting.map(|item| item.spans).unwrap_or_default(),
            want,
            "{name}"
        );
    }
}

#[test]
fn list_markers_toggle_every_selected_line_and_undo_as_one_step() {
    let mut doc = EditorDoc::from_template("first\nsecond\nthird", None);
    assert!(doc.toggle_list(0, doc.text().len(), false));
    assert_eq!(doc.text(), "• first\n• second\n• third");
    assert!(doc.toggle_list(0, doc.text().len(), true));
    assert_eq!(doc.text(), "1. first\n2. second\n3. third");
    assert!(doc.toggle_list(0, doc.text().len(), true));
    assert_eq!(doc.text(), "first\nsecond\nthird");
    assert!(doc.undo());
    assert_eq!(doc.text(), "1. first\n2. second\n3. third");
    assert!(doc.redo());
    assert_eq!(doc.to_template().0, "first\nsecond\nthird");

    let mut empty = EditorDoc::from_template("", None);
    assert!(empty.toggle_list(0, 0, false));
    assert_eq!(empty.text(), "• ");
}

#[test]
fn loading_is_not_undoable_and_a_mismatched_formatting_source_is_ignored() {
    let mut doc = EditorDoc::from_template("{{date}}", None);
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert!(!doc.undo());
    assert!(!doc.redo());
    assert_eq!(doc.to_template().0, "{{date}}");

    let formatting = Formatting {
        source: "other".into(),
        spans: vec![bold_span(0, 5)],
    };
    let doc = EditorDoc::from_template("Hello", Some(&formatting));
    assert!(doc.marks().is_empty());
    assert!(doc.to_template().1.is_none());
}

#[test]
fn pill_identity_stays_put_and_html_ids_disambiguate_duplicates() {
    let mut doc = EditorDoc::from_template("{{date}} and {{date}}", None);
    let first = doc.pills()[0].id;
    let second = doc.pills()[1].id;
    assert_ne!(first, second);
    assert_eq!(doc.html_id(first).as_deref(), Some("time:{{date}}"));
    assert_eq!(doc.html_id(second).as_deref(), Some("time:{{date}}#2"));
    assert_eq!(
        chips::parse_html_pill_id("time:{{date}}#2"),
        Some((Tone::Time, "{{date}}".into()))
    );
    assert_eq!(
        chips::parse_html_pill_id("time:{{date}}"),
        Some((Tone::Time, "{{date}}".into()))
    );

    let between = doc.pills()[0].range.end..doc.pills()[1].range.start;
    doc.replace(between.start, between.end, " & ");
    assert_eq!(doc.pills()[0].id, first);
    assert_eq!(doc.pills()[1].id, second);
    assert_eq!(doc.html_id(second).as_deref(), Some("time:{{date}}#2"));

    let only_second = doc.pills()[1].range.clone();
    let copied = doc.copy(only_second.start, only_second.end).unwrap();
    assert_eq!(copied.plain, "{{date}}");
    assert_eq!(
        copied.runs[0].chip.as_ref().unwrap().html_id,
        "time:{{date}}#2"
    );

    let mut dest = EditorDoc::from_template("", None);
    assert!(dest.paste_own(0, 0, &copied, &|_| None));
    assert_eq!(dest.pills().len(), 1);
    assert_ne!(dest.pills()[0].id, second);
    assert_eq!(
        dest.html_id(dest.pills()[0].id).as_deref(),
        Some("time:{{date}}")
    );
    assert!(dest.undo());
    assert_eq!(dest.text(), "");
    assert!(!dest.can_undo());

    let image = {
        let mut host = EditorDoc::from_template("ab", None);
        host.insert_image(1, 1, PNG, 32, "Dot").unwrap();
        host.pills()
            .iter()
            .find(|pill| pill.kind == PillKind::Image)
            .unwrap()
            .id
    };
    let mut host = EditorDoc::from_template("ab", None);
    let id = host.insert_image(1, 1, PNG, 32, "Dot").unwrap();
    assert_eq!(id, image);
    assert!(host.html_id(id).is_none());
}

#[test]
fn deleted_pill_ids_are_not_reused_and_redo_restores_them() {
    let mut doc = EditorDoc::from_template("Hi", None);
    let id = doc.insert_chip(2, "{{date}}", &|_| None).unwrap();
    assert!(doc.undo());
    assert!(doc.pills().is_empty());
    assert!(doc.redo());
    assert_eq!(doc.pills()[0].id, id);
    let again = doc.insert_chip(0, "{{cursor}}", &|_| None).unwrap();
    assert_ne!(again, id);
    assert_eq!(
        doc.pills()
            .iter()
            .find(|pill| pill.source == "{{date}}")
            .unwrap()
            .id,
        id
    );
}

#[test]
fn copy_writes_template_source_and_paste_rebuilds_chips_in_one_step() {
    let mut doc = EditorDoc::from_template("Hello {{date}} there", None);
    doc.change_format(0, doc.text().len(), FormatChange::Bold);
    let payload = doc.copy(0, doc.text().len()).unwrap();
    assert_eq!(payload.plain, "Hello {{date}} there");
    assert!(payload.plain.contains("{{date}}"));
    assert!(!payload.plain.contains("Date · ISO date"));
    assert!(payload.runs.iter().any(|run| run.chip.is_some()));
    assert!(!payload.marks.is_empty());

    let mut dest = EditorDoc::from_template("{{cursor}}", None);
    let kept = dest.pills()[0].id;
    let end = dest.text().len();
    assert!(dest.paste_own(end, end, &payload, &|_| None));
    assert_eq!(dest.to_template().0, "{{cursor}}Hello {{date}} there");
    let pasted = dest
        .pills()
        .iter()
        .find(|pill| pill.source == "{{date}}")
        .unwrap();
    assert_ne!(pasted.id, kept);
    assert_ne!(pasted.id, doc.pills()[0].id);
    assert!(spans_of(&dest)
        .iter()
        .any(|span| span.style == FormatStyle::Bold));
    assert!(dest.undo());
    assert_eq!(dest.to_template().0, "{{cursor}}");
    assert_eq!(dest.pills().len(), 1);
    assert!(!dest.can_undo());

    let mut forged = payload.clone();
    forged
        .runs
        .iter_mut()
        .find(|run| run.chip.is_some())
        .unwrap()
        .chip
        .as_mut()
        .unwrap()
        .html_id = "time:{{nope}}".into();
    assert!(!dest.paste_own(1, 1, &forged, &|_| None));
    assert_eq!(dest.to_template().0, "{{cursor}}");
}

#[test]
fn escaped_literals_copy_as_source_and_paste_as_display() {
    let source = r"a \{{ b \\ c";
    let doc = EditorDoc::from_template(source, None);
    assert_eq!(doc.text(), "a {{ b \\ c");
    let payload = doc.copy(0, doc.text().len()).unwrap();
    assert_eq!(payload.plain, source);
    assert!(payload.runs.iter().all(|run| run.chip.is_none()));

    let mut dest = EditorDoc::from_template("", None);
    assert!(dest.paste_own(0, 0, &payload, &|_| None));
    assert_eq!(dest.text(), "a {{ b \\ c");
    assert_eq!(dest.to_template().0, source);

    let mut other = EditorDoc::from_template("", None);
    assert!(other.paste_source(0, 0, source, &|_| None));
    assert_eq!(other.text(), "a {{ b \\ c");
    assert!(other.pills().is_empty());
}

#[test]
fn plain_text_from_another_app_becomes_chips_and_labels_stay_literal() {
    let mut doc = EditorDoc::from_template("Say ", None);
    assert!(doc.paste_source(
        doc.text().len(),
        doc.text().len(),
        "Hello {{date}}",
        &|_| None
    ));
    assert_eq!(doc.to_template().0, "Say Hello {{date}}");
    assert_eq!(doc.pills().len(), 1);
    assert_eq!(doc.pills()[0].label, "Date · ISO date");
    assert!(doc.undo());
    assert_eq!(doc.text(), "Say ");

    let mut labels = EditorDoc::from_template("", None);
    labels.paste_source(0, 0, "Hello Date · ISO date", &|_| None);
    assert!(labels.pills().is_empty());
    assert_eq!(labels.text(), "Hello Date · ISO date");
}

#[test]
fn typing_deleting_and_images_update_the_map_and_undo() {
    let mut doc = EditorDoc::from_template("ab{{date}}cd", None);
    let id = doc.pills()[0].id;
    let label = doc.pills()[0].label.clone();
    let beside = doc.text().find('b').unwrap();
    doc.replace(beside, beside + 1, "");
    assert_eq!(doc.pills().len(), 1);
    assert_eq!(doc.pills()[0].id, id);
    assert_eq!(doc.text(), format!("a{label}cd"));

    let pill = doc.pills()[0].range.clone();
    doc.replace(0, pill.start + 1, "");
    assert!(doc.pills().is_empty());
    assert_eq!(doc.text(), "cd");
    assert!(doc.undo());
    assert_eq!(doc.pills()[0].id, id);

    let mut fresh = EditorDoc::from_template("Hi", None);
    let cursor = fresh.insert_chip(2, "{{cursor}}", &|_| None).unwrap();
    assert_eq!(fresh.to_template().0, "Hi{{cursor}}");
    assert_eq!(fresh.pills()[0].id, cursor);
    let mid = fresh.pills()[0].range.start + fresh.pills()[0].range.len() / 2;
    fresh.replace(mid, mid, "X");
    assert_eq!(fresh.pills().len(), 1);
    assert!(!fresh.pills()[0].label.contains('X'));
    assert!(fresh.text().starts_with("HiX") || fresh.text().ends_with('X'));

    let mut braces = EditorDoc::from_template("{{{date}}{", None);
    assert_eq!(braces.pills().len(), 1, "display {}", braces.text());
    let range = braces.pills()[0].range.clone();
    braces.replace(range.start, range.end, "");
    assert_eq!(braces.text(), "{{");
    assert_eq!(braces.to_template().0, r"\{{");

    let mut image = EditorDoc::from_template("Hello", None);
    let id = image.insert_image(0, 0, PNG, 32, "Dot").unwrap();
    assert_eq!(image.images()[0].width, 32);
    assert_eq!(image.text().get(..5), Some("[Dot]"));
    image.set_image_width(id, 0).unwrap();
    assert_eq!(image.images()[0].width, images::MIN_IMAGE_WIDTH);
    image.set_image_width(id, 900).unwrap();
    assert_eq!(image.images()[0].width, images::MAX_IMAGE_WIDTH);
    image.set_image_alt(id, "a{b}\n").unwrap();
    assert_eq!(image.pills()[0].label, "a b");
    image.set_image_alt(id, "   ").unwrap();
    assert_eq!(image.pills()[0].label, "Image");
    assert!(image.undo());
    assert_eq!(image.pills()[0].label, "a b");

    assert_eq!(
        image.insert_image(0, 0, "https://example.com/a.png", 32, "A"),
        Err(ImageError::UnsafeData)
    );
    let tag = EditorDoc::from_template("{{date}}", None);
    assert_eq!(
        {
            let mut doc = tag;
            doc.set_image_width(doc.pills()[0].id, 32)
        },
        Err(ImageError::Missing)
    );
}

#[test]
fn image_budget_width_on_load_and_replacement() {
    let formatting = Formatting {
        source: "Hello".into(),
        spans: vec![FormatSpan {
            start: 0,
            end: 5,
            style: FormatStyle::Image {
                data_url: PNG.into(),
                width: 1,
            },
        }],
    };
    let doc = EditorDoc::from_template("Hello", Some(&formatting));
    assert_eq!(doc.images()[0].width, 1);
    assert!(doc.images()[0].pill.is_some());
    assert_eq!(doc.to_template().0, "Hello");

    let mut doc = EditorDoc::from_template("ab", None);
    let first = doc.insert_image(1, 1, PNG, 1, "A").unwrap();
    assert_eq!(doc.images()[0].width, 16);
    let end = doc.text().len();
    let second = doc.insert_image(0, end, PNG, 40, "B").unwrap();
    assert_ne!(first, second);
    assert_eq!(doc.images().len(), 1);
    assert_eq!(doc.images()[0].width, 40);
    assert_eq!(doc.images()[0].data_url, PNG);

    let wide = long_png();
    assert!(images::accept_data_url(&wide));
    assert!(images::MAX_DATA_URL_LEN - wide.len() < 4);
    assert!(!images::accept_data_url(&format!("{wide}AAAA")));
    let mut doc = EditorDoc::from_template("", None);
    doc.insert_image(0, 0, &wide, 32, "A").unwrap();
    doc.insert_image(doc.text().len(), doc.text().len(), &wide, 32, "B")
        .unwrap();
    assert_eq!(
        doc.insert_image(doc.text().len(), doc.text().len(), &wide, 32, "C"),
        Err(ImageError::TooMany)
    );
}

/// The longest PNG data URL [`images::accept_data_url`] still accepts.
fn long_png() -> String {
    let header = "data:image/png;base64,";
    let mut body = images::MAX_DATA_URL_LEN - header.len();
    body -= body % 4;
    let mut data = String::from("iVBORw0KGgo");
    data.extend(std::iter::repeat_n('A', body - data.len()));
    format!("{header}{data}")
}

#[test]
fn untouched_spans_stay_exact_across_load_and_a_shift() {
    let source = "xx{{date}}yy";
    let formatting = Formatting {
        source: source.into(),
        spans: vec![bold_span(4, 6)],
    };
    let mut doc = EditorDoc::from_template(source, Some(&formatting));
    assert_eq!(doc.marks()[0].source, 4..6);
    assert_eq!(spans_of(&doc), vec![bold_span(4, 6)]);
    assert_eq!(doc.marks()[0].range, doc.pills()[0].range);
    doc.replace(0, 0, "Z");
    assert_eq!(spans_of(&doc), vec![bold_span(5, 7)]);
    assert_eq!(doc.to_template().0, "Zxx{{date}}yy");

    let source = "😀 abc";
    let formatting = Formatting {
        source: source.into(),
        spans: vec![
            bold_span(1, 3),
            FormatSpan {
                start: 0,
                end: 2,
                style: FormatStyle::Italic,
            },
        ],
    };
    let doc = EditorDoc::from_template(source, Some(&formatting));
    assert_eq!(
        spans_of(&doc),
        vec![FormatSpan {
            start: 0,
            end: 2,
            style: FormatStyle::Italic
        }]
    );

    let source = "e\u{0301}x";
    let formatting = Formatting {
        source: source.into(),
        spans: vec![bold_span(0, 1)],
    };
    let doc = EditorDoc::from_template(source, Some(&formatting));
    assert_eq!(doc.source_to_display(1, false), 0);
    assert_eq!(doc.source_to_display(1, true), "e\u{0301}".len());
    assert_eq!(spans_of(&doc), vec![bold_span(0, 1)]);

    let source = r"a\{{b";
    let doc = EditorDoc::from_template(source, None);
    assert_eq!(doc.text(), "a{{b");
    assert_eq!(doc.display_to_source(2, false), 1);
    assert_eq!(doc.display_to_source(2, true), 4);
    let formatting = Formatting {
        source: source.into(),
        spans: vec![bold_span(0, 2)],
    };
    let doc = EditorDoc::from_template(source, Some(&formatting));
    assert_eq!(spans_of(&doc), vec![bold_span(0, 2)]);
    assert_eq!(doc.to_template().0, source);
}

#[test]
fn an_image_beside_a_tag_is_a_pill_and_an_overlapping_one_is_not_merged() {
    let source = "Hello {{date}}";
    let beside = Formatting {
        source: source.into(),
        spans: vec![FormatSpan {
            start: 0,
            end: 6,
            style: FormatStyle::Image {
                data_url: PNG.into(),
                width: 32,
            },
        }],
    };
    let doc = EditorDoc::from_template(source, Some(&beside));
    assert_eq!(doc.to_template().0, source);
    assert!(doc.images()[0].pill.is_some());
    assert!(doc
        .pills()
        .iter()
        .any(|pill| pill.kind == PillKind::Tag && pill.source == "{{date}}"));
    assert_eq!(doc.images()[0].source, 0..6);

    let overlap = Formatting {
        source: source.into(),
        spans: vec![FormatSpan {
            start: 4,
            end: 10,
            style: FormatStyle::Image {
                data_url: PNG.into(),
                width: 32,
            },
        }],
    };
    let doc = EditorDoc::from_template(source, Some(&overlap));
    assert_eq!(doc.to_template().0, source);
    assert!(doc.images()[0].pill.is_none());
    assert_eq!(doc.images()[0].source, 4..10);
    assert_eq!(doc.pills().len(), 1);
    assert_eq!(doc.pills()[0].source, "{{date}}");
    let (saved, formatting) = doc.to_template();
    let formatting = formatting.unwrap();
    assert_eq!(saved, source);
    assert_eq!(formatting.spans[0].start, 4);
    assert_eq!(formatting.spans[0].end, 10);
}

#[test]
fn typing_at_the_end_of_bold_extends_it_and_typing_beside_an_image_does_not() {
    let mut doc = EditorDoc::from_template("Hello", None);
    doc.change_format(0, doc.text().len(), FormatChange::Bold);
    doc.replace(doc.text().len(), doc.text().len(), "!");
    assert_eq!(spans_of(&doc), vec![bold_span(0, 6)]);
    doc.replace(0, 0, "X");
    assert_eq!(spans_of(&doc), vec![bold_span(0, 7)]);

    let mut doc = EditorDoc::from_template("ab", None);
    doc.insert_image(1, 1, PNG, 32, "Dot").unwrap();
    let image = spans_of(&doc)[0].clone();
    let at = doc.images()[0].source.end;
    let display = doc.source_to_display(at, false);
    doc.replace(display, display, "Z");
    let after = spans_of(&doc);
    assert_eq!(after[0].start, image.start);
    assert_eq!(after[0].end, image.end);
    assert!(doc.text().contains('Z'));

    doc.undo();
    let before = doc.source_to_display(image.start, false);
    doc.replace(before, before, "Q");
    let shifted = spans_of(&doc);
    assert_eq!(shifted[0].end - shifted[0].start, image.end - image.start);
    assert_ne!(shifted[0].start, 0);
}

#[test]
fn every_source_and_every_canonical_source_round_trips() {
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
        assert_round_trip(&source);
    }

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
        "{{key name=tab count=3}}",
        "{{delay ms=20}}",
        "😀",
        "|",
        "{{unknown a=\"b\"}}",
    ];
    for _ in 0..6000 {
        let source: String = (0..rng.below(10)).map(|_| *rng.pick(PIECES)).collect();
        assert_round_trip(&source);
        let canonical = template::serialize(&template::parse(&source).nodes);
        assert_round_trip(&canonical);
    }
}

fn assert_round_trip(source: &str) {
    let doc = EditorDoc::from_template(source, None);
    let (saved, _) = doc.to_template();
    assert_eq!(saved, source, "bytes");
    assert_eq!(
        semantic(&template::parse(&saved).nodes),
        semantic(&template::parse(source).nodes)
    );
    assert_eq!(render(&saved), render(source));
}

#[test]
fn edits_leave_a_document_that_reloads_and_undo_restores_it() {
    let mut rng = SplitMix64(99);
    for _ in 0..120 {
        let mut nodes = Vec::new();
        for _ in 0..rng.below(5) {
            nodes.push(if rng.bool() {
                text(&random_text(&mut rng))
            } else {
                Node::Tag(random_tag(&mut rng))
            });
        }
        let source = template::serialize(&nodes);
        let mut doc = EditorDoc::from_template(&source, None);
        assert!(!doc.can_undo());
        let original: Vec<u64> = doc.pills().iter().map(|pill| pill.id.get()).collect();
        for _ in 0..rng.below(4) + 1 {
            match rng.below(5) {
                0 => {
                    let at = rng.below(doc.text().len() + 1);
                    doc.replace(at, at, "Z");
                }
                1 => {
                    let start = rng.below(doc.text().len() + 1);
                    let end = rng.below(doc.text().len() + 1);
                    doc.replace(start.min(end), start.max(end), "");
                }
                2 => {
                    let at = rng.below(doc.text().len() + 1);
                    doc.insert_chip(at, "{{date}}", &|_| None);
                }
                3 => {
                    let at = rng.below(doc.text().len() + 1);
                    let _ = doc.insert_image(at, at, PNG, 32, "Dot");
                }
                _ => {
                    if !doc.text().is_empty() {
                        doc.change_format(0, doc.text().len(), FormatChange::Bold);
                    }
                }
            }
            let (saved, formatting) = doc.to_template();
            let again = EditorDoc::from_template(&saved, formatting.as_ref());
            assert_eq!(again.to_template().0, saved, "reload {saved}");
        }
        while doc.can_undo() {
            assert!(doc.undo());
        }
        assert_eq!(doc.to_template().0, source, "undo {source}");
        let restored: Vec<u64> = doc.pills().iter().map(|pill| pill.id.get()).collect();
        assert_eq!(restored, original);
    }
}
