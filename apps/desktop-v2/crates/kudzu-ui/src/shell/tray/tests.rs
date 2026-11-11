use super::*;
use kudzu_core::system::PermissionStatus;

fn status(paused: bool, secure: bool, tap: TapMode, accessibility: bool) -> EngineStatus {
    EngineStatus {
        can_post_events: true,
        can_expand: true,
        paused,
        tap,
        permissions: PermissionStatus {
            accessibility,
            input_monitoring: true,
        },
        secure_input: secure,
        secure_input_owner: secure.then(|| "Safari".into()),
        summary: String::new(),
    }
}

fn input<'a>(
    status: Option<&'a EngineStatus>,
    clipboard: bool,
    update: &'a UpdateState,
) -> Input<'a> {
    Input {
        app_name: "Kudzu Next",
        dev: true,
        status,
        clipboard_enabled: clipboard,
        palette_hotkey: "Control+Alt+Shift+Space",
        clipboard_hotkey: "Control+Alt+Shift+V",
        update,
    }
}

fn titles(model: &Model) -> Vec<&str> {
    model
        .lines
        .iter()
        .map(|line| match line {
            Line::Item(item) => item.title.as_str(),
            Line::Separator => "---",
        })
        .collect()
}

fn row(model: &Model, row: Row) -> &Item {
    model
        .lines
        .iter()
        .find_map(|line| match line {
            Line::Item(item) if item.row == row => Some(item),
            _ => None,
        })
        .unwrap()
}

#[test]
fn order_labels_and_dev_names_match_the_tauri_tray() {
    let idle = UpdateState::Idle;
    let on = status(false, false, TapMode::Active, true);
    let model = model(&input(Some(&on), true, &idle));
    assert_eq!(model.title, "Dev");
    assert_eq!(model.tooltip, "Kudzu Next — Expanding");
    assert_eq!(
        titles(&model),
        [
            "Expanding",
            "---",
            "Quick Search      ⌃⌥⇧SPACE",
            "Clipboard History      ⌃⌥⇧V",
            "New Snippet from Selection",
            "Pause Expanding",
            "---",
            "Open Kudzu Next…",
            "Settings…",
            "Check for Updates…",
            "---",
            "Quit Kudzu Next",
        ]
    );
    assert!(
        !row(&model, Row::Status).enabled,
        "the status line is a label"
    );
    assert_eq!(row(&model, Row::Pause).shortcut.text(), "⇧⌘P");
    assert_eq!(row(&model, Row::Open).shortcut.text(), "⌘O");
    assert_eq!(row(&model, Row::Settings).shortcut.text(), "⌘,");
    assert_eq!(row(&model, Row::Quit).shortcut, Shortcut::default());
}

#[test]
fn production_names_have_no_dev_title() {
    let idle = UpdateState::Idle;
    let on = status(false, false, TapMode::Active, true);
    let model = model(&Input {
        app_name: "Kudzu",
        dev: false,
        ..input(Some(&on), true, &idle)
    });
    assert_eq!(model.title, "");
    assert_eq!(row(&model, Row::Open).title, "Open Kudzu…");
    assert_eq!(row(&model, Row::Quit).title, "Quit Kudzu");
}

#[test]
fn the_status_line_and_pause_label_follow_the_engine() {
    let idle = UpdateState::Idle;
    let starting = model(&input(None, true, &idle));
    assert_eq!(row(&starting, Row::Status).title, "Starting…");
    assert_eq!(starting.tooltip, "Kudzu Next");
    let paused = status(true, false, TapMode::Active, true);
    let model = model(&input(Some(&paused), true, &idle));
    assert_eq!(row(&model, Row::Status).title, "Paused");
    assert_eq!(row(&model, Row::Pause).title, "Resume Expanding");
    let secure = status(false, true, TapMode::Active, true);
    assert_eq!(
        super::model(&input(Some(&secure), true, &idle)).tooltip,
        "Kudzu Next — Blocked — Secure Input in Safari"
    );
    let off = status(false, false, TapMode::Off, false);
    assert_eq!(
        row(&super::model(&input(Some(&off), true, &idle)), Row::Status).title,
        "Needs permission to work"
    );
}

#[test]
fn clipboard_history_is_disabled_while_recording_is_off() {
    let idle = UpdateState::Idle;
    let on = status(false, false, TapMode::Active, true);
    assert!(row(&model(&input(Some(&on), true, &idle)), Row::Clipboard).enabled);
    assert!(!row(&model(&input(Some(&on), false, &idle)), Row::Clipboard).enabled);
}

#[test]
fn glyph_priority_is_pause_then_secure_input_then_permission() {
    let idle = UpdateState::Idle;
    let kind = |s: &EngineStatus| glyph(Some(s), &idle).kind;
    assert_eq!(
        kind(&status(true, true, TapMode::Off, false)),
        GlyphKind::Paused
    );
    assert_eq!(
        kind(&status(false, true, TapMode::Off, false)),
        GlyphKind::SecureInput
    );
    assert_eq!(
        kind(&status(false, false, TapMode::Off, true)),
        GlyphKind::Permissions
    );
    assert_eq!(
        kind(&status(false, false, TapMode::Active, false)),
        GlyphKind::Permissions,
        "a live tap without Accessibility still needs permission"
    );
    assert_eq!(
        kind(&status(false, false, TapMode::Active, true)),
        GlyphKind::Normal
    );
    assert_eq!(glyph(None, &idle).kind, GlyphKind::Normal);
}

#[test]
fn update_labels_and_the_dot_follow_the_update_state() {
    let ready = UpdateState::Ready {
        version: "0.8.0".into(),
        notes: None,
        date: None,
    };
    let on = status(false, false, TapMode::Active, true);
    assert!(glyph(Some(&on), &ready).update);
    assert!(!glyph(Some(&on), &UpdateState::Checking).update);
    assert_eq!(update_label(&ready), "Restart to Update to 0.8.0");
    assert_eq!(
        update_label(&UpdateState::Downloading {
            version: "0.8.0".into(),
            downloaded: 1,
            total: None,
        }),
        "Downloading 0.8.0…"
    );
    assert_eq!(
        update_label(&UpdateState::Restarting {
            version: "0.8.0".into()
        }),
        "Restarting…"
    );
    assert_eq!(
        update_label(&UpdateState::Checking),
        "Checking for Updates…"
    );
    assert_eq!(update_label(&UpdateState::Idle), "Check for Updates…");
}

#[test]
fn row_ids_round_trip() {
    for row in ROWS {
        assert_eq!(Row::from_id(row.id()), Some(row));
    }
    assert_eq!(Row::from_id(0), None);
}
