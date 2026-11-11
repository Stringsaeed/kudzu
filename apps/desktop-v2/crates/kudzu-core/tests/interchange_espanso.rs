//! Espanso import/export, ported from the Swift `EspansoInterchangeTests`, `ImportParsesCleanlyTests`
//! and `TemplateTagFormatterTests`.

mod interchange_support;

/// Named so `cargo test interchange` selects these.
mod interchange {
    use std::collections::{BTreeSet, HashMap};
    use std::path::{Path, PathBuf};

    use crate::interchange_support::{
        espanso_fixtures, normalize_snippet_ids, write_files, TempDir,
    };
    use kudzu_core::interchange::espanso::{self, EspansoImportResult};
    use kudzu_core::template::{self, Severity};
    use kudzu_core::{CaseMode, FireMode, InsertionMethod, KudzuLibrary, SnippetRecord};

    fn by_trigger(library: &KudzuLibrary) -> HashMap<String, SnippetRecord> {
        library
            .snippets
            .iter()
            .map(|s| (s.triggers[0].clone(), s.clone()))
            .collect()
    }

    fn has_warning(result: &EspansoImportResult, trigger: &str) -> bool {
        result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some(trigger))
    }

    #[test]
    fn golden_directory_import() {
        let result = espanso::import_dir(&espanso_fixtures()).unwrap();
        let library = &result.library;

        let names: Vec<&str> = library.groups.iter().map(|g| g.name.as_str()).collect();
        for expected in [
            "base",
            "_private",
            "dynamic",
            "forms",
            "unsupported",
            "work",
            "meetings",
        ] {
            assert!(
                names.contains(&expected),
                "missing group {expected} in {names:?}"
            );
        }
        assert!(!names.contains(&"_unimported"));

        let work = library.groups.iter().find(|g| g.name == "work").unwrap();
        let meetings = library
            .groups
            .iter()
            .find(|g| g.name == "meetings")
            .unwrap();
        assert_eq!(work.parent_id, None);
        assert_eq!(meetings.parent_id, Some(work.id));

        let t = by_trigger(library);

        // base.yml
        let sig = &t[":sig"];
        assert_eq!(sig.name, "Email Signature");
        assert_eq!(sig.content, "Best regards,\nJane Doe\n");
        assert_eq!(sig.fire_mode, FireMode::Immediate);
        assert_eq!(sig.case_mode, CaseMode::Exact);

        let hello = &t[":hello"];
        assert_eq!(hello.triggers, vec![":hello", ":hi"]);
        assert_eq!(hello.name, ":hello");
        assert_eq!(hello.content, "Hello there!");
        assert_eq!(hello.fire_mode, FireMode::WholeWord);
        assert_eq!(hello.case_mode, CaseMode::Propagate);

        assert_eq!(t["ther"].fire_mode, FireMode::WholeWord);
        assert_eq!(t["ther"].case_mode, CaseMode::Propagate);

        let div = &t[":div"];
        assert_eq!(div.name, "HTML Div");
        assert_eq!(div.content, "<div>{{cursor}}</div>");
        assert_eq!(div.insertion, Some(InsertionMethod::Type));

        assert_eq!(t[":escaped"].content, "Literal braces \\{{not_a_var}} here");

        // _private.yml via `imports:`, using base.yml's global var.
        assert_eq!(t[":secret"].name, "Imported Secret");
        assert_eq!(
            t[":secret"].content,
            "Loaded from _private.yml on {{date format=\"yyyy-MM-dd\"}}"
        );
        assert!(!t.contains_key(":unimported"));

        // forms.yml
        assert_eq!(t[":greet"].name, "Birthday Greeting");
        assert_eq!(
            t[":greet"].content,
            "Hey {{field name=\"name\" default=\"Friend\"}}, {{field name=\"note\" default=\"Happy Birthday\" multiline=true}}!"
        );
        assert_eq!(
            t[":order"].content,
            r#"Order: {{choice name="drink" options="Coffee|Tea\\|Green|Juice" default="Coffee"}} ({{choice name="size" options="Small|Medium|Large"}})"#
        );
        assert_eq!(
            t[":verbose_form"].content,
            "Hi {{field name=\"user\" default=\"Admin\"}}, role: {{choice name=\"role\" options=\"Developer|Designer\" default=\"Developer\"}}"
        );

        // dynamic.yml
        assert_eq!(t[":now"].content, "It's {{date format=\"HH:mm\"}}");
        assert_eq!(
            t[":tomorrow"].content,
            "Tomorrow is {{date format=\"yyyy-MM-dd\" offset=\"+1d\" locale=\"en_US\"}}"
        );
        assert_eq!(
            t[":full_date"].content,
            "{{date format=\"EEEE, MMMM d, yyyy hh:mm:ss a (EEE MMM dd ''yy 'day' DDD zzz xx) 100%\"}}"
        );
        assert_eq!(
            t[":paste_link"].content,
            "<a href=\"{{clipboard}}\">{{cursor}}</a>"
        );
        let sig_id = sig.id.hyphenated().to_string().to_uppercase();
        assert_eq!(
            t[":nested_sig"].content,
            format!("Thanks!\n{{{{snippet id=\"{sig_id}\"}}}}")
        );

        // work/meetings.yml
        let meeting = &t[":meeting"];
        assert_eq!(meeting.group_id, Some(meetings.id));
        assert_eq!(meeting.content, "Let's meet at {{cursor}} tomorrow.");
        assert_eq!(meeting.case_mode, CaseMode::Propagate);
        assert_eq!(
            t[":standup"].content,
            "Yesterday: {{field name=\"yesterday\"}}\nToday: {{field name=\"today\"}}"
        );

        // unsupported.yml: regex and image matches are skipped with warnings.
        assert!(!t.contains_key(":cat"));
        assert!(has_warning(&result, ":greet\\((?P<person>.*)\\)"));
        assert!(has_warning(&result, ":cat"));

        // Markdown and HTML become Formatted snippets (see interchange_rich_text.rs).
        assert_eq!(t[":rich_md"].content, "This text is bold");
        assert_eq!(
            t[":rich_md"].formatting.as_ref().map(|f| f.spans.len()),
            Some(2)
        );
        assert!(!has_warning(&result, ":rich_md"));
        assert_eq!(t[":rich_html"].content, "Hello world");
        assert_eq!(
            t[":rich_html"].formatting.as_ref().map(|f| f.spans.len()),
            Some(1)
        );
        assert!(!has_warning(&result, ":rich_html"));

        for (trigger, content) in [
            (":ip", "IP: \\{{output}}"),
            (":py", "Out: \\{{script_out}}"),
            (":broken_nest", "Missing: \\{{missing_match}}"),
        ] {
            assert_eq!(t[trigger].content, content);
            assert!(
                has_warning(&result, trigger),
                "expected a warning for {trigger}"
            );
        }

        // Random choices and fixed echo values have Kudzu equivalents.
        assert_eq!(t[":rand"].content, "Quote: {{random options=\"One|Two\"}}");
        assert_eq!(t[":echo_test"].content, "Echo: Fixed");
        assert!(!has_warning(&result, ":rand") && !has_warning(&result, ":echo_test"));

        assert_eq!(
            t[":weird_date"].content,
            "{{date format=\"yyyy-%'V'-%'u'\"}}"
        );
        assert!(result
            .warnings
            .iter()
            .any(|w| w.trigger.as_deref() == Some(":weird_date") && w.message.contains("%V")));

        // Warnings about one match carry its file, index and line.
        let cat = result
            .warnings
            .iter()
            .find(|w| w.trigger.as_deref() == Some(":cat"))
            .unwrap();
        assert_eq!(cat.file, "unsupported.yml");
        assert_eq!(cat.match_index, Some(1));
        assert_eq!(cat.line, Some(6));
    }

    #[test]
    fn round_trip_import_export_import() {
        let dir = espanso_fixtures().join("match");
        let files: Vec<PathBuf> = ["base.yml", "forms.yml", "dynamic.yml", "work/meetings.yml"]
            .iter()
            .map(|f| dir.join(f))
            .collect();
        let first = espanso::import_files(&files).unwrap();
        assert!(first.warnings.is_empty(), "{:?}", first.warnings);

        let exported = espanso::export(&first.library);
        for key in ["base.yml", "forms.yml", "dynamic.yml", "work/meetings.yml"] {
            assert!(
                exported.contains_key(key),
                "missing {key}: {:?}",
                exported.keys()
            );
        }

        let temp = TempDir::new("espanso-roundtrip");
        write_files(temp.path(), &exported);
        let second = espanso::import_dir(temp.path()).unwrap();
        assert!(second.warnings.is_empty(), "{:?}", second.warnings);
        assert_eq!(second.library.snippets.len(), first.library.snippets.len());

        let first_by = by_trigger(&first.library);
        let second_by = by_trigger(&second.library);
        for (trigger, s1) in &first_by {
            let s2 = second_by
                .get(trigger)
                .unwrap_or_else(|| panic!("missing {trigger} after round-trip"));
            assert_eq!(s2.name, s1.name);
            assert_eq!(s2.triggers, s1.triggers);
            assert_eq!(s2.fire_mode, s1.fire_mode);
            assert_eq!(s2.case_mode, s1.case_mode);
            assert_eq!(s2.insertion, s1.insertion);
            assert_eq!(
                normalize_snippet_ids(&s2.content, &second.library.snippets),
                normalize_snippet_ids(&s1.content, &first.library.snippets),
                "content of {trigger}"
            );
        }
    }

    #[test]
    fn deterministic_ordering_and_default_paths() {
        let first = espanso::import_dir(&espanso_fixtures()).unwrap();
        let second = espanso::import_dir(&espanso_fixtures()).unwrap();
        let names = |r: &EspansoImportResult| {
            r.library
                .groups
                .iter()
                .map(|g| g.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&first), names(&second));
        assert_eq!(
            names(&first),
            vec![
                "base",
                "_private",
                "dynamic",
                "forms",
                "unsupported",
                "work",
                "meetings"
            ]
        );
        let triggers = |r: &EspansoImportResult| {
            r.library
                .snippets
                .iter()
                .map(|s| s.triggers.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(triggers(&first), triggers(&second));
        let contents = |r: &EspansoImportResult| {
            r.library
                .snippets
                .iter()
                .map(|s| normalize_snippet_ids(&s.content, &r.library.snippets))
                .collect::<Vec<_>>()
        };
        assert_eq!(contents(&first), contents(&second));

        let defaults = espanso::default_config_dirs(Path::new("/Users/tester"));
        assert_eq!(
            defaults[0],
            PathBuf::from("/Users/tester/Library/Application Support/espanso")
        );
        assert_eq!(defaults[1], PathBuf::from("/Users/tester/.config/espanso"));
        assert_eq!(defaults.len(), if cfg!(windows) { 3 } else { 2 });
    }

    #[test]
    fn fixture_imports_parse_without_errors() {
        let result = espanso::import_dir(&espanso_fixtures()).unwrap();
        assert!(!result.library.snippets.is_empty());
        for s in &result.library.snippets {
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
        }
    }

    #[test]
    fn literal_backslashes_survive_import_render_and_export() {
        let temp = TempDir::new("espanso-backslash");
        let yaml = "matches:\n  - trigger: \";path\"\n    replace: 'C:\\Users\\me \\\\server'\n  - trigger: \";bs\"\n    replace: 'ends with backslash \\'\n";
        std::fs::create_dir_all(temp.path().join("match")).unwrap();
        std::fs::write(temp.path().join("match/base.yml"), yaml).unwrap();

        let library = espanso::import_dir(temp.path()).unwrap().library;
        let render = |trigger: &str| {
            let s = library
                .snippets
                .iter()
                .find(|s| s.triggers.iter().any(|t| t == trigger))
                .unwrap();
            let resolve = |_| None;
            let ctx = template::RenderContext {
                now: chrono::Utc::now().fixed_offset(),
                locale: "en_US".into(),
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
            template::render(&s.content, &ctx).text
        };
        assert_eq!(render(";path"), r"C:\Users\me \\server");
        assert_eq!(render(";bs"), r"ends with backslash \");

        let exported = espanso::export(&library);
        let again = TempDir::new("espanso-backslash-again");
        write_files(&again.path().join("match"), &exported);
        let reimported = espanso::import_dir(again.path()).unwrap().library;
        let contents = |l: &KudzuLibrary| {
            l.snippets
                .iter()
                .map(|s| s.content.clone())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(contents(&reimported), contents(&library));
    }

    #[test]
    fn exporter_reads_bare_numbers_and_keeps_unrecognized_tags() {
        let content = "{{field name=qty default=5}}\n{{future size=3}} \\\\server";
        let library = KudzuLibrary {
            snippets: vec![SnippetRecord::new("n", vec![";n".into()], content)],
            ..KudzuLibrary::default()
        };
        let exported = espanso::export(&library);
        let yaml = &exported["base.yml"];
        assert!(yaml.contains('5'), "{yaml}");
        assert!(yaml.contains(r"\{\{future size=3}}"), "{yaml}");
        assert!(yaml.contains(r"\server"), "{yaml}");
    }

    #[test]
    fn exporter_writes_vars_for_dates_clipboard_nested_snippets_and_mixed_forms() {
        let mut target = SnippetRecord::new("Sig", vec![";sig".into()], "Regards");
        target.case_mode = CaseMode::Exact;
        let content = format!(
            "On {{{{date format=\"yyyy-MM-dd\" offset=\"+1d\"}}}} {{{{clipboard}}}} {{{{snippet id=\"{}\"}}}} {{{{field name=\"who\"}}}}",
            target.id
        );
        let mut mixed = SnippetRecord::new("Mixed", vec![";mix".into()], content);
        mixed.fire_mode = FireMode::WholeWord;
        mixed.insertion = Some(InsertionMethod::Paste);
        let library = KudzuLibrary {
            snippets: vec![target, mixed],
            ..KudzuLibrary::default()
        };

        let exported = espanso::export(&library);
        let temp = TempDir::new("espanso-vars");
        write_files(temp.path(), &exported);
        let back = espanso::import_dir(temp.path()).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        let t = by_trigger(&back.library);
        let sig_id = t[";sig"].id.hyphenated().to_string().to_uppercase();
        assert_eq!(
            t[";mix"].content,
            format!(
                "On {{{{date format=\"yyyy-MM-dd\" offset=\"+1d\"}}}} {{{{clipboard}}}} {{{{snippet id=\"{sig_id}\"}}}} {{{{field name=\"who\"}}}}"
            )
        );
        assert_eq!(t[";mix"].fire_mode, FireMode::WholeWord);
        assert_eq!(t[";mix"].case_mode, CaseMode::Propagate);
        assert_eq!(t[";mix"].insertion, Some(InsertionMethod::Paste));
        assert_eq!(t[";sig"].case_mode, CaseMode::Exact);
    }

    #[test]
    fn groups_become_nested_files_and_same_named_groups_share_one() {
        let mut parent = kudzu_core::GroupRecord::new("Work/Stuff");
        parent.sort_index = 0;
        let mut child = kudzu_core::GroupRecord::new("_Emails");
        child.parent_id = Some(parent.id);
        let twin = kudzu_core::GroupRecord::new("Work/Stuff");
        let in_group = |g: &kudzu_core::GroupRecord, trigger: &str| {
            let mut s = SnippetRecord::new(trigger, vec![trigger.into()], "x");
            s.group_id = Some(g.id);
            s
        };
        let library = KudzuLibrary {
            snippets: vec![
                in_group(&child, ";a"),
                in_group(&parent, ";b"),
                in_group(&twin, ";c"),
            ],
            groups: vec![parent.clone(), child.clone(), twin.clone()],
            ..KudzuLibrary::default()
        };
        let exported = espanso::export(&library);
        assert_eq!(
            exported.keys().cloned().collect::<Vec<_>>(),
            vec!["Work_Stuff.yml", "Work_Stuff/Emails.yml"]
        );
        assert!(
            exported["Work_Stuff.yml"].contains(";b") && exported["Work_Stuff.yml"].contains(";c")
        );
    }

    #[test]
    fn malformed_and_missing_inputs_warn_instead_of_failing() {
        let temp = TempDir::new("espanso-malformed");
        let files: HashMap<String, String> = HashMap::from([
            ("bad.yml".into(), "matches: [unclosed\n".into()),
            ("list.yml".into(), "- just\n- a list\n".into()),
            ("empty.yml".into(), "# only a comment\n".into()),
            (
                "ok.yml".into(),
                "imports:\n  - missing.yml\nmatches:\n  - just a string\n  - trigger: ':x'\n    left_word: true\n    replace: 'x'\n  - trigger: ':none'\n".into(),
            ),
        ]);
        write_files(temp.path(), &files);
        let result = espanso::import_dir(temp.path()).unwrap();
        let messages: Vec<&str> = result.warnings.iter().map(|w| w.message.as_str()).collect();
        assert!(
            messages.iter().any(|m| m.starts_with("YAML syntax error")),
            "{messages:?}"
        );
        assert!(messages.contains(&"Expected top-level YAML mapping in match file."));
        assert!(messages.contains(&"Match file not found at 'missing.yml'."));
        assert!(messages.contains(&"Malformed match entry at index 0 was skipped."));
        assert!(messages.contains(
            &"Partial word boundary ('left_word'/'right_word') mapped to whole-word mode."
        ));
        assert!(messages
            .contains(&"Match has no replacement content ('replace', 'form', 'markdown', or 'html') and was skipped."));
        let t = by_trigger(&result.library);
        assert_eq!(t[":x"].fire_mode, FireMode::WholeWord);
        assert_eq!(t.len(), 1);
        // The empty file still yields a group.
        assert!(result.library.groups.iter().any(|g| g.name == "empty"));

        let json = serde_json::to_value(&result.warnings[0]).unwrap();
        assert!(json.get("message").is_some());
    }
}
