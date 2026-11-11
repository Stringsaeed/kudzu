//! Raycast snippet export import.

mod interchange {
    use kudzu_core::interchange::{detect, raycast, ImportResult};
    use kudzu_core::template::{self, Severity};
    use kudzu_core::{CaseMode, FireMode, SnippetRecord};

    const EXPORT: &str = r##"[
      { "name": "Email sign-off", "text": "Best,\n{cursor}", "keyword": "!bye" },
      { "name": "Today", "text": "{date} / {time} / {datetime} / {day}", "keyword": "!now" },
      { "name": "Custom date", "text": "{date format=\"yyyy-MM-dd\" offset=\"+1d\"} {date offset=\"-2M\"} {time offset=\"30m\"}", "keyword": "!d" },
      { "name": "Paste", "text": "{clipboard} {clipboard | trim} {clipboard offset=1} {clipboard offset=2 | trim} {clipboard offset=x}", "keyword": "!p" },
      { "name": "Reply", "text": "Hi {argument name=\"Name\" default=\"there\"},\n{argument name=\"Tone\" options=\"formal, casual\" default=\"casual\"}\n{snippet name=\"Email sign-off\"}", "keyword": "!reply" },
      { "name": "Unsupported", "text": "{selection} {uuid} {snippet name=\"Nope\"} {argument name=\"x\" bogus}", "keyword": "!u" },
      { "name": "Spaced offset", "text": "{date offset=\"+2y +5M\" format=\"yyyy-MM\"}", "keyword": "!so" },
      { "name": "Code", "text": "fn main() { println!(\"{}\", 1); } \\{clipboard} {{literal}} C:\\path", "keyword": "!code" },
      { "name": "No keyword", "text": "Only searchable" },
      { "name": "Empty", "text": "", "keyword": "!empty" },
      "junk"
    ]"##;

    fn render(source: &str) -> String {
        let resolve = |_: uuid::Uuid| None;
        let context = template::RenderContext {
            now: chrono::DateTime::parse_from_rfc3339("2026-03-04T09:05:00+00:00").unwrap(),
            locale: "en_US".into(),
            clipboard: Some("clip".into()),
            form_values: Default::default(),
            resolve_snippet: &resolve,
            resolve_formatting: None,
            root_snippet_id: None,
            selection: None,
            clipboard_history: None,
            new_uuid: None,
            pick: None,
        };
        template::render(source, &context).text
    }

    #[test]
    fn dates_render_with_the_converted_formats() {
        let result = imported();
        assert_eq!(
            render(&snippet(&result, "Today").content),
            "4 Mar 2026 / 09:05 / 4 Mar 2026 at 09:05 / Wednesday"
        );
        assert_eq!(
            render(&snippet(&result, "Custom date").content),
            "2026-03-05 4 Jan 2026 09:35"
        );
    }

    fn imported() -> ImportResult {
        raycast::import_json(EXPORT, "snippets.json").unwrap()
    }

    fn snippet<'a>(result: &'a ImportResult, name: &str) -> &'a SnippetRecord {
        result
            .library
            .snippets
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no snippet named {name}"))
    }

    fn warnings_for<'a>(result: &'a ImportResult, trigger: &str) -> Vec<&'a str> {
        result
            .warnings
            .iter()
            .filter(|w| w.trigger.as_deref() == Some(trigger))
            .map(|w| w.message.as_str())
            .collect()
    }

    #[test]
    fn imports_into_one_raycast_group() {
        let result = imported();
        assert_eq!(result.library.groups.len(), 1);
        assert_eq!(result.library.groups[0].name, "Raycast");
        // 9 usable entries; "Empty" and "junk" are skipped.
        assert_eq!(result.library.snippets.len(), 9);
        let sig = snippet(&result, "Email sign-off");
        assert_eq!(sig.triggers, vec!["!bye"]);
        assert_eq!(sig.content, "Best,\n{{cursor}}");
        assert_eq!(sig.fire_mode, FireMode::Immediate);
        assert_eq!(sig.case_mode, CaseMode::Exact);
        assert!(sig.is_enabled);
        assert_eq!(sig.group_id, Some(result.library.groups[0].id));
    }

    #[test]
    fn dates_use_raycast_defaults_and_convert_offsets() {
        let result = imported();
        assert_eq!(
            snippet(&result, "Today").content,
            "{{date format=\"d MMM yyyy\"}} / {{date format=\"HH:mm\"}} / {{date format=\"d MMM yyyy 'at' HH:mm\"}} / {{date format=\"EEEE\"}}"
        );
        assert_eq!(
            snippet(&result, "Custom date").content,
            "{{date format=\"yyyy-MM-dd\" offset=\"+1d\"}} {{date format=\"d MMM yyyy\" offset=\"-2m\"}} {{date format=\"HH:mm\" offset=\"+30min\"}}"
        );
    }

    #[test]
    fn offsets_with_spaces_between_parts_are_kept() {
        let result = imported();
        assert_eq!(
            snippet(&result, "Spaced offset").content,
            "{{date format=\"yyyy-MM\" offset=\"+2y5m\"}}"
        );
        assert!(warnings_for(&result, "!so").is_empty());
    }

    #[test]
    fn clipboard_modifiers_and_history_offsets() {
        let result = imported();
        assert_eq!(
            snippet(&result, "Paste").content,
            "{{clipboard}} {{clipboard trim=true}} {{clipboard history=1}} {{clipboard history=2 trim=true}} {clipboard offset=x}"
        );
        // Only the offset that isn't a number is left alone.
        let warnings = warnings_for(&result, "!p");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("offset=x"));
    }

    #[test]
    fn arguments_become_form_controls_and_snippets_link_by_name() {
        let result = imported();
        let reply = snippet(&result, "Reply");
        let sig = snippet(&result, "Email sign-off");
        assert_eq!(
            reply.content,
            format!(
                "Hi {{{{field name=\"Name\" default=\"there\"}}}},\n{{{{choice name=\"Tone\" options=\"formal|casual\" default=\"casual\"}}}}\n{{{{snippet id=\"{}\"}}}}",
                sig.id.hyphenated().to_string().to_uppercase()
            )
        );
        assert!(warnings_for(&result, "!reply").is_empty());
    }

    #[test]
    fn unsupported_placeholders_stay_text_with_warnings() {
        let result = imported();
        assert_eq!(
            snippet(&result, "Unsupported").content,
            "{{selection}} {{uuid upper=true}} {snippet name=\"Nope\"} {argument name=\"x\" bogus}"
        );
        assert_eq!(warnings_for(&result, "!u").len(), 2);
    }

    #[test]
    fn literal_braces_and_backslashes_survive() {
        let result = imported();
        let code = snippet(&result, "Code");
        // `\{` is Raycast's escape; `{{` must be escaped for Kudzu; the backslash is doubled
        // only where Kudzu would otherwise read it specially.
        assert_eq!(
            render(&code.content),
            "fn main() { println!(\"{}\", 1); } {clipboard} {{literal}} C:\\path"
        );
    }

    #[test]
    fn keywordless_snippets_import_turned_off() {
        let result = imported();
        let s = snippet(&result, "No keyword");
        assert!(s.triggers.is_empty());
        assert!(!s.is_enabled);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("no keyword")));
    }

    #[test]
    fn skipped_entries_are_reported() {
        let result = imported();
        assert!(result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some("!empty") && w.message.contains("no text")));
        assert!(result
            .warnings
            .iter()
            .any(|w| w.match_index == Some(10) && w.message.contains("isn't a snippet")));
    }

    #[test]
    fn every_imported_template_parses_without_errors() {
        for s in imported().library.snippets {
            let errors: Vec<_> = template::parse(&s.content)
                .diagnostics
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .collect();
            assert!(errors.is_empty(), "{}: {} → {errors:?}", s.name, s.content);
        }
    }

    #[test]
    fn rejects_files_that_are_not_raycast_exports() {
        assert!(raycast::import_json("{\"formatVersion\":1}", "x.json").is_err());
        assert!(raycast::import_json("not json", "x.json").is_err());
        assert!(raycast::import_json("[1, 2]", "x.json").is_err());
        let empty = raycast::import_json("[]", "x.json").unwrap();
        assert!(empty.library.snippets.is_empty() && empty.library.groups.is_empty());
    }

    #[test]
    fn accepts_a_byte_order_mark() {
        let result = raycast::import_json(
            "\u{feff}[{\"name\":\"a\",\"text\":\"b\",\"keyword\":\"c\"}]",
            "x.json",
        )
        .unwrap();
        assert_eq!(result.library.snippets.len(), 1);
    }

    #[test]
    fn detection_tells_raycast_from_kudzu_json() {
        assert_eq!(
            detect::file_format("json", EXPORT).unwrap(),
            detect::ImportSource::Raycast
        );
        assert_eq!(
            detect::file_format(
                "json",
                "{\"formatVersion\":1,\"groups\":[],\"snippets\":[]}"
            )
            .unwrap(),
            detect::ImportSource::Kudzu
        );
        // A leading byte-order mark is tolerated.
        assert_eq!(
            detect::file_format("json", &format!("\u{feff}{EXPORT}")).unwrap(),
            detect::ImportSource::Raycast
        );
        let dir = std::env::temp_dir().join(format!("kudzu-raycast-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("snippets.json");
        std::fs::write(&file, EXPORT).unwrap();
        assert_eq!(
            raycast::import_file(&file).unwrap().library.snippets.len(),
            9
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
