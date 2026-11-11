use super::*;

#[test]
fn keys_parse_into_expectations() {
    assert_eq!(
        parse_key("parity:main:settings-general:button:1:Sync now"),
        Some(Expect::Reference {
            window: "main".into(),
            scene: "settings-general".into(),
            role: "button".into(),
            occurrence: 1,
            name: "Sync now".into()
        })
    );
    // A name keeps its own colons.
    assert!(
        matches!(parse_key("parity:main:x:button:2:a:b"), Some(Expect::Reference { name, .. }) if name == "a:b")
    );
    assert_eq!(
        parse_key("spec:switch/sm:24x14"),
        Some(Expect::Spec {
            what: "switch/sm".into(),
            w: Some(24.0),
            h: Some(14.0)
        })
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
    assert_eq!(parse_key("swatch-1"), None);
    assert_eq!(parse_key("spec:bad"), None);
}

#[test]
fn landmarks_number_occurrences_and_measure_field_boxes() {
    let tree = json!({"elements": [
        {"path": "#0", "role": "button", "label": "Copy", "rect": [0, 0, 20, 20], "fill": "#00000000"},
        {"path": "#1", "role": "button", "label": "Copy", "rect": [0, 30, 20, 20], "fill": "#1447e6ff"},
        {"path": "#2", "role": "textbox", "rect": [10, 10, 200, 32], "fill": "#00000006"},
        {"path": "#2/#0/#1", "role": "textbox", "label": "Name", "rect": [21, 16, 170, 20], "fill": "#00000000"},
        {"path": "#3", "role": "group", "label": "Not a landmark", "rect": [0, 0, 1, 1], "fill": "#00000000"},
    ]});
    let got = landmarks(&tree);
    assert_eq!(got.len(), 3);
    assert_eq!(
        (got[1].occurrence, got[1].fill),
        (2, Some([20.0, 71.0, 230.0, 1.0]))
    );
    assert_eq!(
        got[2].rect,
        [10.0, 10.0, 200.0, 32.0],
        "the box, not the editor"
    );
    assert_eq!(got[0].fill, None);
}

#[test]
fn gates_use_two_points_and_rounded_colours() {
    assert_eq!(
        delta([0.0, 0.0, 10.0, 28.0], [5.0, 5.0, 11.5, 29.0], true),
        1.5
    );
    assert_eq!(
        delta([0.0, 0.0, 10.0, 28.0], [5.0, 0.0, 10.0, 28.0], false),
        5.0
    );
    assert!(colour_eq(
        Some([20.0, 71.0, 230.0, 1.0]),
        Some([21.0, 70.0, 231.0, 1.0])
    ));
    assert!(!colour_eq(
        Some([20.0, 71.0, 230.0, 1.0]),
        Some([30.0, 71.0, 230.0, 1.0])
    ));
    assert!(!colour_eq(None, Some([0.0, 0.0, 0.0, 0.5])));
    assert!(colour_eq(None, None));
    // A faint fill read back through the 8-bit canvas loses its channels, not its paint.
    assert!(colour_eq(
        Some([0.0, 0.0, 0.0, 0.024]),
        Some([12.0, 10.0, 9.0, 0.024])
    ));
    assert!(!colour_eq(
        Some([0.0, 0.0, 0.0, 0.024]),
        Some([0.0, 0.0, 0.0, 0.15])
    ));
}

#[test]
fn spec_rows_gate_only_the_named_sides() {
    let tree = json!({"elements": [
        {"path": "#0", "role": "switch", "key": "spec:switch:32x18.4", "rect": [0, 0, 32, 18.4], "fill": "#1447e6ff"},
        {"path": "#1", "role": "button", "key": "spec:button/sm:_x28", "rect": [0, 0, 90, 31], "fill": "#00000000"},
        {"path": "#2", "role": "group", "key": "info:menu", "rect": [0, 0, 90, 31], "fill": "#00000000"},
    ]});
    let rows = gallery_rows("switch", "light", &tree, &[]);
    assert_eq!(
        rows.iter().map(|r| (r.gated, r.pass)).collect::<Vec<_>>(),
        [(true, true), (true, false), (false, true)]
    );
}

#[test]
fn the_committed_reference_loads() {
    let refs = load_reference(&root().join("docs/migration/reference/metrics.json")).unwrap();
    let general = refs
        .iter()
        .find(|r| r.window == "main" && r.scene == "settings-general" && r.theme == "light")
        .unwrap();
    let sync = general
        .landmarks
        .iter()
        .find(|l| l.name == "Sync now")
        .unwrap();
    assert_eq!((sync.role.as_str(), sync.rect[3]), ("button", 28.0));
    assert_eq!(general.viewport, (1180.0, 760.0));
}
