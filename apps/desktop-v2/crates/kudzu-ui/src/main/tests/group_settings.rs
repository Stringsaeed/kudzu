//! Group settings and the app picker (`PLAN.md` S55). The `group-dialog` scene stays
//! not-yet-built until S73, because its reference also includes the sidebar, list and editor.
//! These tests cover the dialog's own landmarks.

use std::time::Duration;

use kudzu_app::settings::Theme as Stored;
use kudzu_core::library::model::{AppRule, AppRuleMode, GroupRecord, InsertionMethod};
use rune::{Ime, Key, KeyEvent, Mods};
use uuid::Uuid;

use super::{cmd, esc, plain, Rig};
use crate::main::scenes::SCENES;
use crate::theme::{radius, Theme};

const TOLERANCE: f32 = 2.0;

const LANDMARKS: &[(&str, &str, [f32; 4])] = &[
    ("dialog", "Group settings", [330.0, 216.938, 520.0, 326.125]),
    ("heading", "Group settings", [346.0, 232.938, 488.0, 16.0]),
    ("button", "Close", [814.0, 224.938, 28.0, 28.0]),
    ("textbox", "Group name", [346.0, 312.938, 419.734, 36.0]),
    ("switch", "On", [777.734, 321.734, 32.0, 18.391]),
    ("radio", "Default", [669.703, 372.219, 61.844, 28.0]),
    ("radio", "Type", [731.547, 372.219, 48.266, 28.0]),
    ("radio", "Paste", [779.813, 372.219, 52.188, 28.0]),
    ("radio", "All apps", [625.984, 426.781, 66.188, 28.0]),
    ("radio", "Only in…", [692.172, 426.781, 70.281, 28.0]),
    ("radio", "Except…", [762.453, 426.781, 69.547, 28.0]),
    ("button", "Cancel", [704.891, 495.063, 67.359, 32.0]),
    ("button", "Save", [780.25, 495.063, 53.75, 32.0]),
];

fn both_themes() -> [(Stored, &'static Theme); 2] {
    [
        (Stored::Light, Theme::light()),
        (Stored::Dark, Theme::dark()),
    ]
}

fn started(rig: &Rig) -> GroupRecord {
    rig.library()
        .groups
        .iter()
        .find(|group| group.name == "Getting Started")
        .cloned()
        .expect("starter group")
}

fn group(rig: &Rig, id: Uuid) -> GroupRecord {
    rig.library()
        .groups
        .iter()
        .find(|group| group.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("missing group {id}"))
}

fn open_started(rig: &mut Rig) -> Uuid {
    let id = started(rig).id;
    rig.rt
        .update(move |st, _| st.frame.group_settings_id = Some(id));
    rig.settle();
    id
}

fn open_picker(rig: &mut Rig) -> Uuid {
    let id = open_started(rig);
    rig.click_key("group-rule:allow");
    rig.click_key("app-picker-add");
    rig.settle();
    id
}

fn landmark(rig: &Rig, role: &str, label: &str) -> [f32; 4] {
    let hits: Vec<_> = rig
        .rt
        .elements()
        .into_iter()
        .filter(|e| e.role == role && e.label.as_deref() == Some(label))
        .map(|e| e.rect)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{role} {label:?} matched {} elements: {hits:?}",
        hits.len()
    );
    hits[0]
}

fn near(got: [f32; 4], want: [f32; 4], what: &str) {
    let close = got
        .iter()
        .zip(want)
        .all(|(g, w)| (g - w).abs() <= TOLERANCE);
    assert!(close, "{what} is at {got:?}, expected {want:?}");
}

fn is_checked(rig: &Rig, key: &str) -> bool {
    fn walk(value: &serde_json::Value, key: &str) -> Option<bool> {
        if value.get("key").and_then(|k| k.as_str()) == Some(key) {
            let states = value.get("states").and_then(|s| s.as_array());
            return Some(
                states
                    .map(|items| items.iter().any(|s| s.as_str() == Some("checked")))
                    .unwrap_or(false),
            );
        }
        value
            .get("children")?
            .as_array()?
            .iter()
            .find_map(|child| walk(child, key))
    }
    walk(&rig.rt.accessibility_json(), key).unwrap_or(false)
}

fn has_text(rig: &Rig, text: &str) -> bool {
    rig.rt
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some(text))
}

fn has_key(rig: &Rig, key: &str) -> bool {
    rig.rect(key).is_some()
}

fn ids(rig: &Rig) -> Vec<String> {
    rig.rt
        .state
        .group_settings
        .rule
        .as_ref()
        .map(|rule| rule.bundle_ids.clone())
        .unwrap_or_default()
}

fn mail_icon_calls(rig: &Rig) -> usize {
    rig.fixtures
        .apps
        .icon_calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(id, size)| id == "com.apple.mail" && *size == 64)
        .count()
}

fn key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods::default(),
    }
}

fn click_labeled(rig: &mut Rig, role: &str, label: &str) {
    let rect = landmark(rig, role, label);
    let (x, y) = (rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
    rig.settle();
}

fn type_str(rig: &mut Rig, text: &str) {
    for ch in text.chars() {
        rig.press(plain(ch, false));
    }
}

fn wait_until_closed(rig: &mut Rig) {
    for _ in 0..40 {
        if rig.rt.state.frame.group_settings_id.is_none() {
            return;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(30));
        rig.settle();
    }
    panic!(
        "dialog stayed open ({:?}, durable {:?})",
        rig.rt.state.group_settings.error,
        rig.kudzu.library.saved().get()
    );
}

fn wait_for_save_error(rig: &mut Rig) {
    for _ in 0..40 {
        if rig.rt.state.group_settings.error.is_some() {
            return;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(30));
        rig.settle();
    }
    panic!("save never reported an error");
}

/// S73 enables `group-dialog`. The reference also draws the sidebar, list and editor.
#[test]
fn group_dialog_stays_out_of_the_scene_list_until_s73() {
    assert!(SCENES.iter().all(|(name, _)| *name != "group-dialog"));
    let reference = include_str!("../../../../../xtask/src/parity/reference.rs");
    assert!(reference.contains("(\"main\", \"group-dialog\", \"S55\")"));
}

/// The dialog's own reference landmarks, 520 pt wide, within 2 pt, in both themes.
#[test]
fn the_dialog_matches_the_reference_landmarks_in_both_themes() {
    for (stored, theme) in both_themes() {
        let mut rig = Rig::themed(true, stored);
        rig.rt.text = rune::text::TextSystem::new(
            crate::theme::fonts::FILES,
            crate::theme::fonts::families(),
        );
        rig.settle();
        open_started(&mut rig);
        for (role, label, want) in LANDMARKS {
            let got = landmark(&rig, role, label);
            near(got, *want, &format!("{role} {label} ({stored:?})"));
        }
        let dialog = rig
            .rt
            .elements()
            .into_iter()
            .find(|e| e.role == "dialog" && e.label.as_deref() == Some("Group settings"))
            .unwrap();
        assert!((dialog.rect[2] - 520.0).abs() <= TOLERANCE);
        assert_eq!(dialog.fill, theme.c.popover);
        assert_eq!(dialog.radius, radius::XL2);
        let footer = rig
            .rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("group-settings-footer"))
            .unwrap();
        assert_eq!(footer.fill, theme.c.muted.alpha(0.5));
        assert!(!has_key(&rig, "app-picker-add"));
    }
}

#[test]
fn the_dialog_shows_the_groups_name_switch_and_rules() {
    let mut rig = Rig::new(true);
    open_started(&mut rig);
    assert_eq!(rig.rt.state.group_settings.name.text, "Getting Started");
    assert!(rig.rt.state.group_settings.enabled);
    assert!(is_checked(&rig, "group-enabled"));
    assert!(is_checked(&rig, "group-insertion:default"));
    assert!(is_checked(&rig, "group-rule:none"));
    assert!(has_key(&rig, "group-settings-save"));
    assert!(has_key(&rig, "group-settings-cancel"));
    assert!(!has_key(&rig, "app-picker-add"));
    assert_eq!(landmark(&rig, "dialog", "Group settings")[2], 520.0);
}

#[test]
fn edits_stay_in_the_draft_until_save() {
    let mut rig = Rig::new(true);
    let id = open_started(&mut rig);
    let before = group(&rig, id);
    rig.click_key("group-name");
    type_str(&mut rig, "x");
    assert!(
        rig.rt.state.group_settings.name.text.ends_with('x'),
        "typed into {:?}",
        rig.rt.state.group_settings.name.text
    );
    rig.click_key("group-enabled");
    assert!(!rig.rt.state.group_settings.enabled);
    rig.click_key("group-insertion:type");
    assert_eq!(
        rig.rt.state.group_settings.insertion,
        Some(InsertionMethod::Type)
    );
    rig.click_key("group-rule:allow");
    assert_eq!(
        rig.rt
            .state
            .group_settings
            .rule
            .as_ref()
            .map(|rule| rule.mode),
        Some(AppRuleMode::Allow)
    );
    assert!(has_key(&rig, "app-picker-add"));
    assert_eq!(group(&rig, id), before);
}

#[test]
fn a_blank_name_keeps_the_previous_one() {
    let mut rig = Rig::new(true);
    let id = open_started(&mut rig);
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("   "));
    rig.settle();
    rig.click_key("group-settings-save");
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    assert_eq!(group(&rig, id).name, "Getting Started");
    wait_until_closed(&mut rig);
    assert_eq!(group(&rig, id).name, "Getting Started");
}

#[test]
fn save_waits_until_the_revision_is_durable() {
    let mut rig = Rig::new(true);
    let id = started(&rig).id;
    let mut seeded = started(&rig);
    seeded.rule = Some(AppRule {
        mode: AppRuleMode::Deny,
        bundle_ids: vec!["com.apple.mail".into()],
        insertion: Some(InsertionMethod::Type),
    });
    rig.kudzu.library.update_group(seeded).unwrap();
    open_started(&mut rig);
    rig.rt.update(|st, _| {
        st.group_settings.name = rune::TextEdit::new("Renamed");
        st.group_settings.enabled = false;
        st.group_settings.insertion = Some(InsertionMethod::Paste);
    });
    rig.settle();
    rig.click_key("group-settings-save");
    assert!(
        rig.rt.state.frame.group_settings_id.is_some(),
        "Save stays open until the writer reports the revision"
    );
    assert_eq!(group(&rig, id).name, "Renamed");
    wait_until_closed(&mut rig);
    let saved = group(&rig, id);
    assert_eq!(saved.name, "Renamed");
    assert!(!saved.is_enabled);
    assert_eq!(saved.insertion, Some(InsertionMethod::Paste));
    let rule = saved.rule.expect("rule");
    assert_eq!(rule.mode, AppRuleMode::Deny);
    assert_eq!(rule.bundle_ids, ["com.apple.mail"]);
    assert_eq!(rule.insertion, Some(InsertionMethod::Type));
}

/// The directory is unwritable only for the failing attempt, and restored before `Rig` shuts down.
struct Permit(std::path::PathBuf);

impl Drop for Permit {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
    }
}

#[test]
fn save_shows_the_error_and_stays_open_when_the_write_fails() {
    use std::os::unix::fs::PermissionsExt;
    let mut rig = Rig::new(true);
    let permit = Permit(rig.data_dir().to_path_buf());
    let id = open_started(&mut rig);
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Renamed"));
    rig.settle();
    rig.click_key("group-settings-save");
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    std::fs::set_permissions(&permit.0, std::fs::Permissions::from_mode(0o500)).unwrap();
    wait_for_save_error(&mut rig);
    assert_eq!(
        rig.rt.state.group_settings.error.as_deref(),
        Some("The file could not be saved. The change is kept and will be retried.")
    );
    assert!(has_text(
        &rig,
        "The file could not be saved. The change is kept and will be retried."
    ));
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    assert_eq!(group(&rig, id).name, "Renamed");
}

#[test]
fn a_missing_group_stays_open_with_the_error() {
    let mut rig = Rig::new(true);
    let id = Uuid::new_v4();
    rig.rt
        .update(move |st, _| st.frame.group_settings_id = Some(id));
    rig.settle();
    rig.click_key("group-settings-save");
    assert_eq!(
        rig.rt.state.group_settings.error.as_deref(),
        Some("Group not found")
    );
    assert!(has_text(&rig, "Group not found"));
    assert_eq!(rig.rt.state.frame.group_settings_id, Some(id));
}

#[test]
fn cancel_and_escape_close_without_writing() {
    let mut rig = Rig::new(true);
    let id = open_started(&mut rig);
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Nope"));
    rig.settle();
    rig.click_key("group-settings-cancel");
    assert!(rig.rt.state.frame.group_settings_id.is_none());
    assert_eq!(group(&rig, id).name, "Getting Started");

    open_started(&mut rig);
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Nope"));
    rig.settle();
    rig.press(esc());
    assert!(rig.rt.state.frame.group_settings_id.is_none());
    assert_eq!(group(&rig, id).name, "Getting Started");
}

#[test]
fn escape_in_the_picker_closes_only_the_list() {
    let mut rig = Rig::new(true);
    open_picker(&mut rig);
    assert!(has_key(&rig, "app-picker-list"));
    rig.click_key("app-picker-search");
    rig.press(esc());
    assert!(!has_key(&rig, "app-picker-list"));
    assert!(rig.rt.state.frame.group_settings_id.is_some());

    rig.click_key("app-picker-add");
    rig.settle();
    assert!(has_key(&rig, "app-picker-list"));
    click_labeled(&mut rig, "textbox", "Group name");
    assert!(!has_key(&rig, "app-picker-list"));
    assert!(rig.rt.state.frame.group_settings_id.is_some());
}

#[test]
fn the_picker_searches_and_keeps_checks_open() {
    let mut rig = Rig::new(true);
    open_picker(&mut rig);
    for _ in 0..50 {
        if rig.kudzu.apps.installed().get().len() == 12 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        rig.settle();
    }
    assert_eq!(rig.kudzu.apps.installed().get().len(), 12);
    rig.settle();
    rig.click_key("app-picker-search");
    type_str(&mut rig, "mail");
    assert!(
        has_key(&rig, "app-picker-row-com.apple.mail"),
        "query {:?}",
        rig.rt.state.group_settings.query.text
    );
    assert!(!has_key(&rig, "app-picker-row-com.google.Chrome"));

    rig.rt.update(|st, _| {
        st.group_settings.query = rune::TextEdit::new("com.apple.terminal");
    });
    rig.settle();
    assert!(has_key(&rig, "app-picker-row-com.apple.Terminal"));
    assert!(!has_key(&rig, "app-picker-row-com.apple.mail"));

    rig.rt
        .update(|st, _| st.group_settings.query = rune::TextEdit::new("nope"));
    rig.settle();
    assert!(has_text(&rig, "No apps found."));
    assert!(!has_key(&rig, "app-picker-custom"));

    rig.rt.update(|st, _| {
        st.group_settings.query = rune::TextEdit::new("com.example.app");
    });
    rig.settle();
    assert!(has_text(&rig, "Add “com.example.app”"));
    let custom = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("app-picker-custom"))
        .expect("custom row");
    assert_eq!(custom.role, "button");
    assert_eq!(custom.label.as_deref(), Some("Add “com.example.app”"));
    rig.click_key("app-picker-custom");
    assert!(has_key(&rig, "app-picker-list"));
    assert_eq!(rig.rt.state.group_settings.query.text, "");
    assert_eq!(ids(&rig), ["com.example.app".to_string()]);
    assert!(has_key(&rig, "app-picker-chip-com.example.app"));
    let remove = landmark(&rig, "button", "Remove com.example.app");
    assert_eq!((remove[2], remove[3]), (20.0, 20.0));
    click_labeled(&mut rig, "button", "Remove com.example.app");
    assert!(ids(&rig).is_empty());
    assert!(!has_key(&rig, "app-picker-chip-com.example.app"));

    let long = "com.example.supercalifragilistic-expialidocious-app";
    rig.rt
        .update(|st, _| st.group_settings.query = rune::TextEdit::new(long));
    rig.settle();
    rig.click_key("app-picker-custom");
    let chip_text = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some(long))
        .expect("long chip");
    assert!(chip_text.rect[2] <= 160.0, "chip text {:?}", chip_text.rect);
    assert!(rig
        .rt
        .elements()
        .iter()
        .all(|e| { e.key.as_deref().is_none_or(|key| !key.contains("#dup")) }));
    click_labeled(&mut rig, "button", &format!("Remove {long}"));
    assert!(ids(&rig).is_empty());

    rig.click_key("app-picker-row-com.apple.mail");
    assert!(has_key(&rig, "app-picker-list"));
    let mail = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("app-picker-row-com.apple.mail"))
        .expect("mail row");
    assert_eq!(mail.role, "option");
    assert_eq!(mail.label.as_deref(), Some("Mail"));
    assert!(is_checked(&rig, "app-picker-row-com.apple.mail"));
    assert!(has_key(&rig, "app-picker-check-com.apple.mail"));
    assert_eq!(ids(&rig), ["com.apple.mail".to_string()]);
    rig.click_key("app-picker-row-com.apple.mail");
    assert!(has_key(&rig, "app-picker-list"));
    assert!(!has_key(&rig, "app-picker-check-com.apple.mail"));
    assert!(ids(&rig).is_empty());
}

#[test]
fn a_missing_app_shows_its_bundle_id_and_a_monogram() {
    let mut rig = Rig::new(true);
    open_picker(&mut rig);
    rig.rt.update(|st, _| {
        st.group_settings.query = rune::TextEdit::new("com.example.missing");
    });
    rig.settle();
    rig.click_key("app-picker-custom");
    assert!(has_text(&rig, "com.example.missing"));
    assert!(has_key(&rig, "app-icon-mono"));
    assert!(!has_key(&rig, "app-icon"));
    let letter = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("C"))
        .expect("monogram");
    assert_eq!(letter.text_color, Some(Theme::dark().c.muted_foreground));
}

#[test]
fn icons_load_off_the_main_thread_and_stay_cached() {
    let mut rig = Rig::new(true);
    rig.fixtures.apps.icons.lock().unwrap().insert(
        "com.apple.mail".into(),
        crate::widgets::app_icon::PNG_1X1.to_vec(),
    );
    open_picker(&mut rig);
    for _ in 0..50 {
        if mail_icon_calls(&rig) == 1 && has_key(&rig, "app-icon") {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        rig.settle();
    }
    assert_eq!(mail_icon_calls(&rig), 1);
    assert!(has_key(&rig, "app-icon"));
    rig.settle();
    assert_eq!(mail_icon_calls(&rig), 1);
}

#[test]
fn typing_a_group_name_does_not_create_a_snippet() {
    let mut rig = Rig::new(true);
    open_started(&mut rig);
    assert!(rig.rt.state.is_editing_text());
    let before = rig.library().snippets.len();
    rig.press(cmd('n'));
    assert_eq!(rig.library().snippets.len(), before);
    assert!(rig.rt.state.frame.group_settings_id.is_some());
}

#[test]
fn a_writer_that_is_already_failing_shows_the_error_on_save() {
    use std::os::unix::fs::PermissionsExt;
    let mut rig = Rig::new(true);
    let mut record = started(&rig);
    let id = record.id;
    record.name = "First".into();
    let permit = Permit(rig.data_dir().to_path_buf());
    std::fs::set_permissions(&permit.0, std::fs::Permissions::from_mode(0o500)).unwrap();
    rig.kudzu.library.update_group(record).unwrap();
    for _ in 0..40 {
        if rig.kudzu.library.saved().get().error.is_some() {
            break;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(
        rig.kudzu.library.saved().get().error.is_some(),
        "the writer is failing before the dialog opens"
    );
    rig.rt
        .update(move |st, _| st.frame.group_settings_id = Some(id));
    rig.settle();
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Second"));
    rig.settle();
    rig.click_key("group-settings-save");
    assert_eq!(
        rig.rt.state.group_settings.error.as_deref(),
        Some("The file could not be saved. The change is kept and will be retried.")
    );
    assert!(has_text(
        &rig,
        "The file could not be saved. The change is kept and will be retried."
    ));
    let alert = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("group-settings-error"))
        .expect("error");
    assert_eq!(alert.role, "alert");
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    assert_eq!(group(&rig, id).name, "Second");
}

#[test]
fn a_pending_save_keeps_later_edits_and_shows_saving() {
    let mut rig = Rig::new(true);
    let id = open_started(&mut rig);
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Renamed"));
    rig.settle();
    rig.click_key("group-settings-save");
    assert!(has_text(&rig, "Saving…"));
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    let name = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("group-name"))
        .expect("name");
    assert!(name.inert, "the form is disabled while the save is pending");
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Later"));
    wait_until_closed_or_durable(&mut rig);
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    assert_eq!(rig.rt.state.group_settings.name.text, "Later");
    assert_eq!(group(&rig, id).name, "Renamed");
}

#[test]
fn return_in_the_name_field_saves() {
    let mut rig = Rig::new(true);
    let id = open_started(&mut rig);
    assert!(rig.rt.state.group_settings.name.focused);
    rig.rt.update(|st, _| {
        st.group_settings.name = rune::TextEdit::new("Renamed");
        st.group_settings.name.focused = true;
    });
    rig.settle();
    rig.press(key(Key::Enter));
    assert_eq!(group(&rig, id).name, "Renamed");
    wait_until_closed(&mut rig);
    assert_eq!(group(&rig, id).name, "Renamed");
}

#[test]
fn the_app_list_scrolls_inside_a_short_window() {
    let mut rig = Rig::new(true);
    rig.rt.set_viewport((900.0, 560.0), 1.0);
    rig.settle();
    open_picker(&mut rig);
    for _ in 0..50 {
        if rig.kudzu.apps.installed().get().len() == 12 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        rig.settle();
    }
    assert_eq!(rig.kudzu.apps.installed().get().len(), 12);
    rig.settle();
    let rows = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref().is_some_and(|k| k.ends_with("-rows")))
        .expect("row viewport");
    assert!(rows.rect[3] <= 288.0, "rows are {rows:?}");
    let list = rig.rect("app-picker-list").expect("list");
    assert!(
        list[0] >= -0.5
            && list[1] >= -0.5
            && list[0] + list[2] <= 900.5
            && list[1] + list[3] <= 560.5,
        "list {list:?} leaves the 900×560 window"
    );
}

#[test]
fn the_search_field_moves_the_active_row() {
    let mut rig = Rig::new(true);
    open_picker(&mut rig);
    for _ in 0..50 {
        if rig.kudzu.apps.installed().get().len() == 12 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        rig.settle();
    }
    assert!(
        rig.rt.state.group_settings.query.focused,
        "the search field is focused when the list opens"
    );
    let focused = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .expect("focused element");
    assert!(
        focused.path.contains("app-picker-search"),
        "focused path {}",
        focused.path
    );
    rig.rt.ime(Ime::Preedit("か".into(), None));
    rig.rt.frame(0.016);
    rig.press(key(Key::Down));
    assert!(ids(&rig).is_empty());
    rig.rt.ime(Ime::Commit(String::new()));
    rig.rt.frame(0.016);
    rig.press(key(Key::Down));
    assert_eq!(rig.rt.state.group_settings.picker_active, 1);
    rig.press(key(Key::End));
    assert_eq!(rig.rt.state.group_settings.picker_active, 11);
    rig.press(key(Key::Home));
    assert_eq!(rig.rt.state.group_settings.picker_active, 0);
    rig.press(key(Key::Down));
    rig.press(key(Key::Enter));
    assert!(!ids(&rig).is_empty(), "Enter toggles the active row");
    assert!(has_key(&rig, "app-picker-list"));
}

#[test]
fn adding_a_bundle_id_does_not_toggle_it_off() {
    let mut rig = Rig::new(true);
    open_picker(&mut rig);
    rig.rt.update(|st, _| {
        st.group_settings.rule.as_mut().unwrap().bundle_ids = vec!["com.example.app".into()];
        st.group_settings.query = rune::TextEdit::new("com.example.app");
    });
    rig.settle();
    rig.click_key("app-picker-custom");
    assert_eq!(ids(&rig), ["com.example.app".to_string()]);
    assert_eq!(rig.rt.state.group_settings.query.text, "");
}

#[test]
fn a_press_outside_the_list_does_not_drop_the_draft() {
    let mut rig = Rig::new(true);
    open_picker(&mut rig);
    rig.rt
        .update(|st, _| st.group_settings.name = rune::TextEdit::new("Kept"));
    rig.settle();
    rig.rt.pointer_move(8.0, 8.0);
    rig.rt.pointer_down(8.0, 8.0);
    rig.rt.pointer_up(8.0, 8.0);
    rig.settle();
    assert!(rig.rt.state.frame.group_settings_id.is_some());
    assert_eq!(rig.rt.state.group_settings.name.text, "Kept");
    assert!(!has_key(&rig, "app-picker-list"));
}

#[test]
fn changing_the_rule_mode_clears_only_the_rules_insertion() {
    let mut rig = Rig::new(true);
    let id = started(&rig).id;
    let mut seeded = started(&rig);
    seeded.rule = Some(AppRule {
        mode: AppRuleMode::Allow,
        bundle_ids: vec!["com.apple.mail".into()],
        insertion: Some(InsertionMethod::Type),
    });
    rig.kudzu.library.update_group(seeded).unwrap();
    open_started(&mut rig);
    rig.click_key("group-rule:deny");
    let draft = rig.rt.state.group_settings.rule.clone().expect("rule");
    assert_eq!(draft.mode, AppRuleMode::Deny);
    assert_eq!(draft.bundle_ids, ["com.apple.mail"]);
    assert_eq!(draft.insertion, None);
    rig.click_key("group-settings-save");
    wait_until_closed(&mut rig);
    let rule = group(&rig, id).rule.expect("saved rule");
    assert_eq!(rule.mode, AppRuleMode::Deny);
    assert_eq!(rule.bundle_ids, ["com.apple.mail"]);
    assert_eq!(rule.insertion, None);
}

#[test]
fn open_loads_the_named_group_before_the_next_frame() {
    let mut rig = Rig::new(true);
    let first = started(&rig).id;
    let (other, _) = rig
        .kudzu
        .library
        .create_group("Other".into(), None)
        .unwrap();
    rig.rt.update(move |st, _| {
        crate::main::group_settings::open(st, first);
        crate::main::group_settings::open(st, other.id);
    });
    assert_eq!(rig.rt.state.group_settings.name.text, "Other");
    assert!(rig.rt.state.group_settings.name.focused);
}

fn wait_until_closed_or_durable(rig: &mut Rig) {
    for _ in 0..40 {
        let saved = rig.kudzu.library.saved().get();
        if saved.durable.0 >= 2 {
            rig.settle();
            return;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(30));
        rig.settle();
    }
    panic!(
        "writer never became durable: {:?}",
        rig.kudzu.library.saved().get()
    );
}
