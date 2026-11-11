//! TextExpander (.textexpander property list and CSV) import.

mod interchange {
    use kudzu_core::interchange::{coordinator, detect, textexpander, ImportResult};
    use kudzu_core::template::{self, Severity};
    use kudzu_core::{CaseMode, FireMode, SnippetRecord};

    const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>groupInfo</key>
  <dict>
    <key>groupName</key><string>Work &amp; Co</string>
    <key>expandAfterMode</key><integer>2</integer>
  </dict>
  <key>snippetsTE2</key>
  <array>
    <dict>
      <key>abbreviation</key><string>sig</string>
      <key>abbreviationMode</key><integer>0</integer>
      <key>creationDate</key><date>2020-05-06T07:08:09Z</date>
      <key>modificationDate</key><date>2021-05-06T07:08:09Z</date>
      <key>label</key><string>Signature</string>
      <key>plainText</key><string>Best regards,
%filltext:name=Sender:default=Me%%|</string>
      <key>snippetType</key><integer>0</integer>
      <key>uuidString</key><string>6E4FE1E9-0000-4000-8000-000000000001</string>
    </dict>
    <dict>
      <key>abbreviation</key><string>ddate</string>
      <key>abbreviationMode</key><integer>1</integer>
      <key>plainText</key><string>%A, %B %e, %Y</string>
      <key>snippetType</key><integer>0</integer>
    </dict>
    <dict>
      <key>abbreviation</key><string>both</string>
      <key>plainText</key><string>%snippet:sig% + %snippet:missing% + %clipboard</string>
    </dict>
    <dict>
      <key>abbreviation</key><string>rich</string>
      <key>plainText</key><string>Bold &amp; 50% done</string>
      <key>snippetType</key><integer>1</integer>
    </dict>
    <dict>
      <key>abbreviation</key><string>script</string>
      <key>plainText</key><string>do shell script "ls"</string>
      <key>snippetType</key><integer>2</integer>
    </dict>
    <dict>
      <key>abbreviation</key><string></string>
      <key>label</key><string>Loose</string>
      <key>plainText</key><string>No trigger</string>
    </dict>
    <dict>
      <key>abbreviation</key><string>broken</string>
    </dict>
  </array>
</dict>
</plist>"#;

    fn plist() -> ImportResult {
        textexpander::import_data(PLIST.as_bytes(), "Work.textexpander").unwrap()
    }

    fn snippet<'a>(result: &'a ImportResult, name: &str) -> &'a SnippetRecord {
        result
            .library
            .snippets
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no snippet named {name}"))
    }

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
    fn plist_groups_and_dates() {
        let result = plist();
        assert_eq!(result.library.groups.len(), 1);
        assert_eq!(result.library.groups[0].name, "Work & Co");
        let sig = snippet(&result, "Signature");
        assert_eq!(sig.triggers, vec!["sig"]);
        assert_eq!(sig.case_mode, CaseMode::Exact);
        assert_eq!(sig.fire_mode, FireMode::Immediate);
        assert_eq!(sig.created_at.to_rfc3339(), "2020-05-06T07:08:09+00:00");
        assert_eq!(sig.updated_at.to_rfc3339(), "2021-05-06T07:08:09+00:00");
        assert_eq!(
            sig.content,
            "Best regards,\n{{field name=\"Sender\" default=\"Me\"}}{{cursor}}"
        );
        let date = snippet(&result, "ddate");
        assert_eq!(date.case_mode, CaseMode::Propagate);
        assert_eq!(date.content, "{{date format=\"EEEE, MMMM d, yyyy\"}}");
        assert_eq!(render(&date.content), "Wednesday, March 4, 2026");
    }

    #[test]
    fn nested_snippets_resolve_by_prefixed_abbreviation() {
        let result = plist();
        let sig = snippet(&result, "Signature");
        let both = snippet(&result, "both");
        assert_eq!(
            both.content,
            format!(
                "{{{{snippet id=\"{}\"}}}} + %snippet:missing% + {{{{clipboard}}}}",
                sig.id.hyphenated().to_string().to_uppercase()
            )
        );
        assert!(result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some("both") && w.message.contains("missing")));
    }

    #[test]
    fn rich_scripts_and_incomplete_snippets() {
        let result = plist();
        let rich = snippet(&result, "rich");
        assert_eq!(rich.content, "Bold & 50% done");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some("rich") && w.message.contains("formatting")));
        assert!(result.library.snippets.iter().all(|s| s.name != "script"));
        assert!(result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some("script") && w.message.contains("Script")));
        assert!(result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some("broken") && w.message.contains("missing")));
        let loose = snippet(&result, "Loose");
        assert!(loose.triggers.is_empty() && !loose.is_enabled);
        assert_eq!(result.library.snippets.len(), 5);
    }

    #[test]
    fn every_imported_template_parses_without_errors() {
        for s in plist().library.snippets {
            let errors: Vec<_> = template::parse(&s.content)
                .diagnostics
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .collect();
            assert!(errors.is_empty(), "{}: {} → {errors:?}", s.name, s.content);
        }
    }

    #[test]
    fn csv_with_header_groups_and_multiline_cells() {
        let csv = "\u{feff}Abbreviation,Content,Label,Group\r\n;hi,\"Hello,\r\nworld %Y\",Greeting,Chat\r\n;bye,Bye,,Chat\r\n;x,X,,Other\r\n;no,,,\r\n";
        let result = textexpander::import_data(csv.as_bytes(), "export.csv").unwrap();
        let names: Vec<_> = result
            .library
            .groups
            .iter()
            .map(|g| g.name.as_str())
            .collect();
        assert_eq!(names, vec!["Chat", "Other"]);
        assert_eq!(result.library.snippets.len(), 3);
        let hi = snippet(&result, "Greeting");
        assert_eq!(hi.triggers, vec![";hi"]);
        assert_eq!(hi.content, "Hello,\nworld {{date format=\"yyyy\"}}");
        assert_eq!(snippet(&result, ";bye").content, "Bye");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("no content")));
    }

    #[test]
    fn csv_with_textexpanders_own_header_skips_the_header_row() {
        let csv = "abbreviation,snippet,label\n;sig,Best regards,Sig\n";
        let result = textexpander::import_data(csv.as_bytes(), "export.csv").unwrap();
        assert_eq!(result.library.snippets.len(), 1);
        assert_eq!(snippet(&result, "Sig").triggers, vec![";sig"]);
    }

    #[test]
    fn csv_whose_first_abbreviation_starts_with_an_angle_bracket_is_not_a_plist() {
        let result = textexpander::import_data(b"<sig,Hello\n", "x.csv").unwrap();
        assert_eq!(result.library.snippets[0].triggers, vec!["<sig"]);
    }

    #[test]
    fn csv_semicolons_inside_quotes_do_not_change_the_delimiter() {
        let result =
            textexpander::import_data(b"sig,\"Hi; there; you\",Signature\nb,plain,Lab\n", "x.csv")
                .unwrap();
        assert_eq!(snippet(&result, "Signature").content, "Hi; there; you");
        assert_eq!(result.library.snippets.len(), 2);
    }

    #[test]
    fn csv_without_header_is_abbreviation_then_content() {
        let result = textexpander::import_data(b";a,Alpha\n;b,Beta,Bee\n", "plain.csv").unwrap();
        assert_eq!(result.library.groups[0].name, "plain");
        assert_eq!(result.library.snippets.len(), 2);
        assert_eq!(snippet(&result, "Bee").content, "Beta");
    }

    #[test]
    fn unreadable_files_say_what_to_do() {
        let binary = textexpander::import_data(b"bplist00\x00\x01", "x.textexpander").unwrap_err();
        assert!(binary.to_string().contains("CSV"));
        let zip = textexpander::import_data(b"PK\x03\x04rest", "x.textexpander").unwrap_err();
        assert!(zip.to_string().contains("CSV"));
        assert!(
            textexpander::import_data(b"<plist><dict></dict></plist>", "x.textexpander").is_err()
        );
        assert!(textexpander::import_data(b"<plist><dict>", "x.textexpander").is_err());
        assert!(textexpander::import_data(b"", "x.csv").is_err());
    }

    #[test]
    fn detection_and_loading_by_extension() {
        let dir = std::env::temp_dir().join(format!("kudzu-te-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Work.textexpander");
        std::fs::write(&file, PLIST).unwrap();
        assert_eq!(
            coordinator::detect_import_source(&file).unwrap(),
            detect::ImportSource::TextExpander
        );
        assert_eq!(
            coordinator::load_import(&file)
                .unwrap()
                .library
                .snippets
                .len(),
            5
        );
        assert!(textexpander::import_file(&dir).is_err());
        let other = dir.join("notes.txt");
        std::fs::write(&other, "x").unwrap();
        assert!(coordinator::detect_import_source(&other).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
