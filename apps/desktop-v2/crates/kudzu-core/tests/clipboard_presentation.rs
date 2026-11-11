use chrono::{DateTime, Utc};
use kudzu_core::clipboard::{facts::*, query::*, sections::*, ClipEntry};
use serde_json::{json, Value};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/migration/reference/fixtures/clipboard.json"
    ))
    .unwrap()
}
fn entries(f: &Value) -> Vec<ClipEntry> {
    serde_json::from_value(f["rows"][0]["result"]["order"].clone()).unwrap()
}
fn now() -> DateTime<Utc> {
    "2026-11-11T12:00:00Z".parse().unwrap()
}
#[test]
fn reference_queries_filters_and_ranges() {
    let f = fixture();
    let entries = entries(&f);
    let index = ClipSearch::new(&entries);
    for c in f["queries"].as_array().unwrap() {
        let q = ClipQuery::parse(c["query"].as_str().unwrap());
        assert_eq!(json!(q), c["parsed"]);
        for filter in c["filters"].as_array().unwrap() {
            let kind = serde_json::from_value(filter["filter"].clone()).unwrap();
            let ids: Vec<_> = index
                .search(kind, &q)
                .iter()
                .map(|e| e.id.to_string())
                .collect();
            assert_eq!(json!(ids), filter["ids"]);
        }
    }
    for c in f["ranges"].as_array().unwrap() {
        let terms: Vec<String> = serde_json::from_value(c["terms"].clone()).unwrap();
        assert_eq!(
            json!(match_ranges(c["text"].as_str().unwrap(), &terms)),
            c["ranges"]
        );
    }
}
#[test]
fn reference_sections_and_labels() {
    let f = fixture();
    let entries = entries(&f);
    for c in f["rows"].as_array().unwrap() {
        let actual = build_rows(
            &entries,
            RowOptions {
                pinned_first: c["pinnedFirst"].as_bool().unwrap(),
                sections: c["sections"].as_bool().unwrap(),
            },
            now(),
        );
        let expected_order: Vec<ClipEntry> =
            serde_json::from_value(c["result"]["order"].clone()).unwrap();
        assert_eq!(actual.order, expected_order.iter().collect::<Vec<_>>());
        let expected_rows = c["result"]["rows"].as_array().unwrap();
        assert_eq!(actual.rows.len(), expected_rows.len());
        for (row, expected) in actual.rows.iter().zip(expected_rows) {
            let mut expected = expected.clone();
            if let Some(entry) = expected.get_mut("entry") {
                let typed: ClipEntry = serde_json::from_value(entry.clone()).unwrap();
                *entry = json!(typed);
            }
            assert_eq!(json!(row), expected);
        }
    }
    for (e, c) in entries.iter().zip(f["entries"].as_array().unwrap()) {
        assert_eq!(json!(e.id), c["id"]);
        assert_eq!(json!(describe(e)), c["description"]);
        assert_eq!(spoken_label(e, false, now()), c["spoken"]);
        assert_eq!(spoken_label(e, true, now()), c["concealed"]);
        assert_eq!(clip_time(e.copied_at, now(), false), c["time"]);
        assert_eq!(exact_time(e.copied_at, now()), c["exact"]);
        for x in c["expiry"].as_array().unwrap() {
            let days = x["days"].as_u64().unwrap() as u32;
            assert_eq!(expires_in(e, days, now()), x["output"]);
            assert_eq!(retention_label(days), x["label"]);
        }
    }
    for c in f["bytes"].as_array().unwrap() {
        assert_eq!(format_bytes(c["n"].as_u64().unwrap()), c["label"]);
    }
}
#[test]
fn search_5000_p95_under_five_ms() {
    let f = fixture();
    let base = entries(&f);
    let entries: Vec<_> = (0..5000).map(|i| base[i % base.len()].clone()).collect();
    let index = ClipSearch::new(&entries);
    let queries = [
        "design Thursday",
        "app:safari",
        "is:image",
        "missing",
        "",
        "is:code const",
    ]
    .map(ClipQuery::parse);
    let mut times = Vec::new();
    for _ in 0..100 {
        for q in &queries {
            let start = std::time::Instant::now();
            std::hint::black_box(index.search(ClipFilter::All, q));
            times.push(start.elapsed());
        }
    }
    times.sort();
    let p95 = times[times.len() * 95 / 100];
    eprintln!("5000 clips, 600 cached searches: p95={p95:?}");
    assert!(p95 <= std::time::Duration::from_millis(5), "p95={p95:?}");
}

#[test]
fn boundary_rules_and_presentation_policies() {
    let mut e = entries(&fixture()).remove(0);
    for (alias, kind) in [
        ("text", "text"),
        ("code", "code"),
        ("link", "link"),
        ("links", "link"),
        ("url", "link"),
        ("color", "color"),
        ("colour", "color"),
        ("image", "image"),
        ("images", "image"),
        ("img", "image"),
        ("file", "file"),
        ("files", "file"),
        ("other", "other"),
    ] {
        assert_eq!(
            json!(ClipQuery::parse(&format!("IS:{alias}")).kind),
            json!(kind)
        );
    }
    assert_eq!(
        ClipQuery::parse("app:first APP:last app: is:img is:bad").app,
        Some(String::new())
    );
    assert_eq!(
        ClipQuery::parse("\u{feff}alpha\u{a0}beta\u{85}gamma").terms,
        ["alpha", "beta\u{85}gamma"]
    );
    assert!(RowOptions::history(" ", ClipFilter::All).pinned_first);
    assert!(!RowOptions::history("app:", ClipFilter::All).pinned_first);
    assert!(!RowOptions::history("", ClipFilter::Pinned).pinned_first);
    assert!(!RowOptions::picker("").pinned_first);
    assert!(!RowOptions::picker("word").sections);
    assert_eq!(
        match_ranges("abcabc", &["bc".into(), "ca".into()]),
        [(1, 6)]
    );
    assert_eq!(match_ranges("abc", &[String::new()]), []);
    assert_eq!(word_count("\u{feff}one\n two\u{a0}three"), 3);
    e.text_len = 1234;
    e.line_count = 2;
    assert_eq!(text_facts(&e, None), "1,234 characters · 2 lines");
    assert_eq!(
        text_facts(&e, Some("one two")),
        "1,234 characters · 2 words · 2 lines"
    );
    assert_eq!(basename("/"), "");
    assert_eq!(dirname("a"), "/");
    assert_eq!(tildify("/Users/me/Desktop/a"), "~/Desktop/a");
    for (days, label) in [
        (30, "for 1 month"),
        (90, "for 3 months"),
        (14, "for 2 weeks"),
        (730, "for 2 years"),
        (2, "for 2 days"),
    ] {
        assert_eq!(retention_label(days), label);
    }
    e.copied_at = now() - chrono::Duration::seconds(45);
    assert_eq!(clip_time(e.copied_at, now(), false), "1m ago");
    e.copied_at = now() - chrono::Duration::days(1) + chrono::Duration::hours(1);
    assert_eq!(expires_in(&e, 1, now()), "Within the hour");
    e.pinned = true;
    assert_eq!(expires_in(&e, 0, now()), "Never (pinned)");
    let previous_year = "2025-11-11T12:00:00Z".parse().unwrap();
    assert_eq!(day_label(previous_year, &now()), "Tue, Nov 11, 2025");
    let offset = chrono::FixedOffset::east_opt(4 * 3600).unwrap();
    let local_now = now().with_timezone(&offset);
    let previous_utc_day = "2026-11-10T22:00:00Z".parse().unwrap();
    assert_eq!(day_label(previous_utc_day, &local_now), "Today");
    e.kind = kudzu_core::clipboard::ClipKind::Link;
    for (url, title, subtitle) in [
        ("www.example.com/", "example.com", "www.example.com/"),
        (
            "https://WWW.example.com:443/caf%C3%A9?q=%2F#anchor",
            "example.com",
            "/café?q=%2F",
        ),
        ("https://example.com/%zz", "https://example.com/%zz", ""),
    ] {
        e.text = Some(url.into());
        let d = describe(&e);
        assert_eq!(d.title, title);
        assert_eq!(d.subtitle, subtitle);
    }
}

#[test]
fn link_descriptions_normalize_url_dot_segments() {
    let mut e = entries(&fixture()).remove(1);
    for (url, subtitle) in [
        ("https://example.com/a/../b", "/b"),
        ("https://example.com/a/%2e%2e/b?x=1", "/b?x=1"),
        ("https://example.com/a/./", "/a"),
        ("https://example.com/a//b", "/a//b"),
    ] {
        e.text = Some(url.into());
        assert_eq!(describe(&e).subtitle, subtitle);
    }
}
