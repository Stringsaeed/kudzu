use super::*;
use chrono::{Duration, TimeZone};

fn item(pairs: &[(&str, &[u8])]) -> PasteItem {
    pairs
        .iter()
        .map(|(t, d)| (t.to_string(), d.to_vec()))
        .collect()
}

fn text(s: &str) -> Vec<PasteItem> {
    vec![item(&[(uti::PLAIN_TEXT, s.as_bytes())])]
}

fn at(h: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap() + Duration::hours(h)
}

#[test]
fn before_current_skips_the_entry_that_is_on_the_clipboard() {
    let mut h = History::new();
    for (i, s) in ["oldest", "middle", "newest"].iter().enumerate() {
        h.ingest(&classify(&text(s), None).unwrap(), None, None, at(i as i64));
    }
    let short = h.clone();
    let text_of = |n, current| {
        short
            .before_current(n, current)
            .and_then(|e| e.text.clone())
    };
    assert_eq!(text_of(1, Some("newest")).as_deref(), Some("middle"));
    assert_eq!(text_of(2, Some("newest")).as_deref(), Some("oldest"));
    assert_eq!(text_of(3, Some("newest")), None);
    // The clipboard holds something the history never recorded: the newest entry is the previous copy.
    assert_eq!(
        text_of(1, Some("from a password manager")).as_deref(),
        Some("newest")
    );
    assert_eq!(text_of(1, None).as_deref(), Some("newest"));
    assert_eq!(text_of(0, Some("unrecorded")), None);
    assert_eq!(text_of(0, Some("newest")).as_deref(), Some("newest"));
    assert_eq!(History::new().before_current(1, Some("x")), None);
    // Long text is compared by the stored excerpt.
    let long = "x".repeat(TEXT_EXCERPT_LIMIT + 50);
    h.ingest(&classify(&text(&long), None).unwrap(), None, None, at(9));
    assert_eq!(
        h.before_current(1, Some(&long))
            .and_then(|e| e.text.as_deref()),
        Some("newest")
    );
}

#[test]
fn classifies_text_links_colors_and_code() {
    assert_eq!(
        classify(&text("Hello there"), None).unwrap().kind,
        ClipKind::Text
    );
    assert_eq!(
        classify(&text("https://kudzu.app/docs?x=1"), None)
            .unwrap()
            .kind,
        ClipKind::Link
    );
    assert_eq!(
        classify(&text("  mailto:me@example.com "), None)
            .unwrap()
            .kind,
        ClipKind::Link
    );
    assert_eq!(
        classify(&text("www.example.org"), None).unwrap().kind,
        ClipKind::Link
    );
    let c = classify(&text("#1E90FF"), None).unwrap();
    assert_eq!(
        (c.kind, c.color.as_deref()),
        (ClipKind::Color, Some("#1e90ff"))
    );
    assert_eq!(
        classify(&text("oklch(0.7 0.1 250)"), None).unwrap().kind,
        ClipKind::Color
    );
    assert_eq!(
        classify(&text("rgb(255, 0, 0)"), None).unwrap().kind,
        ClipKind::Color
    );
    assert_eq!(
        classify(&text("git commit -m \"fix\""), None).unwrap().kind,
        ClipKind::Code
    );
    let js = "function add(a, b) {\n  const sum = a + b;\n  return sum;\n}";
    assert_eq!(classify(&text(js), None).unwrap().kind, ClipKind::Code);
    let prose = "Dear team,\nThanks for the meeting today.\nSee you tomorrow.";
    assert_eq!(classify(&text(prose), None).unwrap().kind, ClipKind::Text);
    // Not colours or links.
    assert_eq!(
        classify(&text("#hashtag"), None).unwrap().kind,
        ClipKind::Text
    );
    assert_eq!(
        classify(&text("https://"), None).unwrap().kind,
        ClipKind::Text
    );
    assert_eq!(
        classify(&text("see https://a.com now"), None).unwrap().kind,
        ClipKind::Text
    );
}

#[test]
fn code_apps_tip_the_balance() {
    assert_eq!(
        classify(&text("let x = foo();"), Some("com.microsoft.VSCode"))
            .unwrap()
            .kind,
        ClipKind::Code
    );
    assert_eq!(
        classify(&text("let x = foo();"), Some("com.apple.Notes"))
            .unwrap()
            .kind,
        ClipKind::Text
    );
}

#[test]
fn ignores_whitespace_and_empty() {
    assert!(classify(&text("  \n "), None).is_none());
    assert!(classify(&[], None).is_none());
}

#[test]
fn files_win_and_decode_paths() {
    let items = vec![
        item(&[
            (uti::FILE_URL, b"file:///Users/me/My%20Report.pdf"),
            (uti::PLAIN_TEXT, b"My Report.pdf"),
        ]),
        item(&[(uti::FILE_URL, b"file://localhost/tmp/folder/")]),
    ];
    let c = classify(&items, None).unwrap();
    assert_eq!(c.kind, ClipKind::File);
    assert_eq!(
        c.files,
        vec![
            "/Users/me/My Report.pdf".to_string(),
            "/tmp/folder".to_string()
        ]
    );
}

#[test]
fn lone_images_are_images_but_text_wins() {
    let img = vec![item(&[(uti::TIFF, b"tiffdata"), (uti::PNG, b"pngdata")])];
    let c = classify(&img, None).unwrap();
    assert_eq!(
        (c.kind, c.image_type.as_deref()),
        (ClipKind::Image, Some(uti::PNG))
    );
    let cells = vec![item(&[(uti::PLAIN_TEXT, b"a\tb"), (uti::TIFF, b"x")])];
    assert_eq!(classify(&cells, None).unwrap().kind, ClipKind::Text);
}

#[test]
fn rich_text_is_flagged_and_unknown_data_is_other() {
    let rich = vec![item(&[(uti::RTF, b"{\\rtf1}"), (uti::PLAIN_TEXT, b"Bold")])];
    assert!(classify(&rich, None).unwrap().rich);
    let other = vec![item(&[("com.figma.private", b"\x01\x02")])];
    let c = classify(&other, None).unwrap();
    assert_eq!((c.kind, c.size), (ClipKind::Other, 2));
}

#[test]
fn private_markers_are_detected() {
    let concealed = vec![item(&[
        (uti::PLAIN_TEXT, b"hunter2"),
        ("org.nspasteboard.ConcealedType", b""),
    ])];
    assert!(is_private(&concealed));
    assert!(!is_private(&text("hi")));
}

#[test]
fn repeats_fold_into_one_entry_with_timings() {
    let mut h = History::new();
    let a = classify(&text("alpha"), None).unwrap();
    let b = classify(&text("beta"), None).unwrap();
    let first = h.ingest(&a, None, None, at(0));
    assert!(matches!(first, Ingest::Added(_)));
    h.ingest(&b, None, None, at(1));
    let again = h.ingest(
        &a,
        Some(ClipSource {
            bundle_id: "x".into(),
            name: "X".into(),
            path: None,
        }),
        None,
        at(2),
    );
    assert!(matches!(again, Ingest::Repeated(_)));
    assert_eq!(h.entries.len(), 2);
    let top = &h.entries[0];
    assert_eq!(top.id, first.entry().id, "same id across repeats");
    assert_eq!(top.copy_count, 2);
    assert_eq!(top.timings, vec![at(0), at(2)]);
    assert_eq!(top.first_copied_at, at(0));
    assert_eq!(top.copied_at, at(2));
    assert_eq!(top.source.as_ref().unwrap().name, "X");
}

#[test]
fn timings_are_capped() {
    let mut h = History::new();
    let a = classify(&text("x"), None).unwrap();
    for i in 0..(MAX_TIMINGS as i64 + 10) {
        h.ingest(&a, None, None, at(i));
    }
    let e = &h.entries[0];
    assert_eq!(e.timings.len(), MAX_TIMINGS);
    assert_eq!(e.copy_count as usize, MAX_TIMINGS + 10);
    assert_eq!(*e.timings.last().unwrap(), at(MAX_TIMINGS as i64 + 9));
}

#[test]
fn detects_secrets_and_keeps_corrections() {
    let mut h = History::new();
    let pw = h
        .ingest(
            &classify(&text("P@ssw0rd!"), None).unwrap(),
            None,
            None,
            at(0),
        )
        .entry()
        .clone();
    assert_eq!(pw.secret, Some(SecretKind::Password));
    assert!(pw.is_secret());
    let note = h
        .ingest(
            &classify(&text("lunch at noon"), None).unwrap(),
            None,
            None,
            at(1),
        )
        .entry()
        .clone();
    assert!(!note.is_secret());

    // The user's choice wins, and survives copying the same text again.
    assert!(!h.set_secret(pw.id, Some(false)).unwrap().is_secret());
    let again = h
        .ingest(
            &classify(&text("P@ssw0rd!"), None).unwrap(),
            None,
            None,
            at(2),
        )
        .entry()
        .clone();
    assert_eq!(again.id, pw.id);
    assert!(!again.is_secret());
    assert!(h.set_secret(note.id, Some(true)).unwrap().is_secret());
    assert!(h.set_secret(pw.id, None).unwrap().is_secret());
}

#[test]
fn rescans_history_from_older_rules_once() {
    let mut h = History::new();
    h.ingest(
        &classify(&text("token: abcd1234efgh"), None).unwrap(),
        None,
        None,
        at(0),
    );
    h.entries[0].secret = None;
    h.secret_rules = 0;
    assert!(h.rescan_secrets());
    assert_eq!(h.entries[0].secret, Some(SecretKind::Credential));
    assert!(!h.rescan_secrets(), "already current");
    // An index written before the field existed decodes as needing a scan.
    let old: History = serde_json::from_str(r#"{"version":1,"entries":[]}"#).unwrap();
    assert_eq!(old.secret_rules, 0);
}

#[test]
fn excerpt_keeps_lengths_of_long_text() {
    let mut h = History::new();
    let long = "é".repeat(TEXT_EXCERPT_LIMIT + 100) + "\nend";
    let e = h
        .ingest(&classify(&text(&long), None).unwrap(), None, None, at(0))
        .entry()
        .clone();
    assert_eq!(e.text.unwrap().chars().count(), TEXT_EXCERPT_LIMIT);
    assert_eq!(e.text_len, TEXT_EXCERPT_LIMIT + 104);
    assert_eq!(e.line_count, 2);
}

#[test]
fn retention_keeps_pins_and_respects_forever() {
    let mut h = History::new();
    for (i, s) in ["old", "older", "fresh"].iter().enumerate() {
        h.ingest(
            &classify(&text(s), None).unwrap(),
            None,
            None,
            at(i as i64 * 24),
        );
    }
    // at(0)=old, at(24)=older, at(48)=fresh; now = at(24*9)
    let old_id = h
        .entries
        .iter()
        .find(|e| e.text.as_deref() == Some("old"))
        .unwrap()
        .id;
    h.set_pinned(old_id, true);
    let mut forever = h.clone();
    assert!(forever.prune(at(24 * 400), 0).is_empty());
    let removed = h.prune(at(24 * 9), 7);
    assert_eq!(
        removed
            .iter()
            .map(|e| e.text.clone().unwrap())
            .collect::<Vec<_>>(),
        vec!["older"]
    );
    assert_eq!(h.entries.len(), 2);
    assert_eq!(h.get(old_id).unwrap().expires_at(7), None);
    let fresh = h
        .entries
        .iter()
        .find(|e| e.text.as_deref() == Some("fresh"))
        .unwrap();
    assert_eq!(fresh.expires_at(7), Some(at(48 + 24 * 7)));
}

#[test]
fn caps_unpinned_entries() {
    let mut h = History::new();
    for i in 0..(MAX_ENTRIES + 3) {
        h.ingest(
            &classify(&text(&format!("n{i}")), None).unwrap(),
            None,
            None,
            at(0),
        );
    }
    let removed = h.prune(at(1), 0);
    assert_eq!(removed.len(), 3);
    assert_eq!(h.entries.len(), MAX_ENTRIES);
    // The oldest go first.
    assert_eq!(removed[0].text.as_deref(), Some("n2"));
}

#[test]
fn remove_restore_clear_and_use() {
    let mut h = History::new();
    for (i, s) in ["a", "b", "c"].iter().enumerate() {
        h.ingest(&classify(&text(s), None).unwrap(), None, None, at(i as i64));
    }
    let b = h.entries[1].id;
    let gone = h.remove(&[b]);
    assert_eq!(gone.len(), 1);
    assert_eq!(h.restore(gone.clone()).len(), 1);
    assert_eq!(
        h.entries
            .iter()
            .map(|e| e.text.clone().unwrap())
            .collect::<String>(),
        "cba"
    );
    assert!(h.restore(gone).is_empty(), "no duplicates");

    let a = h.entries[2].id;
    let used = h.mark_used(a, at(10)).unwrap();
    assert_eq!((used.use_count, h.entries[0].id), (1, a));

    h.set_pinned(a, true);
    assert_eq!(h.clear(false).len(), 2);
    assert_eq!(h.entries.len(), 1);
    assert_eq!(h.clear(true).len(), 1);
}

#[test]
fn blobs_round_trip_and_reject_garbage() {
    let items = vec![
        item(&[(uti::PLAIN_TEXT, b"hi"), (uti::RTF, b"{\\rtf}")]),
        item(&[(uti::PNG, &[0u8, 1, 2, 255])]),
    ];
    let bytes = encode_items(&items);
    assert_eq!(decode_items(&bytes).unwrap(), items);
    assert_eq!(text_of(&items).as_deref(), Some("hi"));
    assert_eq!(image_of(&items).unwrap().1, "image/png");
    assert!(decode_items(b"nope").is_none());
    assert!(decode_items(&bytes[..bytes.len() - 1]).is_none());
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(decode_items(&extra).is_none());
    assert_eq!(
        decode_items(&encode_items(&[])).unwrap(),
        Vec::<PasteItem>::new()
    );
}

#[test]
fn history_json_is_stable() {
    let mut h = History::new();
    h.ingest(
        &classify(&text("#fff"), None).unwrap(),
        None,
        Some(ImageSize {
            width: 1,
            height: 2,
        }),
        at(0),
    );
    let json = serde_json::to_string(&h).unwrap();
    assert!(json.contains("\"copiedAt\"") && json.contains("\"kind\":\"color\""));
    let back: History = serde_json::from_str(&json).unwrap();
    assert_eq!(back, h);
    // Older files without optional fields still load.
    let minimal = r#"{"entries":[{"id":"6f1c2b9e-8a33-4f0e-9a51-2a7b9a5f2c11","kind":"text","hash":"00","firstCopiedAt":"2026-11-01T00:00:00Z","copiedAt":"2026-11-01T00:00:00Z","copyCount":1}]}"#;
    let h: History = serde_json::from_str(minimal).unwrap();
    assert_eq!((h.version, h.entries[0].pinned), (1, false));
}

#[test]
fn helpers() {
    assert_eq!(excerpt("héllo", 2), "hé");
    assert_eq!(image_mime_for_path("/a/B.JPG"), Some("image/jpeg"));
    assert_eq!(image_mime_for_path("/a/b.txt"), None);
    assert_ne!(
        fnv1a([b"ab".as_slice(), b"c"]),
        fnv1a([b"a".as_slice(), b"bc"])
    );
    assert!(default_ignored_apps().contains(&"com.1password.1password".to_string()));
}
