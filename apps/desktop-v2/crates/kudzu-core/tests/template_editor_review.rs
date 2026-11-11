//! Regressions for the S23 review. Display offsets are UTF-8 bytes.
use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};
use kudzu_core::template::doc::{EditorDoc, FormatChange};
use kudzu_core::template::{chips::Tone, images};

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6x8AAAAASUVORK5CYII=";
fn styled(source: &str, start: usize, end: usize) -> EditorDoc {
    EditorDoc::from_template(
        source,
        Some(&Formatting {
            source: source.into(),
            spans: vec![FormatSpan {
                start,
                end,
                style: FormatStyle::Bold,
            }],
        }),
    )
}
#[test]
fn deleting_the_first_repeated_character_keeps_the_second_bold() {
    let mut doc = styled("xx", 1, 2);
    doc.replace(0, 1, "");
    assert_eq!(doc.text(), "x");
    assert_eq!(doc.marks()[0].range, 0..1);
}
#[test]
fn deleting_a_repeated_word_maps_the_actual_selection() {
    let mut doc = styled("abc·abc", 4, 7);
    doc.replace(0, 3, "");
    assert_eq!(doc.text(), "·abc");
    assert_eq!(doc.marks()[0].range, 2..5);
}
#[test]
fn repair_keeps_bold_on_the_edited_word() {
    let mut doc = styled("a {{ b c", 7, 8);
    doc.replace(8, 8, "!");
    assert_eq!(doc.text(), "a {{ b c!");
    assert_eq!(doc.marks()[0].range, 7..9);
}
#[test]
fn pasting_repeated_text_stamps_marks_at_the_insertion() {
    let payload = styled("x", 0, 1).copy(0, 1).unwrap();
    let mut doc = EditorDoc::from_template("xx", None);
    doc.paste_own(0, 0, &payload, &|_| None);
    assert_eq!(doc.text(), "xxx");
    assert_eq!(doc.marks()[0].range, 0..1);
}
#[test]
fn escaped_braces_allow_an_insert_between_them() {
    let mut doc = EditorDoc::from_template(r"\{{", None);
    doc.replace(1, 1, "x");
    assert_eq!(doc.text(), "{x{");
    assert_eq!(doc.to_template().0, "{x{");
}
#[test]
fn escaped_braces_allow_one_brace_to_be_deleted() {
    let mut doc = EditorDoc::from_template(r"\{{", None);
    doc.replace(1, 2, "");
    assert_eq!(doc.text(), "{");
    assert_eq!(doc.to_template().0, "{");
}
#[test]
fn scalar_boundaries_inside_combining_zwj_and_crlf_are_editable() {
    for (text, at) in [("e\u{301}", 1), ("👩\u{200d}💻", 4), ("\r\n", 1)] {
        let mut doc = EditorDoc::from_template(text, None);
        doc.replace(at, at, "x");
        let mut expected = text.to_string();
        expected.insert(at, 'x');
        assert_eq!(doc.text(), expected);
    }
}
#[test]
fn empty_formatting_source_does_not_match_nonempty_content() {
    let doc = EditorDoc::from_template(
        "abc",
        Some(&Formatting {
            source: String::new(),
            spans: vec![FormatSpan {
                start: 0,
                end: 3,
                style: FormatStyle::Bold,
            }],
        }),
    );
    assert!(doc.marks().is_empty());
    assert!(doc.to_template().1.is_none());
}
#[test]
fn duplicate_html_identity_does_not_change_when_an_earlier_chip_is_added() {
    let mut doc = EditorDoc::from_template("{{date}}", None);
    let id = doc.pills()[0].id;
    let html = doc.html_id(id);
    doc.insert_chip(0, "{{date}}", &|_| None).unwrap();
    assert_eq!(doc.html_id(id), html);
    assert_ne!(doc.pills()[0].id, id);
}
#[test]
fn escape_repair_does_not_rewrite_a_distant_windows_path() {
    let mut doc = EditorDoc::from_template(r"C:\Users {x{", None);
    doc.replace(10, 11, "");
    assert_eq!(doc.text(), r"C:\Users {{");
    assert_eq!(doc.to_template().0, r"C:\Users \{{");
}
#[test]
fn paste_rejects_a_forged_tone_even_when_the_metadata_agrees() {
    let doc = EditorDoc::from_template("{{date}}", None);
    let mut payload = doc.copy(0, doc.text().len()).unwrap();
    let chip = payload.runs[0].chip.as_mut().unwrap();
    chip.tone = Tone::Cursor;
    chip.html_id = "cursor:{{date}}".into();
    let mut dest = EditorDoc::from_template("", None);
    assert!(!dest.paste_own(0, 0, &payload, &|_| None));
    assert!(!dest.can_undo());
}
#[test]
fn labels_have_the_same_control_normalization_as_rune() {
    let src = "{{field name=\"a\nb\tc\u{85}d\u{2028}e\u{2029}f\"}}";
    let doc = EditorDoc::from_template(src, None);
    assert_eq!(doc.pills()[0].label, "a b c d e f");
    assert_eq!(doc.to_template().0, src);
}
#[test]
fn image_alt_trims_before_replacing_braces() {
    assert_eq!(images::sanitize_alt("  {a}\n  "), " a ");
    assert_eq!(images::sanitize_alt("{}"), "  ");
    assert_eq!(images::sanitize_alt(" \n"), "Image");
}
#[test]
fn unsafe_link_change_preserves_the_existing_link_and_history() {
    let mut doc = EditorDoc::from_template("abc", None);
    doc.change_format(0, 3, FormatChange::Link("https://example.com".into()));
    let old = doc.to_template();
    assert!(!doc.change_format(0, 3, FormatChange::Link("javascript:bad".into())));
    assert_eq!(doc.to_template(), old);
    assert!(doc.undo());
    assert!(doc.marks().is_empty());
}
#[test]
fn clearing_middle_of_unaligned_image_drops_the_whole_image() {
    let source = "ab{{date}}cd";
    let mut doc = EditorDoc::from_template(
        source,
        Some(&Formatting {
            source: source.into(),
            spans: vec![FormatSpan {
                start: 0,
                end: 12,
                style: FormatStyle::Image {
                    data_url: PNG.into(),
                    width: 32,
                },
            }],
        }),
    );
    let pill = doc.pills()[0].range.clone();
    doc.change_format(pill.start, pill.end, FormatChange::Clear);
    assert!(doc.images().is_empty());
    assert_eq!(doc.to_template().0, source);
    assert!(doc.undo());
    assert_eq!(doc.images().len(), 1);
}
#[test]
fn lists_do_not_insert_inside_a_multiline_image_chip() {
    let src = "a\nb";
    let mut doc = EditorDoc::from_template(
        src,
        Some(&Formatting {
            source: src.into(),
            spans: vec![FormatSpan {
                start: 0,
                end: 3,
                style: FormatStyle::Image {
                    data_url: PNG.into(),
                    width: 32,
                },
            }],
        }),
    );
    let id = doc.pills()[0].id;
    doc.toggle_list(0, doc.text().len(), false);
    assert_eq!(doc.to_template().0, "• a\nb");
    assert_eq!(doc.pills()[0].id, id);
}

#[test]
fn formatted_mode_survives_empty_spans_on_load() {
    let doc = EditorDoc::from_template(
        "abc",
        Some(&Formatting {
            source: "abc".into(),
            spans: vec![],
        }),
    );
    assert!(doc.to_template().1.is_some());
}

#[test]
fn formatted_toggle_is_one_undo_step_and_clears_spans() {
    let mut doc = EditorDoc::from_template("abc", None);
    assert!(!doc.formatted());
    assert!(doc.set_formatted(true));
    assert!(doc.to_template().1.unwrap().spans.is_empty());
    doc.change_format(0, 3, FormatChange::Bold);
    assert!(doc.set_formatted(false));
    assert!(doc.to_template().1.is_none());
    assert!(doc.undo());
    assert!(doc.formatted());
    assert_eq!(doc.marks()[0].range, 0..3);
    assert!(doc.undo());
    assert!(doc.to_template().1.unwrap().spans.is_empty());
    assert!(doc.undo());
    assert!(!doc.formatted());
    assert!(!doc.can_undo());
}

#[test]
fn replace_reports_the_applied_display_range() {
    let mut doc = EditorDoc::from_template(r"\{{", None);
    assert_eq!(doc.replace(1, 1, "x"), 1..2);
    assert_eq!(doc.replace(1, 2, ""), 1..1);
    let mut doc = EditorDoc::from_template("a{{date}}z", None);
    assert_eq!(doc.replace(2, 4, "X"), 1..2);
    assert_eq!(doc.text(), "aXz");
}

#[test]
fn slash_replacement_and_configuration_preserve_chip_identity_and_marks() {
    let mut doc = styled("Say /da", 4, 7);
    let id = doc.replace_with_chip(4..7, "{{date}}", &|_| None).unwrap();
    assert_eq!(doc.to_template().0, "Say {{date}}");
    assert_eq!(doc.marks()[0].range, doc.pills()[0].range);
    assert!(doc.set_chip_source(id, "{{time}}", &|_| None));
    assert_eq!(doc.pills()[0].id, id);
    assert_eq!(doc.pills()[0].source, "{{time}}");
    assert_eq!(doc.marks()[0].range, doc.pills()[0].range);
    assert!(!doc.set_chip_source(id, "not a tag", &|_| None));
    assert!(doc.undo());
    assert_eq!(doc.pills()[0].source, "{{date}}");
    assert!(doc.undo());
    assert_eq!(doc.text(), "Say /da");
    assert!(!doc.can_undo());
}

#[test]
fn relabel_keeps_source_marks_identity_and_undo_history() {
    let src = "{{snippet id=\"c5af65ac-e426-4046-802b-11ccdd3a62ba\"}} suffix";
    let mut doc = styled(src, 2, 8);
    let id = doc.pills()[0].id;
    doc.replace(doc.text().len(), doc.text().len(), "!");
    let saved = doc.to_template();
    doc.relabel(&|_| Some("New\nname"));
    assert_eq!(doc.pills()[0].label, "↳ New name");
    assert_eq!(doc.pills()[0].id, id);
    assert_eq!(doc.to_template(), saved);
    assert_eq!(doc.marks()[0].range, doc.pills()[0].range);
    assert!(doc.undo());
    assert_eq!(doc.pills()[0].label, "↳ New name");
    assert!(!doc.can_undo());
}

#[test]
fn fragment_adapter_validates_tags_tones_and_plain_text() {
    use kudzu_core::template::doc::CopyPayload;
    let display = "Hello Date · ISO date!";
    let pills = [(6..22, "time:{{date}}".to_string())];
    let payload = CopyPayload::from_fragment("Hello {{date}}!", &pills, display).unwrap();
    let mut doc = EditorDoc::from_template("", None);
    assert!(doc.paste_own(0, 0, &payload, &|_| None));
    assert_eq!(doc.text(), display);
    assert_eq!(doc.to_template().0, "Hello {{date}}!");
    assert!(CopyPayload::from_fragment("wrong", &pills, display).is_none());
    assert!(CopyPayload::from_fragment(
        "Hello {{date}}!",
        &[(6..22, "cursor:{{date}}".into())],
        display
    )
    .is_none());
    doc.replace(0, doc.text().len(), "{{date}}");
    assert!(doc.pills().is_empty());
    assert_eq!(doc.to_template().0, r"\{{date}}");
}

#[test]
fn combining_text_after_a_chip_survives_save_and_reload() {
    for suffix in ["\u{301}", "\u{200d}💻"] {
        let mut doc = EditorDoc::from_template("{{date}}", None);
        doc.replace(doc.text().len(), doc.text().len(), suffix);
        let saved = doc.to_template();
        let loaded = EditorDoc::from_template(&saved.0, saved.1.as_ref());
        assert_eq!(loaded.text(), doc.text());
        assert_eq!(loaded.pills().len(), 1);
    }
}

#[test]
fn lists_include_the_last_selected_line_ending_at_a_newline() {
    let mut doc = EditorDoc::from_template("first\r\nsecond\r\n", None);
    doc.toggle_list(0, doc.text().len(), true);
    assert_eq!(doc.text(), "1. first\r\n2. second\r\n");
}

#[test]
fn an_empty_edit_inside_a_grapheme_does_not_create_history() {
    let mut doc = EditorDoc::from_template("e\u{301}", None);
    doc.replace(1, 1, "");
    assert_eq!(doc.text(), "e\u{301}");
    assert!(!doc.undo());
}

#[test]
fn a_literal_backslash_and_brace_before_a_chip_do_not_escape_the_chip() {
    let mut doc = EditorDoc::from_template(r"C:\{", None);
    doc.insert_chip(doc.text().len(), "{{date}}", &|_| None);
    assert_eq!(doc.to_template().0, r"C:\\{{{date}}");
    let loaded = EditorDoc::from_template(&doc.to_template().0, None);
    assert_eq!(loaded.text(), doc.text());
    assert_eq!(loaded.pills().len(), 1);
}

#[test]
fn copying_one_escaped_brace_keeps_its_mark() {
    let doc = styled(r"\{{", 0, 3);
    let payload = doc.copy(1, 2).unwrap();
    let mut pasted = EditorDoc::from_template("", None);
    pasted.paste_own(0, 0, &payload, &|_| None);
    assert_eq!(pasted.text(), "{");
    assert_eq!(pasted.marks()[0].range, 0..1);
}

#[test]
fn pasting_bold_brace_beside_a_brace_marks_only_the_paste() {
    let payload = styled("{", 0, 1).copy(0, 1).unwrap();
    let mut doc = EditorDoc::from_template("{", None);
    doc.paste_own(1, 1, &payload, &|_| None);
    assert_eq!(doc.text(), "{{");
    assert_eq!(doc.marks()[0].range, 1..2);
}

#[test]
fn pasted_partial_chip_spans_keep_their_source_endpoints() {
    let doc = styled("{{date}}", 2, 4);
    let payload = doc.copy(0, doc.text().len()).unwrap();
    let mut dest = EditorDoc::from_template("x", None);
    dest.paste_own(1, 1, &payload, &|_| None);
    assert_eq!(dest.marks()[0].source, 3..5);
}

#[test]
fn copying_marks_after_a_named_chip_uses_the_copied_label_width() {
    let source = "{{snippet id=\"c5af65ac-e426-4046-802b-11ccdd3a62ba\"}} text";
    let from = source.len() - 4;
    let formatting = Formatting {
        source: source.into(),
        spans: vec![FormatSpan {
            start: from,
            end: source.len(),
            style: FormatStyle::Bold,
        }],
    };
    let doc = EditorDoc::from_template_named(source, Some(&formatting), &|_| Some("A"));
    let payload = doc.copy(0, doc.text().len()).unwrap();
    assert_eq!(payload.marks[0].source, from..source.len());
    let mut dest = EditorDoc::from_template("", None);
    dest.paste_own(0, 0, &payload, &|_| Some("A much longer name"));
    assert_eq!(&dest.text()[dest.marks()[0].range.clone()], "text");
}

#[test]
fn imported_image_labels_are_normalized_and_clearing_restores_literal_source() {
    let source = "a\nb\u{85}c";
    let mut doc = EditorDoc::from_template(
        source,
        Some(&Formatting {
            source: source.into(),
            spans: vec![FormatSpan {
                start: 0,
                end: 5,
                style: FormatStyle::Image {
                    data_url: PNG.into(),
                    width: 32,
                },
            }],
        }),
    );
    assert_eq!(doc.pills()[0].label, "a b c");
    assert_eq!(doc.to_template().0, source);
    doc.set_formatted(false);
    assert_eq!(doc.text(), source);
    assert_eq!(doc.to_template().0, source);
}

#[test]
fn snippet_names_can_borrow_the_live_library() {
    use std::collections::HashMap;
    let id = "c5af65ac-e426-4046-802b-11ccdd3a62ba";
    let mut library = HashMap::from([(id.to_string(), String::from("First name"))]);
    let source = format!("{{{{snippet id=\"{id}\"}}}}");
    let mut doc =
        EditorDoc::from_template_named(&source, None, &|id| library.get(id).map(String::as_str));
    assert_eq!(doc.pills()[0].label, "↳ First name");
    library.insert(id.to_string(), "Renamed".to_string());
    doc.relabel(&|id| library.get(id).map(String::as_str));
    assert_eq!(doc.pills()[0].label, "↳ Renamed");
    assert_eq!(doc.to_template().0, source);
    assert!(!doc.can_undo());
}
