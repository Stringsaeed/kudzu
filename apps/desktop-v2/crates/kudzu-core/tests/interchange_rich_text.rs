//! Rich text import: Espanso `markdown:`/`html:` matches and TextExpander rich text snippets
//! become Formatted snippets (`formatting.source == content`, UTF-16 spans).

mod interchange_support;

mod interchange {
    use std::collections::HashMap;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use crate::interchange_support::{write_files, TempDir};
    use kudzu_core::formatting::{FormatStyle, Formatting};
    use kudzu_core::interchange::{espanso, textexpander, ImportResult};
    use kudzu_core::template::{self, RenderContext, Severity};
    use kudzu_core::SnippetRecord;

    const DOT: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==";

    fn fixtures() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
    }

    fn espanso_rich() -> ImportResult {
        espanso::import_dir(&fixtures().join("espanso-rich")).unwrap()
    }

    fn textexpander_rich() -> ImportResult {
        textexpander::import_file(&fixtures().join("textexpander/rich.textexpander")).unwrap()
    }

    fn by_trigger(result: &ImportResult) -> HashMap<String, SnippetRecord> {
        result
            .library
            .snippets
            .iter()
            .map(|s| (s.triggers[0].clone(), s.clone()))
            .collect()
    }

    fn warnings_for<'a>(result: &'a ImportResult, trigger: &str) -> Vec<&'a str> {
        result
            .warnings
            .iter()
            .filter(|w| w.trigger.as_deref() == Some(trigger))
            .map(|w| w.message.as_str())
            .collect()
    }

    /// The source text under each span, with a short style name, in span order.
    fn styled(s: &SnippetRecord) -> Vec<(String, String)> {
        let f = s.formatting.as_ref().expect("formatted");
        let utf16: Vec<u16> = s.content.encode_utf16().collect();
        let mut spans = f.spans.clone();
        spans.sort();
        spans
            .iter()
            .map(|span| {
                let text = String::from_utf16(&utf16[span.start..span.end])
                    .expect("span on a char boundary");
                let style = match &span.style {
                    FormatStyle::Bold => "b".to_string(),
                    FormatStyle::Italic => "i".to_string(),
                    FormatStyle::Underline => "u".to_string(),
                    FormatStyle::Link { url } => format!("a {url}"),
                    FormatStyle::Image { width, .. } => format!("img {width}"),
                };
                (style, text)
            })
            .collect()
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut out: Vec<_> = list
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        out
    }

    fn sorted(mut v: Vec<(String, String)>) -> Vec<(String, String)> {
        v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        v
    }

    /// Every span survives rendering, so each endpoint is a valid source boundary, and the
    /// template parses without errors.
    fn assert_formatting_is_valid(s: &SnippetRecord) {
        let errors: Vec<_> = template::parse(&s.content)
            .diagnostics
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "{:?}: {} → {errors:?}",
            s.triggers,
            s.content
        );
        let Some(formatting) = &s.formatting else {
            return;
        };
        assert_eq!(formatting.source, s.content, "{:?}", s.triggers);
        let resolve = |_: uuid::Uuid| None;
        let context = RenderContext {
            now: chrono::DateTime::parse_from_rfc3339("2026-03-04T09:05:00+00:00").unwrap(),
            locale: "en_US".into(),
            clipboard: Some("clip".into()),
            form_values: HashMap::from([("Who".to_string(), "Sam".to_string())]),
            resolve_snippet: &resolve,
            resolve_formatting: None,
            root_snippet_id: None,
            selection: None,
            clipboard_history: None,
            new_uuid: None,
            pick: None,
        };
        let rendered = template::render_formatted(&s.content, Some(formatting), &context);
        assert_eq!(
            rendered.formatting.as_ref().map(Vec::len),
            Some(formatting.spans.len()),
            "{:?}: a span was dropped while rendering {:?} with {:?}",
            s.triggers,
            s.content,
            formatting.spans
        );
        for span in &formatting.spans {
            if let FormatStyle::Link { url } = &span.style {
                assert!(kudzu_core::formatting::safe_link(url), "{url}");
            }
            if let FormatStyle::Image { data_url, width } = &span.style {
                assert!(kudzu_core::formatting::safe_image(data_url));
                assert!((1..=512).contains(width));
            }
        }
    }

    // MARK: - Espanso

    #[test]
    fn espanso_markdown_becomes_formatted() {
        let result = espanso_rich();
        let t = by_trigger(&result);
        for s in result.library.snippets.iter() {
            assert_formatting_is_valid(s);
        }

        let md = &t[":md"];
        assert_eq!(md.content, "This text is bold, both and a link.");
        assert_eq!(
            sorted(styled(md)),
            pairs(&[
                ("i", "text"),
                ("b", "bold"),
                ("b", "both"),
                ("i", "both"),
                ("a https://kudzu.site", "a link"),
            ])
        );
        assert!(
            warnings_for(&result, ":md").is_empty(),
            "{:?}",
            warnings_for(&result, ":md")
        );

        let plain = &t[":md_plain"];
        assert_eq!(plain.content, "Nothing special");
        assert_eq!(
            plain.formatting,
            Some(Formatting {
                source: "Nothing special".into(),
                spans: vec![]
            })
        );
    }

    #[test]
    fn espanso_markdown_formats_template_tags_as_a_whole() {
        let result = espanso_rich();
        let t = by_trigger(&result);
        let vars = &t[":md_vars"];
        assert_eq!(
            vars.content,
            "Today {{date format=\"yyyy-MM-dd\"}} at {{cursor}} and my_var {{date format=\"yyyy-MM-dd\"}}"
        );
        // `_my_var_` is emphasis, but underscores inside a `{{…}}` reference never are.
        assert_eq!(
            sorted(styled(vars)),
            pairs(&[("b", "{{date format=\"yyyy-MM-dd\"}}"), ("i", "my_var")])
        );

        let braces = &t[":md_braces"];
        assert_eq!(braces.content, "a { b \\{{x}} and \\{{ c and back\\\\slash");
        assert_eq!(
            sorted(styled(braces)),
            pairs(&[("b", "a { b"), ("i", "\\{{ c")])
        );
    }

    #[test]
    fn espanso_markdown_offsets_are_utf16() {
        let t = by_trigger(&espanso_rich());
        let s = &t[":md_unicode"];
        assert_eq!(s.content, "😀 héllo 日本語 é😀x سلام");
        assert_eq!(sorted(styled(s)), pairs(&[("b", "héllo"), ("i", "é😀x")]));
        let spans = &s.formatting.as_ref().unwrap().spans;
        let bold = spans.iter().find(|s| s.style == FormatStyle::Bold).unwrap();
        assert_eq!((bold.start, bold.end), (3, 8));
    }

    #[test]
    fn espanso_markdown_blocks_and_what_is_dropped() {
        let result = espanso_rich();
        let t = by_trigger(&result);
        let s = &t[":md_blocks"];
        assert_eq!(
            s.content,
            "Title\n• one\n• two\n• nested\n1. first\n2. second\n\nquoted\ncode and gone\na\tb\n1\t2"
        );
        assert_eq!(sorted(styled(s)), pairs(&[("b", "Title"), ("b", "two")]));
        let warnings = warnings_for(&result, ":md_blocks");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        for dropped in [
            "heading",
            "nested list",
            "block quote",
            "code",
            "strikethrough",
            "horizontal rule",
            "table",
        ] {
            assert!(
                warnings[0].contains(dropped),
                "{dropped} missing from {}",
                warnings[0]
            );
        }
    }

    #[test]
    fn espanso_markdown_images_embed_only_inline_data() {
        let result = espanso_rich();
        let t = by_trigger(&result);
        let s = &t[":md_images"];
        assert_eq!(s.content, "[Dot] and [Remote] and [Wide]");
        let spans = &s.formatting.as_ref().unwrap().spans;
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].start, spans[0].end), (0, 5));
        assert_eq!(
            spans[0].style,
            FormatStyle::Image {
                data_url: DOT.into(),
                width: 16
            }
        );
        let warnings = warnings_for(&result, ":md_images");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("images that aren't embedded"),
            "{}",
            warnings[0]
        );
        assert!(
            warnings[0].contains("images larger than"),
            "{}",
            warnings[0]
        );
    }

    #[test]
    fn espanso_markdown_links_must_be_safe() {
        let result = espanso_rich();
        let t = by_trigger(&result);
        let s = &t[":md_unsafe"];
        assert_eq!(s.content, "click https://auto.link/x plain");
        assert_eq!(
            styled(s),
            pairs(&[("a https://auto.link/x", "https://auto.link/x")])
        );
        assert!(warnings_for(&result, ":md_unsafe")[0].contains("links"));
    }

    #[test]
    fn espanso_html_becomes_formatted() {
        let result = espanso_rich();
        let t = by_trigger(&result);

        let html = &t[":html"];
        assert_eq!(html.content, "Hello world & you\n\nmail under");
        assert_eq!(
            sorted(styled(html)),
            pairs(&[
                ("b", "world"),
                ("i", "you"),
                ("a mailto:a@b.c", "mail"),
                ("u", "under")
            ])
        );
        assert!(warnings_for(&result, ":html").is_empty());

        let style = &t[":html_style"];
        assert_eq!(style.content, "Red big u");
        assert_eq!(sorted(styled(style)), pairs(&[("b", "Red"), ("u", "u")]));
        let warnings = warnings_for(&result, ":html_style");
        assert_eq!(warnings.len(), 1);
        for dropped in ["fonts", "text sizes", "colors"] {
            assert!(
                warnings[0].contains(dropped),
                "{dropped} missing from {}",
                warnings[0]
            );
        }

        let lists = &t[":html_lists"];
        assert_eq!(lists.content, "• one\n• two\n3. c\n4. d");
        assert_eq!(styled(lists), pairs(&[("i", "d")]));

        let broken = &t[":html_broken"];
        assert_eq!(broken.content, "unclosed tags link");
        assert_eq!(
            sorted(styled(broken)),
            pairs(&[
                ("b", "unclosed tags link"),
                ("i", "tags link"),
                ("a https://x.y/", "link")
            ])
        );

        let table = &t[":html_table"];
        assert_eq!(table.content, "a\tb\nc\td");
        assert!(warnings_for(&result, ":html_table")[0].contains("table"));

        let image = &t[":html_image"];
        assert_eq!(image.content, "[Logo]\n[Remote]");
        assert_eq!(styled(image), pairs(&[("img 40", "[Logo]")]));

        let vars = &t[":html_vars"];
        assert_eq!(vars.content, "{{clipboard}} x");
        assert_eq!(styled(vars), pairs(&[("b", "{{clipboard}}")]));
    }

    #[test]
    fn espanso_hostile_rich_text_falls_back_without_hanging() {
        let temp = TempDir::new("espanso-hostile");
        let deep_html = format!("{}x{}", "<b><i>".repeat(20_000), "</i></b>".repeat(20_000));
        let deep_md = format!("{}x", "> ".repeat(5_000));
        let stars = "*a ".repeat(50_000);
        let brackets = "[".repeat(50_000) + &"](".repeat(50_000);
        let ticks = "`".repeat(30_000) + " x";
        let huge = "word ".repeat(400_000);
        let quote = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let yaml = format!(
            "matches:\n  - trigger: \":deep_html\"\n    html: \"{}\"\n  - trigger: \":deep_md\"\n    markdown: \"{}\"\n  - trigger: \":stars\"\n    markdown: \"{}\"\n  - trigger: \":brackets\"\n    markdown: \"{}\"\n  - trigger: \":ticks\"\n    markdown: \"{}\"\n  - trigger: \":huge\"\n    html: \"<b>{}</b>\"\n  - trigger: \":unterminated\"\n    html: \"<a href=\\\"https://x\"\n  - trigger: \":lt\"\n    html: \"1 < 2 <b\"\n",
            quote(&deep_html),
            quote(&deep_md),
            quote(&stars),
            quote(&brackets),
            quote(&ticks),
            quote(&huge),
        );
        let files = HashMap::from([("match/hostile.yml".to_string(), yaml)]);
        write_files(temp.path(), &files);
        let started = Instant::now();
        let result = espanso::import_dir(temp.path()).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "took {:?}",
            started.elapsed()
        );
        let t = by_trigger(&result);
        for s in result.library.snippets.iter() {
            assert_formatting_is_valid(s);
        }
        // Too deep or too large: imported as the original plain text, with a warning that says why.
        assert!(t[":deep_html"].formatting.is_none());
        assert_eq!(t[":deep_html"].content, deep_html);
        assert!(warnings_for(&result, ":deep_html")[0].contains("plain text"));
        assert!(t[":huge"].formatting.is_none());
        assert!(warnings_for(&result, ":huge")[0].contains("plain text"));
        // Unterminated markup is text, not a failure.
        assert_eq!(t[":unterminated"].content, "<a href=\"https://x");
        assert_eq!(t[":lt"].content, "1 < 2 <b");
        assert!(t[":stars"].formatting.is_some());
        assert!(t[":ticks"].formatting.is_some());
    }

    // MARK: - TextExpander

    #[test]
    fn textexpander_rich_text_is_cocoa_rtf_in_extra_info() {
        let result = textexpander_rich();
        let t = by_trigger(&result);
        for s in result.library.snippets.iter() {
            assert_formatting_is_valid(s);
        }

        // A real export's shape: one font, paragraph indents, nothing Kudzu needs to warn about.
        let flip = &t[";flip"];
        assert_eq!(flip.content, "(╯°□°）╯︵ ┻━┻");
        assert_eq!(
            flip.formatting,
            Some(Formatting {
                source: flip.content.clone(),
                spans: vec![]
            })
        );
        assert!(
            warnings_for(&result, ";flip").is_empty(),
            "{:?}",
            warnings_for(&result, ";flip")
        );

        let styled_snippet = &t[";styled"];
        assert_eq!(
            styled_snippet.content,
            "Hi {{field name=\"Who\"}}, see our site and café 😀 done 50%\n• item one\n• item two"
        );
        assert_eq!(
            sorted(styled(styled_snippet)),
            pairs(&[
                ("b", "{{field name=\"Who\"}}"),
                ("a https://kudzu.site/", "our site"),
                ("i", "café"),
                ("u", "😀 done"),
                ("b", "item"),
            ])
        );
        // A bold face of the same family is not a font change; link styling is not a color.
        assert!(
            warnings_for(&result, ";styled").is_empty(),
            "{:?}",
            warnings_for(&result, ";styled")
        );
    }

    #[test]
    fn textexpander_names_what_was_dropped() {
        let result = textexpander_rich();
        let t = by_trigger(&result);
        let loud = &t[";loud"];
        assert_eq!(loud.content, "Red big gone serif \\{{x}} back\\\\slash");
        assert!(loud.formatting.as_ref().unwrap().spans.is_empty());
        let warnings = warnings_for(&result, ";loud");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        for dropped in [
            "fonts",
            "text sizes",
            "colors",
            "strikethrough",
            "alignment",
        ] {
            assert!(
                warnings[0].contains(dropped),
                "{dropped} missing from {}",
                warnings[0]
            );
        }

        let pic = &t[";pic"];
        assert_eq!(pic.content, "Logo: [Image] and ");
        assert_eq!(styled(pic), pairs(&[("img 40", "[Image]")]));
        assert_eq!(
            pic.formatting.as_ref().unwrap().spans[0].style,
            FormatStyle::Image {
                data_url: DOT.into(),
                width: 40
            }
        );
        assert!(warnings_for(&result, ";pic")[0].contains("images that aren't embedded"));
    }

    #[test]
    fn textexpander_falls_back_to_plain_text() {
        let result = textexpander_rich();
        let t = by_trigger(&result);
        for (trigger, content, reason) in [
            (";archive", "Archived text", "archive"),
            (";mismatch", "The real text", "plain text"),
            (";broken", "Broken", "plain text"),
        ] {
            assert_eq!(t[trigger].content, content);
            assert!(t[trigger].formatting.is_none(), "{trigger}");
            let warnings = warnings_for(&result, trigger);
            assert_eq!(warnings.len(), 1, "{trigger}: {warnings:?}");
            assert!(warnings[0].contains(reason), "{trigger}: {}", warnings[0]);
        }
    }

    #[test]
    fn textexpander_hostile_rtf_never_hangs() {
        let plist = |rtf: &str, plain: &str| {
            let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
            format!(
                "<plist><dict><key>snippetsTE2</key><array><dict><key>abbreviation</key><string>x</string><key>plainText</key><string>{}</string><key>snippetType</key><integer>1</integer><key>extraInfo</key><dict><key>RTF</key><string>{}</string></dict></dict></array></dict></plist>",
                escape(plain),
                escape(rtf)
            )
        };
        let cases = [
            (
                format!("{{\\rtf1 {}x{}}}", "{".repeat(50_000), "}".repeat(50_000)),
                "x".to_string(),
            ),
            ("{\\rtf1 \\bin999999999 x}".to_string(), "x".to_string()),
            (
                "{\\rtf1 \\u99999999999999999999 x}".to_string(),
                "x".to_string(),
            ),
            (
                "{\\rtf1 {\\pict\\pngblip zz}x}".to_string(),
                "x".to_string(),
            ),
            (
                format!("{{\\rtf1 {}}}", "\\b x ".repeat(200_000)),
                "x".repeat(200_000),
            ),
            ("{\\rtf1 \\'zz\\'".to_string(), "x".to_string()),
        ];
        for (rtf, plain) in cases {
            let started = Instant::now();
            let result =
                textexpander::import_data(plist(&rtf, &plain).as_bytes(), "x.textexpander")
                    .unwrap();
            assert!(started.elapsed() < Duration::from_secs(10));
            for s in &result.library.snippets {
                assert_formatting_is_valid(s);
            }
        }
    }
}
