use std::collections::HashMap;

use kudzu_core::formatting::{self, FormatSpan, FormatStyle, Formatting};
use kudzu_core::template::{self, RenderContext};
use kudzu_core::SnippetRecord;

fn bold(start: usize, end: usize) -> FormatSpan {
    FormatSpan {
        start,
        end,
        style: FormatStyle::Bold,
    }
}

#[test]
fn old_libraries_remain_plain_and_formatting_round_trips() {
    let mut snippet: SnippetRecord = serde_json::from_str(r#"{"id":"00000000-0000-0000-0000-000000000001","content":"Hello"}"#).unwrap();
    assert!(snippet.formatting.is_none());
    snippet.formatting = Some(Formatting {
        source: "Hello".into(),
        spans: vec![bold(0, 5)],
    });
    let restored: SnippetRecord = serde_json::from_str(&serde_json::to_string(&snippet).unwrap()).unwrap();
    assert_eq!(restored.formatting, snippet.formatting);
    assert_eq!(restored.content, "Hello");
}

#[test]
fn formatting_tracks_templates_escapes_unicode_and_the_cursor() {
    let source = r"😀 \{{literal}} {{field name=who}}{{cursor}}!";
    let start = source.find("{{field").unwrap() - 2; // UTF-8 → UTF-16 for the emoji.
    let end = start + "{{field name=who}}".len();
    let formatting = Formatting {
        source: source.into(),
        spans: vec![bold(start, end)],
    };
    let ctx = RenderContext {
        now: "2026-01-01T00:00:00Z".parse().unwrap(),
        locale: "en_US".into(),
        clipboard: None,
        form_values: HashMap::from([("who".into(), "<Sam & سلمى>".into())]),
        resolve_snippet: &|_| None,
        resolve_formatting: None,
        root_snippet_id: None,
            selection: None, clipboard_history: None, new_uuid: None, pick: None,
    };
    let result = template::render_formatted(source, Some(&formatting), &ctx);
    assert_eq!(result.text, "😀 {{literal}} <Sam & سلمى>!");
    assert_eq!(result.cursor_offset, Some(27));
    assert_eq!(result.formatting, Some(vec![bold(15, 27)]));
    let html = formatting::html(&result.text, result.formatting.as_deref().unwrap());
    assert!(html.contains("<strong>&lt;Sam &amp; سلمى&gt;</strong>!"), "{html}");
}

#[test]
fn stale_formatting_does_not_style_edits_from_an_older_mac() {
    let ctx = RenderContext {
        now: "2026-01-01T00:00:00Z".parse().unwrap(),
        locale: "en_US".into(),
        clipboard: None,
        form_values: HashMap::new(),
        resolve_snippet: &|_| None,
        resolve_formatting: None,
        root_snippet_id: None,
            selection: None, clipboard_history: None, new_uuid: None, pick: None,
    };
    let formatting = Formatting {
        source: "before".into(),
        spans: vec![bold(0, 6)],
    };
    let rendered = template::render_formatted("after", Some(&formatting), &ctx);
    assert_eq!(rendered.text, "after");
    assert!(rendered.formatting.is_none());
}

#[test]
fn html_handles_overlaps_and_rejects_active_links_and_invalid_ranges() {
    let spans = vec![
        bold(0, 2),
        FormatSpan {
            start: 1,
            end: 3,
            style: FormatStyle::Italic,
        },
    ];
    assert!(formatting::html("abc", &spans).contains("<strong>a</strong><strong><em>b</em></strong><em>c</em>"));
    for url in ["javascript:alert(1)", "data:text/html,bad", "https://ok\" onmouseover=\"bad"] {
        let html = formatting::html(
            "safe",
            &[FormatSpan {
                start: 0,
                end: 4,
                style: FormatStyle::Link { url: url.into() },
            }],
        );
        assert!(!html.contains("<a "), "{html}");
    }
    assert!(!formatting::html("😀", &[bold(1, 999)]).contains("<strong>"));
    let html = formatting::html(
        "Site",
        &[FormatSpan {
            start: 0,
            end: 4,
            style: FormatStyle::Link {
                url: "https://example.com/?a=1&b=2".into(),
            },
        }],
    );
    assert!(html.contains("href=\"https://example.com/?a=1&amp;b=2\""));
}

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg==";

#[test]
fn a_formatted_reply_preserves_its_nested_signature_but_a_plain_reply_does_not() {
    let id = uuid::Uuid::new_v4();
    let source = format!("Hello\n{{{{snippet id=\"{id}\"}}}}");
    let signature = Formatting {
        source: "[Logo]".into(),
        spans: vec![FormatSpan {
            start: 0,
            end: 6,
            style: FormatStyle::Image {
                data_url: PNG.into(),
                width: 120,
            },
        }],
    };
    let resolve = |key| (key == id).then(|| "[Logo]".into());
    let resolve_formatting = |key| (key == id).then(|| signature.clone());
    let ctx = RenderContext {
        now: "2026-01-01T00:00:00Z".parse().unwrap(),
        locale: "en_US".into(),
        clipboard: None,
        form_values: HashMap::new(),
        resolve_snippet: &resolve,
        resolve_formatting: Some(&resolve_formatting),
        root_snippet_id: None,
            selection: None, clipboard_history: None, new_uuid: None, pick: None,
    };
    let rich = template::render_formatted(
        &source,
        Some(&Formatting {
            source: source.clone(),
            spans: vec![],
        }),
        &ctx,
    );
    assert_eq!(rich.text, "Hello\n[Logo]");
    let spans = rich.formatting.unwrap();
    assert_eq!((spans[0].start, spans[0].end), (6, 12));
    assert!(formatting::html(&rich.text, &spans).contains(PNG));
    let plain = template::render(&source, &ctx);
    assert_eq!(plain.text, "Hello\n[Logo]");
    assert!(plain.formatting.is_none());
}

#[test]
fn embedded_images_round_trip_with_a_plain_text_alternative() {
    let mut snippet = SnippetRecord::new("Signature", vec![";sig".into()], "Hi [Logo]!");
    let image = FormatSpan {
        start: 3,
        end: 9,
        style: FormatStyle::Image {
            data_url: PNG.into(),
            width: 120,
        },
    };
    snippet.formatting = Some(Formatting {
        source: snippet.content.clone(),
        spans: vec![image.clone()],
    });
    let json = serde_json::to_string(&snippet).unwrap();
    let copy: SnippetRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(copy.formatting, snippet.formatting);
    assert_eq!(copy.content, "Hi [Logo]!");
    let html = formatting::html(&copy.content, std::slice::from_ref(&image));
    assert!(html.contains("width=\"120\" alt=\"[Logo]\""));
    assert!(html.contains(PNG));
    assert_eq!(
        formatting::insertion_text(&copy.content, std::slice::from_ref(&image)),
        "Hi \u{fffc}!"
    );
    assert_eq!(formatting::insertion_offset(9, &[image]), 4);
    assert!(!formatting::safe_image("https://example.com/image.png"));
    assert!(!formatting::safe_image("data:image/svg+xml;base64,PHN2Zz4="));
}

#[test]
fn distinct_formatting_is_not_deduplicated_and_removal_syncs() {
    use kudzu_core::interchange::dedupe;
    use kudzu_core::{sync, KudzuLibrary};
    let plain = SnippetRecord::new("Plain", vec![";x".into()], "Hello");
    let mut rich = SnippetRecord::new("Rich", vec![";x".into()], "Hello");
    rich.formatting = Some(Formatting {
        source: "Hello".into(),
        spans: vec![bold(0, 5)],
    });
    let mut library = KudzuLibrary {
        snippets: vec![plain, rich.clone()],
        ..Default::default()
    };
    assert!(dedupe::plan(&library, &HashMap::new()).snippet_merges.is_empty());
    let name = sync::snippet_name(rich.id);
    let mut index = sync::SyncIndex::default();
    let record = sync::encode(&name, &library, &index, "test").unwrap();
    sync::record_saved(&mut index, &record);
    library.snippets[1].formatting = None;
    let updated = sync::encode(&name, &library, &index, "test").unwrap();
    assert!(!updated.payload.contains("formatting"));
}
