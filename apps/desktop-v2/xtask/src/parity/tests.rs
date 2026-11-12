use super::compare::{self, colour_eq, parse_key, Expect, Kind, Status};
use super::reference::{self, Landmark, RefState, Style};
use super::tree::{self, Element};
use super::*;

fn style(padding: [f32; 4], color: Option<[f32; 4]>, background: Option<[f32; 4]>) -> Style {
    Style {
        font_size: 14.0,
        font_weight: 500.0,
        radius: [Some(8.0); 4],
        padding,
        color,
        background,
        shadow: "none".into(),
    }
}

fn landmark(role: &str, name: &str, rect: [f32; 4], style: Style) -> Landmark {
    Landmark {
        role: role.into(),
        name: name.into(),
        occurrence: 1,
        rect,
        style,
    }
}

fn state(window: &str, scene: &str, theme: &str, landmarks: Vec<Landmark>) -> RefState {
    RefState {
        window: window.into(),
        scene: scene.into(),
        theme: theme.into(),
        file: format!("{window}/{scene}.{theme}.png"),
        viewport: (400.0, 300.0),
        landmarks,
    }
}

fn el(path: &str, role: &str, rect: [f32; 4]) -> Element {
    Element {
        path: path.into(),
        role: role.into(),
        key: None,
        label: None,
        rect,
        fill: None,
        color: None,
        text: None,
    }
}

fn opts(window: Option<&str>, scene: Option<&str>) -> Options {
    Options {
        window: window.map(Into::into),
        scene: scene.map(Into::into),
        themes: vec!["light".into(), "dark".into()],
        out: PathBuf::from("/nonexistent"),
        build: false,
        gate: true,
    }
}

fn listing(entries: &[(&str, &[&str])]) -> Listing {
    entries
        .iter()
        .map(|(w, s)| (w.to_string(), s.iter().map(|x| x.to_string()).collect()))
        .collect()
}

const INK: [f32; 4] = [12.0, 10.0, 9.0, 1.0];
const BLUE: [f32; 4] = [20.0, 71.0, 230.0, 1.0];

fn refs() -> Vec<RefState> {
    ["light", "dark"]
        .iter()
        .flat_map(|t| {
            [
                state("main", "library", t, vec![]),
                state("main", "settings-general", t, vec![]),
            ]
        })
        .collect()
}

fn failed(jobs: &[Job]) -> Vec<&Row> {
    jobs.iter()
        .filter_map(|j| if let Job::Row(r) = j { Some(r) } else { None })
        .filter(|r| r.failed())
        .collect()
}

// ---- Fail closed: the plan ----

#[test]
fn an_unbuilt_unmarked_reference_scene_fails() {
    let l = listing(&[("main", &["library"]), ("gallery", &["button"])]);
    let jobs = plan(&opts(None, None), &refs(), &l, &[]).unwrap();
    let f = failed(&jobs);
    assert_eq!(f.len(), 2, "settings-general in both themes");
    assert!(f.iter().all(|r| r.page == "main/settings-general"));
    assert!(jobs.contains(&Job::Screen {
        window: "main".into(),
        scene: "library".into(),
        theme: "dark".into()
    }));
}

#[test]
fn a_not_yet_built_mark_reports_its_step_and_does_not_fail() {
    let l = listing(&[("main", &[]), ("gallery", &["button"])]);
    let pending = [
        ("main", "library", "S50"),
        ("main", "settings-general", "S61"),
    ];
    let jobs = plan(&opts(None, None), &refs(), &l, &pending).unwrap();
    assert!(failed(&jobs).is_empty());
    let marked: Vec<&Row> = jobs
        .iter()
        .filter_map(|j| if let Job::Row(r) = j { Some(r) } else { None })
        .collect();
    assert_eq!(marked.len(), 4);
    assert!(marked.iter().all(|r| r.kind == Kind::Pending));
    assert!(marked.iter().any(|r| r.checks[0].expected == "S61"));
}

#[test]
fn a_stale_mark_on_a_built_scene_fails() {
    let l = listing(&[
        ("main", &["library", "settings-general"]),
        ("gallery", &["button"]),
    ]);
    let jobs = plan(
        &opts(None, None),
        &refs(),
        &l,
        &[("main", "library", "S50")],
    )
    .unwrap();
    let f = failed(&jobs);
    assert_eq!(f.len(), 2);
    assert!(f[0].checks[0].got.contains("still marked"));
}

#[test]
fn mistyped_filters_are_errors() {
    let l = listing(&[("main", &["library"]), ("gallery", &["button"])]);
    assert!(plan(&opts(Some("mian"), None), &refs(), &l, &[]).is_err());
    assert!(plan(&opts(None, Some("libary")), &refs(), &l, &[]).is_err());
    assert!(
        plan(&opts(Some("gallery"), Some("library")), &refs(), &l, &[]).is_err(),
        "a scene of another window"
    );
    assert!(plan(&opts(Some("gallery"), Some("button")), &refs(), &l, &[]).is_ok());
}

#[test]
fn empty_listings_are_errors() {
    assert!(
        plan(&opts(None, None), &refs(), &Listing::new(), &[]).is_err(),
        "no windows"
    );
    let no_gallery = listing(&[("main", &["library", "settings-general"])]);
    assert!(plan(&opts(None, None), &refs(), &no_gallery, &[]).is_err());
    let empty_gallery = listing(&[("main", &["library", "settings-general"]), ("gallery", &[])]);
    assert!(plan(&opts(None, None), &refs(), &empty_gallery, &[]).is_err());
}

#[test]
fn a_run_that_compares_nothing_is_an_error() {
    let l = listing(&[("main", &[]), ("gallery", &["button"])]);
    let pending = [
        ("main", "library", "S50"),
        ("main", "settings-general", "S61"),
    ];
    let err = plan(&opts(Some("main"), None), &refs(), &l, &pending).unwrap_err();
    assert!(err.to_string().contains("nothing to compare"));
}

#[test]
fn a_scene_without_a_reference_is_reported_not_dropped() {
    let l = listing(&[
        ("main", &["library", "settings-general", "scratch"]),
        ("gallery", &["button"]),
    ]);
    let jobs = plan(&opts(Some("main"), None), &refs(), &l, &[]).unwrap();
    let info: Vec<&Row> = jobs
        .iter()
        .filter_map(|j| if let Job::Row(r) = j { Some(r) } else { None })
        .collect();
    assert_eq!(info.len(), 2);
    assert!(info
        .iter()
        .all(|r| r.kind == Kind::Info && r.page == "main/scratch"));
}

#[test]
fn a_failing_command_is_an_error() {
    assert!(run_checked(&mut Command::new("false")).is_err());
    assert!(run_checked(&mut Command::new("/nonexistent/kudzu")).is_err());
    assert_eq!(
        run_checked(Command::new("printf").arg("a\n\nb\n"))
            .map(|s| lines(&s))
            .unwrap(),
        ["a", "b"]
    );
}

#[test]
fn options_reject_unknown_themes_and_arguments() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(options(&args(&["--theme", "sepia"])).is_err());
    assert!(options(&args(&["--frobnicate"])).is_err());
    assert_eq!(
        options(&args(&["--theme", "dark"])).unwrap().themes,
        ["dark"]
    );
}

// ---- Fail closed: the reference ----

#[test]
fn states_md_and_metrics_must_agree() {
    let md = vec![("main".to_string(), "library".to_string())];
    let both = vec![
        state("main", "library", "light", vec![]),
        state("main", "library", "dark", vec![]),
    ];
    assert!(reference::check_consistent(&md, &both).is_ok());
    assert!(
        reference::check_consistent(&md, &both[..1]).is_err(),
        "dark capture missing"
    );
    let extra = vec![
        ("main".to_string(), "library".to_string()),
        ("main".to_string(), "insights".to_string()),
    ];
    assert!(reference::check_consistent(&extra, &both).is_err());
}

#[test]
fn not_yet_built_marks_must_name_reference_scenes_once() {
    let r = refs();
    assert!(reference::check_pending(&[("main", "library", "S50")], &r).is_ok());
    assert!(reference::check_pending(&[("main", "libary", "S50")], &r).is_err());
    assert!(reference::check_pending(
        &[("main", "library", "S50"), ("main", "library", "S51")],
        &r
    )
    .is_err());
}

#[test]
fn malformed_metrics_are_errors() {
    assert!(reference::parse_metrics(r#"{"schemaVersion": 1, "states": []}"#).is_err());
    let bad_landmark = r#"{"schemaVersion": 2, "tables": {"styles": [], "colors": [], "shadows": []},
        "states": [{"window": "main", "scene": "x", "theme": "light", "file": "f", "viewport": {"width": 1, "height": 1},
        "landmarks": [["button", "A", 1, [0, 0, 1], 0]]}]}"#;
    assert!(reference::parse_metrics(bad_landmark).is_err());
}

#[test]
fn the_committed_reference_is_consistent_and_fully_accounted_for() {
    let dir = root().join("docs/migration/reference");
    let refs =
        reference::parse_metrics(&fs::read_to_string(dir.join("metrics.json")).unwrap()).unwrap();
    let md =
        reference::parse_states_md(&fs::read_to_string(dir.join("STATES.md")).unwrap()).unwrap();
    reference::check_consistent(&md, &refs).unwrap();
    reference::check_pending(reference::NOT_YET_BUILT, &refs).unwrap();
    assert_eq!((md.len(), refs.len()), (64, 128));
    let sync = refs
        .iter()
        .find(|r| r.scene == "settings-general" && r.theme == "light")
        .unwrap();
    let sync = sync
        .landmarks
        .iter()
        .find(|l| l.name == "Sync now")
        .unwrap();
    assert_eq!(
        (sync.rect[3], sync.style.padding, sync.style.font_size),
        (28.0, [0.0, 10.0, 0.0, 10.0], 12.8)
    );
}

// ---- Measuring ----

#[test]
fn keys_parse_into_expectations() {
    assert!(
        matches!(parse_key("parity:main:x:button:2:a:b"), Some(Expect::Reference { name, occurrence: 2, .. }) if name == "a:b")
    );
    assert_eq!(
        parse_key("spec:button/sm:_x28"),
        Some(Expect::Spec {
            what: "button/sm".into(),
            w: None,
            h: Some(28.0)
        })
    );
    assert_eq!(parse_key("info:menu"), Some(Expect::Info("menu".into())));
    assert_eq!(parse_key("spec:bad"), None);
}

#[test]
fn colours_compare_premultiplied() {
    assert!(colour_eq(
        Some([0.0, 0.0, 0.0, 0.024]),
        Some([12.0, 10.0, 9.0, 0.024])
    ));
    assert!(!colour_eq(Some(BLUE), Some([30.0, 71.0, 230.0, 1.0])));
    assert!(!colour_eq(None, Some([0.0, 0.0, 0.0, 0.5])));
}

#[test]
fn a_field_measures_as_its_box_with_the_editor_as_content() {
    let mut editor = el("#2/#0/in", "textbox", [21.0, 16.0, 170.0, 20.0]);
    editor.label = Some("Name".into());
    editor.color = Some(INK);
    let els = vec![el("#2", "textbox", [10.0, 10.0, 200.0, 32.0]), editor];
    let got = tree::landmarks(&els);
    assert_eq!(got.len(), 1);
    assert_eq!(
        (got[0].rect, got[0].content, got[0].ink),
        (
            [10.0, 10.0, 200.0, 32.0],
            Some([21.0, 16.0, 170.0, 20.0]),
            Some(INK)
        )
    );
}

fn button(text_x: f32, ink: [f32; 4]) -> Vec<Element> {
    let mut b = el("b", "button", [0.0, 0.0, 60.0, 28.0]);
    b.label = Some("Clear".into());
    let mut t = el("b/#0", "text", [text_x, 4.0, 38.0, 20.0]);
    t.text = Some("Clear".into());
    t.color = Some(ink);
    vec![b, t]
}

fn checks_of(els: &[Element], want: &Landmark) -> Vec<compare::Check> {
    compare::style_checks(want, &tree::landmarks(els)[0], false)
}

fn status(checks: &[compare::Check], prop: &str) -> Status {
    checks
        .iter()
        .find(|c| c.prop == prop)
        .unwrap_or_else(|| panic!("{prop} not checked"))
        .status
}

/// A pane with a muted badge first and body text after it.
fn pane(body_ink: [f32; 4]) -> Vec<Element> {
    let mut p = el("p", "complementary", [0.0, 0.0, 300.0, 200.0]);
    p.label = Some("Preview".into());
    let mut badge = el("p/#0", "text", [10.0, 10.0, 40.0, 18.0]);
    badge.text = Some("Text".into());
    badge.color = Some([120.0, 113.0, 107.0, 1.0]);
    let mut body = el("p/#1", "text", [10.0, 40.0, 200.0, 18.0]);
    body.text = Some("hello".into());
    // Body text is the pane's colour at 90%.
    body.color = Some([body_ink[0], body_ink[1], body_ink[2], 0.9]);
    vec![p, badge, body]
}

#[test]
fn a_preview_pane_takes_the_colour_its_unstyled_text_inherits() {
    let want = landmark(
        "complementary",
        "Preview",
        [0.0, 0.0, 300.0, 200.0],
        style([0.0; 4], Some(INK), None),
    );
    let ok = checks_of(&pane(INK), &want);
    assert_eq!(status(&ok, "text colour"), Status::Pass);
    let wrong = checks_of(&pane([1.0, 2.0, 3.0, 1.0]), &want);
    assert_eq!(status(&wrong, "text colour"), Status::Fail);
    // Any other role still measures its first text.
    let mut other = want.clone();
    other.role = "region".into();
    let mut els = pane(INK);
    els[0].role = "region".into();
    assert_eq!(
        status(&checks_of(&els, &other), "text colour"),
        Status::Fail
    );
}

#[test]
fn style_checks_gate_ink_and_padding_and_leave_the_rest_to_review() {
    let want = landmark(
        "button",
        "Clear",
        [0.0, 0.0, 60.0, 28.0],
        style([0.0, 10.0, 0.0, 10.0], Some(INK), None),
    );
    let ok = checks_of(&button(11.0, INK), &want);
    assert_eq!(status(&ok, "size"), Status::Pass);
    assert_eq!(status(&ok, "text colour"), Status::Pass);
    assert_eq!(
        (status(&ok, "padding left"), status(&ok, "padding right")),
        (Status::Pass, Status::Pass)
    );
    assert!(
        !ok.iter().any(|c| c.prop == "padding top"),
        "no padding: placed by alignment"
    );
    for prop in [
        "font size / weight",
        "radius",
        "border",
        "shadow",
        "focus ring",
    ] {
        assert_eq!(
            status(&ok, prop),
            Status::Review,
            "{prop} is not silently passed"
        );
    }
    assert_eq!(
        status(&checks_of(&button(11.0, BLUE), &want), "text colour"),
        Status::Fail
    );
    assert_eq!(
        status(&checks_of(&button(17.0, INK), &want), "padding left"),
        Status::Fail,
        "the leading side is exact"
    );
    assert_eq!(
        status(&checks_of(&button(4.0, INK), &want), "padding left"),
        Status::Fail,
        "content intrudes"
    );
}

#[test]
fn padding_beyond_the_floor_is_reviewed_when_alignment_or_size_explains_it() {
    let want = landmark(
        "textbox",
        "Notes",
        [0.0, 0.0, 296.0, 80.0],
        style([8.0, 10.0, 8.0, 10.0], Some(INK), None),
    );
    let mut b = el("t", "button", [0.0, 0.0, 296.0, 80.0]);
    b.label = Some("Notes".into());
    let mut t = el("t/#0", "text", [11.0, 9.0, 60.0, 40.0]);
    t.text = Some("Mac\nLinux".into());
    let c = checks_of(&[b.clone(), t], &want);
    assert_eq!(status(&c, "padding top"), Status::Pass);
    assert_eq!(
        status(&c, "padding bottom"),
        Status::Review,
        "a min-height box leaves room below"
    );
    let mut centred = el("t/#0", "text", [118.0, 30.0, 60.0, 20.0]);
    centred.text = Some("OK".into());
    assert_eq!(
        status(&checks_of(&[b, centred], &want), "padding left"),
        Status::Review,
        "centred"
    );
}

#[test]
fn a_switch_without_text_reviews_its_ink_and_knob() {
    let mut s = el("s", "switch", [0.0, 0.0, 32.0, 18.4]);
    s.label = Some("On".into());
    s.fill = Some(BLUE);
    let want = landmark(
        "switch",
        "On",
        [0.0, 0.0, 32.0, 18.39],
        style([0.0; 4], Some(INK), Some(BLUE)),
    );
    let c = checks_of(&[s], &want);
    assert_eq!(
        (status(&c, "size"), status(&c, "background")),
        (Status::Pass, Status::Pass)
    );
    assert_eq!(
        (status(&c, "text colour"), status(&c, "knob")),
        (Status::Review, Status::Review)
    );
}

#[test]
fn gallery_samples_check_child_landmarks_by_offset() {
    let parent = landmark(
        "dialog",
        "Group",
        [100.0, 100.0, 300.0, 200.0],
        style([0.0; 4], None, None),
    );
    let child = landmark(
        "button",
        "Save",
        [300.0, 260.0, 60.0, 28.0],
        style([0.0; 4], None, None),
    );
    let refs = vec![state("main", "group-dialog", "light", vec![parent, child])];
    let mut d = el("d", "dialog", [0.0, 0.0, 300.0, 200.0]);
    d.key = Some("parity:main:group-dialog:dialog:1:Group".into());
    let mut save = el("d/#0", "button", [200.0, 160.0, 60.0, 28.0]);
    save.label = Some("Save".into());
    let rows = compare::gallery_rows("dialog", "light", &[d.clone(), save.clone()], &refs);
    assert_eq!(status(&rows[0].checks, "child button “Save”"), Status::Pass);
    save.rect[0] = 190.0;
    let rows = compare::gallery_rows("dialog", "light", &[d.clone(), save], &refs);
    assert!(rows[0].failed(), "moved 10 pt");
    let rows = compare::gallery_rows("dialog", "light", &[d], &refs);
    assert!(rows[0].failed(), "child missing");
}

#[test]
fn gallery_scenes_without_samples_fail_unless_listed() {
    let rows = compare::gallery_rows("dialog", "light", &[el("x", "group", [0.0; 4])], &[]);
    assert!(rows[0].failed());
    let rows = compare::gallery_rows("tokens", "light", &[], &[]);
    assert_eq!((rows[0].kind, rows[0].failed()), (Kind::Info, false));
    let mut lost = el("x", "button", [0.0, 0.0, 10.0, 10.0]);
    lost.key = Some("parity:main:nowhere:button:1:X".into());
    assert!(compare::gallery_rows("button", "light", &[lost], &[])
        .iter()
        .all(Row::failed));
}

#[test]
fn screen_rows_fail_missing_landmarks_and_carry_crops() {
    let want = landmark(
        "button",
        "Clear",
        [0.0, 0.0, 60.0, 28.0],
        style([0.0, 10.0, 0.0, 10.0], Some(INK), None),
    );
    let gone = landmark(
        "button",
        "Gone",
        [0.0, 40.0, 60.0, 28.0],
        style([0.0; 4], None, None),
    );
    let st = state("main", "settings-activity", "light", vec![want, gone]);
    let mut extra = el("e", "button", [0.0, 80.0, 20.0, 20.0]);
    extra.label = Some("Extra".into());
    let mut els = button(11.0, INK);
    els.push(extra);
    let rows = compare::screen_rows(&st, &els);
    assert_eq!(rows.len(), 3);
    assert!(!rows[0].failed() && rows[1].failed());
    assert_eq!(rows[2].kind, Kind::Info);
    assert_eq!(
        rows[0].crop.as_ref().unwrap().rune,
        Some([0.0, 0.0, 60.0, 28.0])
    );
    assert_eq!(
        rows[1].crop.as_ref().unwrap().reference,
        Some([0.0, 40.0, 60.0, 28.0])
    );
}

#[test]
fn the_sheet_pairs_full_screens_and_crops_landmarks() {
    let pngs = std::env::temp_dir().join(format!("kudzu-parity-sheet-{}", std::process::id()));
    fs::create_dir_all(pngs.join("main")).unwrap();
    fs::write(pngs.join("main/settings-activity.light.png"), b"png").unwrap();
    let want = landmark(
        "button",
        "Clear",
        [0.0, 0.0, 60.0, 28.0],
        style([0.0; 4], None, None),
    );
    let st = state("main", "settings-activity", "light", vec![want]);
    let rows = compare::screen_rows(&st, &button(11.0, INK));
    let mut renders = BTreeMap::new();
    let render = |reference| report::Render {
        png: "/r/main.png".into(),
        viewport: (400.0, 300.0),
        reference,
    };
    renders.insert(
        "main/settings-activity.light".to_string(),
        render(Some((st.file.clone(), st.viewport))),
    );
    let viewports = BTreeMap::from([(st.file.clone(), st.viewport)]);
    let html = report::sheet(&rows, &pngs, &renders, &viewports);
    assert!(html.contains("main/settings-activity.light"));
    assert!(
        html.contains("settings-activity.light.png\"><img src=\"/r/main.png\">"),
        "full-screen pair"
    );
    assert_eq!(
        html.matches("class=c ").count(),
        2,
        "a reference crop and a Rune crop"
    );
    fs::remove_dir_all(&pngs).unwrap();
    let html = report::sheet(&rows, &pngs, &renders, &viewports);
    assert!(
        html.contains("reference PNG missing"),
        "a missing file is shown, not skipped"
    );
}

#[test]
fn the_report_counts_failures_pending_and_reviews() {
    let want = landmark(
        "button",
        "Clear",
        [0.0, 0.0, 60.0, 28.0],
        style([0.0; 4], Some(INK), None),
    );
    let st = state("main", "settings-activity", "light", vec![want]);
    let rows = compare::screen_rows(&st, &button(11.0, BLUE));
    let md = report::markdown(&rows);
    assert!(md.contains("1 rows: 1 failed"));
    assert!(md.contains("**FAIL**") && md.contains("text colour"));
    assert_eq!(report::json(&rows)[0]["verdict"], "FAIL");
}

fn permission_card_els(
    root: &str,
    label: &str,
    y: f32,
    selected: bool,
    granted: bool,
    badge: &str,
) -> Vec<Element> {
    let success = [0.0, 153.0, 86.0, 1.0];
    let muted = [121.0, 113.0, 107.0, 1.0];
    let mut card = el(root, "button", [41.0, y, 338.0, 62.5]);
    card.label = Some(label.into());
    card.fill = Some(if selected {
        [20.0, 69.0, 226.0, 0.051]
    } else {
        [255.0, 255.0, 255.0, 0.5]
    });

    let mut media = el(
        &format!("{root}/0"),
        "container",
        [56.0, y + 15.25, 32.0, 32.0],
    );
    media.fill = Some(if granted {
        [0.0, 153.0, 86.0, 0.1]
    } else {
        [12.0, 10.0, 9.0, 0.05]
    });

    let mut glyph = el(
        &format!("{root}/0/0"),
        "icon",
        [64.0, y + 23.25, 16.0, 16.0],
    );
    glyph.color = Some(if granted { success } else { muted });

    let mut title = el(
        &format!("{root}/1/0"),
        "text",
        [100.0, y + 13.0, 90.0, 18.0],
    );
    title.text = Some(label.into());
    title.color = Some(INK);

    let mut out = vec![card, media, glyph, title];
    if granted {
        let mut check_icon = el(
            &format!("{root}/2/0"),
            "icon",
            [300.0, y + 24.0, 14.0, 14.0],
        );
        check_icon.color = Some(success);
        out.push(check_icon);
    }
    let mut badge_el = el(
        &format!("{root}/2/1"),
        "text",
        [318.0, y + 24.0, 44.0, 14.0],
    );
    badge_el.text = Some(badge.into());
    badge_el.color = Some(if granted { success } else { muted });
    out.push(badge_el);
    out
}

#[test]
fn permissions_row_landmarks_are_gated_and_catch_wrong_state() {
    let st = state("permissions", "allowed", "light", vec![]);

    // Valid `allowed` scene: Accessibility granted ("Allowed"), Input Monitoring ungranted ("Optional").
    let mut ok_els = permission_card_els("0", "Accessibility", 274.0, true, true, "Allowed");
    ok_els.extend(permission_card_els(
        "1",
        "Input Monitoring",
        344.5,
        false,
        false,
        "Optional",
    ));
    let ok_rows = compare::screen_rows(&st, &ok_els);
    assert_eq!(ok_rows.len(), 2);
    assert!(
        ok_rows.iter().all(|r| r.kind == Kind::Gated && !r.failed()),
        "both permission row cards must be Gated and pass when matching mock.ts: {ok_rows:?}"
    );

    // Wrong `Input Monitoring` state (granted = true / "Allowed" in `allowed` scene) MUST fail gated parity!
    let mut bad_els = permission_card_els("0", "Accessibility", 274.0, true, true, "Allowed");
    bad_els.extend(permission_card_els(
        "1",
        "Input Monitoring",
        344.5,
        false,
        true,
        "Allowed",
    ));
    let bad_rows = compare::screen_rows(&st, &bad_els);
    let im_row = bad_rows
        .iter()
        .find(|r| r.item == "button “Input Monitoring” #1")
        .expect("Input Monitoring row must exist");
    assert_eq!(im_row.kind, Kind::Gated);
    assert!(
        im_row.failed(),
        "wrong Input Monitoring grant state must fail gated parity"
    );
}
