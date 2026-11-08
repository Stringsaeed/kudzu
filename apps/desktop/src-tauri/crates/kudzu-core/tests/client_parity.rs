//! `kudzu_core::client` reproduces the web UI's TypeScript exactly: every expectation here was
//! produced by running `apps/desktop/lib` itself (see `fixtures/client/generate.mjs`), in
//! America/New_York with the clock fixed at 2026-11-08 12:30 PM.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday};
use kudzu_core::client::{clipboard as clip, formatting, groups, js, keys, search, stats, template, updates};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use kudzu_core::formatting::{FormatSpan, FormatStyle};
use kudzu_core::template::AttributeValue;
use kudzu_core::KudzuLibrary;
use serde_json::Value;
use uuid::Uuid;

/// US Eastern time (the rules in force since 2007), standing in for the tz database.
#[derive(Debug, Clone, Copy)]
struct Eastern;

fn nth_sunday(year: i32, month: u32, n: u8) -> NaiveDate {
    NaiveDate::from_weekday_of_month_opt(year, month, Weekday::Sun, n).unwrap()
}

impl Eastern {
    fn is_dst_utc(utc: &NaiveDateTime) -> bool {
        let y = utc.year();
        let start = nth_sunday(y, 3, 2).and_hms_opt(7, 0, 0).unwrap();
        let end = nth_sunday(y, 11, 1).and_hms_opt(6, 0, 0).unwrap();
        *utc >= start && *utc < end
    }
    fn offset(dst: bool) -> FixedOffset {
        FixedOffset::west_opt(if dst { 4 * 3600 } else { 5 * 3600 }).unwrap()
    }
}

impl TimeZone for Eastern {
    type Offset = FixedOffset;
    fn from_offset(_: &FixedOffset) -> Self {
        Eastern
    }
    fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<FixedOffset> {
        self.offset_from_local_datetime(&local.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
        let fits = |dst: bool| {
            let off = Self::offset(dst);
            let utc = *local - Duration::seconds(i64::from(off.local_minus_utc()));
            (Self::is_dst_utc(&utc) == dst).then_some(off)
        };
        match (fits(true), fits(false)) {
            (Some(a), Some(b)) => LocalResult::Ambiguous(a, b),
            (Some(a), None) | (None, Some(a)) => LocalResult::Single(a),
            (None, None) => LocalResult::None,
        }
    }
    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&utc.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        Self::offset(Self::is_dst_utc(utc))
    }
}

struct Fixture {
    v: Value,
    now: i64,
    library: KudzuLibrary,
}

fn fixture() -> Fixture {
    let raw = include_str!("fixtures/client/parity.json");
    let v: Value = serde_json::from_str(raw).unwrap();
    let library = serde_json::from_value(v["library"].clone()).unwrap();
    Fixture { now: v["now"].as_i64().unwrap(), library, v }
}

fn arr(v: &Value) -> &Vec<Value> {
    v.as_array().unwrap_or_else(|| panic!("not an array: {v}"))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn id(v: &Value) -> Uuid {
    s(v).parse().unwrap()
}

fn ids(v: &Value) -> Vec<Uuid> {
    arr(v).iter().map(id).collect()
}

fn strings(v: &Value) -> Vec<String> {
    arr(v).iter().map(|x| s(x).to_string()).collect()
}

fn field_name(f: Option<search::MatchField>) -> Value {
    match f {
        None => Value::Null,
        Some(search::MatchField::Name) => "name".into(),
        Some(search::MatchField::Trigger) => "trigger".into(),
        Some(search::MatchField::Content) => "content".into(),
    }
}

#[test]
fn search_ranks_like_the_web_ui() {
    let f = fixture();
    let usage = stats::usage_by_id(&f.library);
    let mut cache = search::SearchCache::default();
    let index = cache.build_index(&f.library.snippets, &usage);
    for case in arr(&f.v["searchCases"]) {
        let q = s(&case["query"]);
        let hits = search::search(&index, q, case["limit"].as_u64().unwrap() as usize, f.now);
        let want = arr(&case["hits"]);
        let got: Vec<Uuid> = hits.iter().map(|h| h.snippet.id).collect();
        assert_eq!(got, want.iter().map(|h| id(&h[0])).collect::<Vec<_>>(), "order for {q:?}");
        for (h, w) in hits.iter().zip(want) {
            assert!((h.score - w[1].as_f64().unwrap()).abs() < 1e-9, "score of {} for {q:?}: {} vs {}", h.snippet.id, h.score, w[1]);
            assert_eq!(field_name(h.field), w[2], "field of {} for {q:?}", h.snippet.id);
            let hits: Vec<u64> = arr(&w[3]).iter().map(|x| x.as_u64().unwrap()).collect();
            assert_eq!(h.name_hits.iter().map(|&x| x as u64).collect::<Vec<_>>(), hits, "name hits of {} for {q:?}", h.snippet.id);
        }
    }
    // A rebuild reuses folded fields untouched by usage-only updates.
    let again = cache.build_index(&f.library.snippets, &HashMap::new());
    assert!(std::sync::Arc::ptr_eq(&index[0].folded, &again[0].folded));
}

#[test]
fn palette_filters_scopes_and_orders_like_the_web_ui() {
    let f = fixture();
    let usage = stats::usage_by_id(&f.library);
    let mut cache = search::SearchCache::default();
    for case in arr(&f.v["paletteCases"]) {
        let q = s(&case["query"]);
        let scope = match s(&case["scope"]) {
            "all" => search::PaletteScope::All,
            "favorites" => search::PaletteScope::Favorites,
            _ => search::PaletteScope::Recent,
        };
        let parsed = search::parse_palette_query(q);
        assert_eq!(parsed.text, s(&case["text"]), "text of {q:?}");
        assert_eq!(parsed.group_filter.as_deref(), case["groupFilter"].as_str(), "group filter of {q:?}");
        let hits = search::palette_hits(&f.library, q, scope, &usage, &mut cache, f.now);
        assert_eq!(hits.iter().map(|h| h.snippet.id).collect::<Vec<_>>(), ids(&case["ids"]), "{q:?} in {scope:?}");
        assert_eq!(search::palette_offers_create(q, &hits), case["createRow"].as_bool().unwrap(), "create row for {q:?}");
    }
}

#[test]
fn library_list_views_and_sorts_like_the_web_ui() {
    let f = fixture();
    let usage = stats::usage_by_id(&f.library);
    let mut cache = search::SearchCache::default();
    for case in arr(&f.v["listCases"]) {
        let view = match s(&case["view"]["kind"]) {
            "all" => search::ListView::All,
            "favorites" => search::ListView::Favorites,
            "recent" => search::ListView::Recent,
            "disabled" => search::ListView::Disabled,
            _ => search::ListView::Group(id(&case["view"]["id"])),
        };
        let sort = match s(&case["sort"]) {
            "relevance" => search::ListSort::Relevance,
            "name" => search::ListSort::Name,
            "edited" => search::ListSort::Edited,
            _ => search::ListSort::Used,
        };
        let filter = s(&case["filter"]);
        let hits = search::list_hits(&f.library, view, filter, sort, &usage, &mut cache, f.now);
        assert_eq!(hits.iter().map(|h| h.snippet.id).collect::<Vec<_>>(), ids(&case["ids"]), "{view:?} {filter:?} {sort:?}");
    }
}

#[test]
fn group_helpers_match_the_web_ui() {
    let f = fixture();
    let g = &f.v["groupCases"];
    let gs = &f.library.groups;
    let tree = groups::build_tree(gs);
    let rows: Vec<(Uuid, u64)> = groups::flatten(&tree, None).iter().map(|n| (n.group.id, n.depth as u64)).collect();
    assert_eq!(rows, arr(&g["tree"]).iter().map(|r| (id(&r[0]), r[1].as_u64().unwrap())).collect::<Vec<_>>());
    let collapsed = HashSet::from([id(&Value::from("00000000-0000-4000-8000-000000002329"))]);
    assert_eq!(groups::flatten(&tree, Some(&collapsed)).iter().map(|n| n.group.id).collect::<Vec<_>>(), ids(&g["collapsed"]));
    let counts = groups::recursive_counts(gs, &f.library.snippets);
    let want: HashMap<Uuid, usize> = g["counts"].as_object().unwrap().iter().map(|(k, v)| (k.parse().unwrap(), v.as_u64().unwrap() as usize)).collect();
    assert_eq!(counts, want);
    for p in arr(&g["paths"]) {
        assert_eq!(groups::group_path(gs, Some(id(&p[0]))), strings(&p[1]));
    }
    for e in arr(&g["enabled"]) {
        assert_eq!(groups::is_group_enabled(gs, Some(id(&e[0]))), e[1].as_bool().unwrap());
    }
    for d in arr(&g["descendants"]) {
        let mut got: Vec<String> = groups::descendant_ids(gs, id(&d[0])).iter().map(Uuid::to_string).collect();
        got.sort();
        assert_eq!(got, strings(&d[1]));
    }
}

#[test]
fn stats_match_the_web_ui() {
    let f = fixture();
    let st = &f.v["statsCases"];
    let usage = stats::usage_by_id(&f.library);
    let want = st["usage"].as_object().unwrap();
    assert_eq!(usage.len(), want.len());
    for (k, v) in want {
        let u = usage[&k.parse::<Uuid>().unwrap()];
        assert_eq!((u.count, u.chars, u.last), (v["count"].as_i64().unwrap(), v["chars"].as_i64().unwrap(), v["last"].as_i64().unwrap()), "{k}");
    }
    let t = stats::totals(&f.library, &usage, 45.0);
    let w = &st["totals"];
    assert_eq!((t.expansions, t.chars, t.active_snippets as u64, t.used_snippets as u64), (
        w["expansions"].as_i64().unwrap(),
        w["chars"].as_i64().unwrap(),
        w["activeSnippets"].as_u64().unwrap(),
        w["usedSnippets"].as_u64().unwrap()
    ));
    assert!((t.minutes_saved - w["minutesSaved"].as_f64().unwrap()).abs() < 1e-9);
    for d in arr(&st["durations"]) {
        assert_eq!(stats::format_duration(d[0].as_f64().unwrap()), s(&d[1]), "{}", d[0]);
    }
    for c in arr(&st["counts"]) {
        assert_eq!(stats::format_count(c[0].as_i64().unwrap()), s(&c[1]), "{}", c[0]);
    }
    for r in arr(&st["relative"]) {
        let secs = r[0].as_i64().unwrap();
        let ms = if secs == 0 { 0 } else { f.now - secs * 1000 };
        assert_eq!(stats::relative_time(ms, f.now, &Eastern), s(&r[1]), "{secs}s ago");
    }
}

fn attr_value(v: &Value) -> AttributeValue {
    match v {
        Value::String(x) => AttributeValue::String(x.clone()),
        Value::Bool(b) => AttributeValue::Bool(*b),
        Value::Number(n) => AttributeValue::Number(n.as_f64().unwrap()),
        other => panic!("unexpected attribute {other}"),
    }
}

fn tone_name(t: template::ChipTone) -> &'static str {
    match t {
        template::ChipTone::Time => "time",
        template::ChipTone::Clipboard => "clipboard",
        template::ChipTone::Cursor => "cursor",
        template::ChipTone::Snippet => "snippet",
        template::ChipTone::Form => "form",
        template::ChipTone::Error => "error",
    }
}

#[test]
fn template_tokens_labels_and_summaries_match_the_web_ui() {
    let f = fixture();
    let first = "00000000-0000-4000-8000-000000000001";
    let names = |id: &str| (id == first).then(|| "First snippet".to_string());
    for case in arr(&f.v["templateCases"]) {
        let source = s(&case["source"]);
        let tokens = template::tokenize(source);
        let want = arr(&case["tokens"]);
        assert_eq!(tokens.len(), want.len(), "{source:?}");
        for (t, w) in tokens.iter().zip(want) {
            match t {
                template::Token::Text(t) => assert_eq!((Value::from("text"), Value::from(t.from), Value::from(t.to), Value::from(t.text.as_str())), (w[0].clone(), w[1].clone(), w[2].clone(), w[3].clone()), "{source:?}"),
                template::Token::Tag(t) => {
                    assert_eq!((s(&w[0]), t.from as u64, t.to as u64, t.source.as_str(), t.name.as_str(), t.verbatim), ("tag", w[1].as_u64().unwrap(), w[2].as_u64().unwrap(), s(&w[3]), s(&w[4]), w[6].as_bool().unwrap()), "{source:?}");
                    let attrs: Vec<(String, AttributeValue)> = t.attrs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
                    let want: Vec<(String, AttributeValue)> = arr(&w[5]).iter().map(|e| (s(&e[0]).to_string(), attr_value(&e[1]))).collect();
                    assert_eq!(attrs, want, "{source:?}");
                }
            }
        }
        let labels: Vec<(String, String)> = tokens
            .iter()
            .filter_map(|t| match t {
                template::Token::Tag(t) => Some((template::chip_label(t, Some(&names)), tone_name(template::chip_tone(t)).to_string())),
                _ => None,
            })
            .collect();
        assert_eq!(labels, arr(&case["labels"]).iter().map(|l| (s(&l[0]).to_string(), s(&l[1]).to_string())).collect::<Vec<_>>(), "{source:?}");
        let facts = template::analyze(source);
        let a = &case["analyze"];
        assert_eq!(
            (facts.is_form, facts.field_names, facts.uses_clipboard, facts.has_cursor, facts.snippet_ids, facts.has_errors),
            (a["isForm"].as_bool().unwrap(), strings(&a["fieldNames"]), a["usesClipboard"].as_bool().unwrap(), a["hasCursor"].as_bool().unwrap(), strings(&a["snippetIds"]), a["hasErrors"].as_bool().unwrap()),
            "{source:?}"
        );
        assert_eq!(template::plain_summary(source, Some(&names), template::SUMMARY_MAX), s(&case["summary"]), "{source:?}");
        assert_eq!(template::plain_summary(source, None, 10), s(&case["summary10"]), "{source:?}");
    }
    for case in arr(&f.v["buildTagCases"]) {
        let attrs: Vec<(String, Option<AttributeValue>)> = arr(&case[1]).iter().map(|e| (s(&e[0]).to_string(), (!e[1].is_null()).then(|| attr_value(&e[1])))).collect();
        assert_eq!(template::build_tag(s(&case[0]), attrs), s(&case[2]));
    }
}

#[test]
fn formatting_matches_the_web_ui() {
    let f = fixture();
    for case in arr(&f.v["formattingCases"]) {
        let text = s(&case["text"]);
        let spans: Vec<FormatSpan> = serde_json::from_value(case["spans"].clone()).unwrap();
        let cursor = case["cursor"].as_u64().map(|c| c as usize);
        let valid: Vec<FormatSpan> = serde_json::from_value(case["valid"].clone()).unwrap();
        assert_eq!(formatting::valid_spans(text, &spans), valid, "{text:?}");
        let parts = formatting::formatted_parts(text, &spans, cursor);
        let want = arr(&case["parts"]);
        assert_eq!(parts.len(), want.len(), "{text:?}");
        for (p, w) in parts.iter().zip(want) {
            let image: Option<FormatSpan> = serde_json::from_value(w[2].clone()).unwrap();
            let styles: Vec<FormatStyle> = serde_json::from_value(w[3].clone()).unwrap();
            assert_eq!((p.start as u64, p.text.as_str(), &p.image, &p.styles), (w[0].as_u64().unwrap(), s(&w[1]), &image, &styles), "{text:?}");
        }
        assert_eq!(formatting::formatted_html(text, &spans), s(&case["html"]), "{text:?}");
    }
    for l in arr(&f.v["links"]) {
        assert_eq!(formatting::safe_link(s(&l[0])), l[1].as_bool().unwrap(), "{:?}", l[0]);
    }
}

#[test]
fn key_labels_match_the_web_ui_on_a_mac() {
    let f = fixture();
    for c in arr(&f.v["keyCases"]["labels"]) {
        assert_eq!(keys::key_labels(s(&c[0]), true), strings(&c[1]), "{}", c[0]);
    }
    for c in arr(&f.v["keyCases"]["accelerators"]) {
        assert_eq!(keys::accelerator_labels(s(&c[0]), true), strings(&c[1]), "{}", c[0]);
    }
}

fn clip_filter(v: &str) -> clip::ClipFilter {
    match v {
        "all" => clip::ClipFilter::All,
        "pinned" => clip::ClipFilter::Pinned,
        kind => clip::ClipFilter::Kind(serde_json::from_value(Value::from(kind)).unwrap()),
    }
}

#[test]
fn clipboard_search_rows_and_text_match_the_web_ui() {
    let f = fixture();
    let c = &f.v["clipCases"];
    let entries: Vec<ClipEntry> = serde_json::from_value(c["entries"].clone()).unwrap();
    for p in arr(&c["parse"]) {
        let q = clip::parse_query(s(&p[0]));
        let kind: Option<ClipKind> = serde_json::from_value(p[1]["kind"].clone()).unwrap();
        assert_eq!((q.terms, q.app.as_deref(), q.kind), (strings(&p[1]["terms"]), p[1]["app"].as_str(), kind), "{}", p[0]);
    }
    let mut cache = clip::HaystackCache::default();
    for case in arr(&c["search"]) {
        let (q, filter) = (s(&case[0]), clip_filter(s(&case[1])));
        let want = ids(&case[2]);
        assert_eq!(clip::search_clips(&entries, filter, q).iter().map(|e| e.id).collect::<Vec<_>>(), want, "{q:?} under {filter:?}");
        assert_eq!(clip::search_clips_cached(&entries, filter, q, &mut cache).iter().map(|e| e.id).collect::<Vec<_>>(), want);
    }
    for (e, d) in entries.iter().zip(arr(&c["describe"])) {
        let desc = clip::describe(e);
        let got = (
            desc.title,
            desc.subtitle,
            clip::spoken_label(e, false, f.now, &Eastern),
            clip::spoken_label(e, true, f.now, &Eastern),
            clip::clip_time(&e.copied_at, f.now, false, &Eastern),
            clip::clip_time(&e.copied_at, f.now, true, &Eastern),
            clip::exact_time(&e.copied_at, f.now, &Eastern),
            clip::expires_in(e, 7, f.now),
            clip::expires_in(e, 1, f.now),
            clip::secret_reason(e),
        );
        let want = (
            s(&d[1]["title"]).to_string(),
            s(&d[1]["subtitle"]).to_string(),
            s(&d[2]).to_string(),
            s(&d[3]).to_string(),
            s(&d[4]).to_string(),
            s(&d[5]).to_string(),
            s(&d[6]).to_string(),
            s(&d[7]).to_string(),
            s(&d[8]).to_string(),
            s(&d[9]).to_string(),
        );
        assert_eq!(got, want, "entry {}", e.id);
        assert_eq!((clip::is_secret(e), clip::has_text(e), clip::image_format(e)), (d[10].as_bool().unwrap(), d[11].as_bool().unwrap(), s(&d[12])), "entry {}", e.id);
    }
    let refs: Vec<&ClipEntry> = entries.iter().collect();
    for r in arr(&c["rows"]) {
        let opts = clip::RowOptions { pinned_first: r[0].as_bool().unwrap(), sections: r[1].as_bool().unwrap() };
        let (rows, order) = clip::build_rows(&refs, opts, f.now, &Eastern);
        let got: Vec<Value> = rows
            .iter()
            .map(|row| match row {
                clip::ClipRow::Header { key, label, count } => serde_json::json!(["header", key, label, count]),
                clip::ClipRow::Entry { key, index, dated, .. } => serde_json::json!(["entry", key, index, dated]),
            })
            .collect();
        assert_eq!(&got, arr(&r[2]), "{opts:?}");
        assert_eq!(order.len(), entries.len());
    }
    for r in arr(&c["ranges"]) {
        let got: Vec<Value> = clip::match_ranges(s(&r[0]), &strings(&r[1])).into_iter().map(|(a, b)| serde_json::json!([a, b])).collect();
        assert_eq!(&got, arr(&r[2]), "{}", r[0]);
    }
    for b in arr(&c["bytes"]) {
        assert_eq!(clip::format_bytes(b[0].as_f64().unwrap() as u64), s(&b[1]));
    }
    for r in arr(&c["retention"]) {
        assert_eq!(clip::retention_label(r[0].as_u64().unwrap() as u32), s(&r[1]));
    }
    for w in arr(&c["words"]) {
        assert_eq!(clip::word_count(s(&w[0])) as u64, w[1].as_u64().unwrap(), "{:?}", w[0]);
    }
    for d in arr(&c["days"]) {
        let ms = f.now - d[0].as_i64().unwrap() * 3_600_000;
        assert_eq!(clip::day_label(ms, f.now, &Eastern), s(&d[1]), "{}h ago", d[0]);
    }
}

#[test]
fn release_notes_match_the_web_ui() {
    let f = fixture();
    for case in arr(&f.v["notesCases"]) {
        let got: Vec<Value> = updates::parse_notes(case[0].as_str())
            .into_iter()
            .map(|n| match n.heading {
                Some(h) => serde_json::json!({ "heading": h, "items": n.items }),
                None => serde_json::json!({ "items": n.items }),
            })
            .collect();
        assert_eq!(&got, arr(&case[1]), "{:?}", case[0]);
    }
}

#[test]
fn to_fixed_matches_javascript() {
    let f = fixture();
    for c in arr(&f.v["fixed"]) {
        let x = c[0].as_f64().unwrap();
        for d in 0..3 {
            assert_eq!(js::to_fixed(x, d), s(&c[d + 1]), "{x}.toFixed({d})");
        }
    }
}

#[test]
fn eastern_time_zone_helper_is_sane() {
    let summer: DateTime<Utc> = "2026-07-01T16:00:00Z".parse().unwrap();
    let winter: DateTime<Utc> = "2026-12-01T17:00:00Z".parse().unwrap();
    assert_eq!(js::time_short(&summer.with_timezone(&Eastern)), "12:00 PM");
    assert_eq!(js::time_short(&winter.with_timezone(&Eastern)), "12:00 PM");
}
