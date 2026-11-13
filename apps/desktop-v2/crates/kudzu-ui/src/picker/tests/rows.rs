//! The list: order, sections, pills, the four empty states, and the compact sizes.

use kudzu_core::clipboard::query::{clip_filter_key, ClipFilter};
use rune::Key;

use super::{cmd, key, with, Rig};
use crate::clip::filters::PICKER_FILTERS;
use crate::clip::model::RowKind;

fn headers(rig: &Rig) -> Vec<String> {
    rig.state()
        .model
        .picker
        .clips
        .list
        .rows()
        .iter()
        .filter_map(|r| match &r.kind {
            RowKind::Header { label, .. } => Some(label.clone()),
            RowKind::Entry { .. } => None,
        })
        .collect()
}

#[test]
fn newest_first_with_date_sections_only_without_a_query() {
    let mut rig = Rig::with_clips(&["alpha", "alphabet", "beta"]);
    assert_eq!(rig.order_texts(), vec!["beta", "alphabet", "alpha"]);
    assert_eq!(headers(&rig), vec!["Today"]);
    rig.type_text("alph");
    assert_eq!(rig.order_texts(), vec!["alphabet", "alpha"]);
    assert!(headers(&rig).is_empty(), "no sections while searching");
}

#[test]
fn tab_and_shift_tab_cycle_the_six_pills_and_filter_the_list() {
    let mut rig = Rig::with_clips(&["plain text", "https://example.com"]);
    // The radio's own checked state, from the accessibility tree.
    let checked = |rig: &Rig| -> Vec<String> {
        let tree = rig.rt.accessibility_json();
        ["all", "pinned", "text", "image", "link", "file"]
            .into_iter()
            .filter(|key| is_checked(&tree, &format!("picker-pills-{key}")))
            .map(|key| {
                let filter = PICKER_FILTERS
                    .into_iter()
                    .find(|f| clip_filter_key(*f) == key)
                    .unwrap();
                crate::clip::filters::pill_label(filter).to_owned()
            })
            .collect()
    };
    assert_eq!(checked(&rig), vec!["All"]);
    let order = ["Pinned", "Text", "Images", "Links", "Files", "All"];
    for name in order {
        rig.press(key(Key::Tab));
        assert_eq!(checked(&rig), vec![name]);
    }
    rig.press(with(Key::Tab, false, true, false));
    assert_eq!(checked(&rig), vec!["Files"]);
    rig.press(with(Key::Tab, false, true, false));
    rig.press(with(Key::Tab, false, true, false));
    assert_eq!(checked(&rig), vec!["Images"]);
    rig.press(key(Key::Tab));
    assert_eq!(checked(&rig), vec!["Links"]);
    assert_eq!(rig.order_texts(), vec!["https://example.com"]);
    assert_eq!(rig.state().model.picker.filter, ClipFilter::Link);
}

#[test]
fn clicking_a_pill_filters_and_keeps_search_focused() {
    let mut rig = Rig::with_clips(&["plain text", "https://example.com"]);
    let r = rig.rect("picker-pills-link").unwrap();
    rig.rt.pointer_move(r[0] + 5.0, r[1] + 5.0);
    rig.rt.pointer_down(r[0] + 5.0, r[1] + 5.0);
    rig.rt.pointer_up(r[0] + 5.0, r[1] + 5.0);
    rig.settle();
    assert_eq!(rig.order_texts(), vec!["https://example.com"]);
}

#[test]
fn arrows_and_control_n_p_clamp_and_page_moves_seven() {
    let texts: Vec<String> = (0..20).map(|i| format!("clip {i:02}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut rig = Rig::with_clips(&refs);
    assert_eq!(rig.active_text(), "clip 19");
    rig.press(key(Key::Up));
    assert_eq!(rig.active_text(), "clip 19", "clamped at the top");
    rig.press(key(Key::Down));
    assert_eq!(rig.active_text(), "clip 18");
    rig.press(with(Key::Text("n".into()), false, false, true));
    assert_eq!(rig.active_text(), "clip 17");
    rig.press(with(Key::Text("p".into()), false, false, true));
    assert_eq!(rig.active_text(), "clip 18");
    rig.press(key(Key::PageDown));
    assert_eq!(rig.active_text(), "clip 11");
    rig.press(key(Key::PageUp));
    assert_eq!(rig.active_text(), "clip 18");
    for _ in 0..4 {
        rig.press(key(Key::PageDown));
    }
    assert_eq!(rig.active_text(), "clip 00", "clamped at the bottom");
    rig.press(key(Key::Down));
    assert_eq!(rig.active_text(), "clip 00");
}

#[test]
fn a_new_query_starts_at_the_top() {
    let mut rig = Rig::with_clips(&["apple", "apricot", "avocado"]);
    rig.press(key(Key::Down));
    rig.press(key(Key::Down));
    assert_eq!(rig.active_text(), "apple");
    rig.type_text("a");
    assert_eq!(rig.active_text(), "avocado");
}

// MARK: the four empty states, with their exact copy

#[test]
fn nothing_copied_yet() {
    let mut rig = Rig::new();
    rig.open();
    assert!(rig.has_text("Nothing copied yet."));
    assert!(rig.has_text("Copy something in any app and it shows up here."));
}

#[test]
fn a_query_with_no_match() {
    let mut rig = Rig::with_clips(&["alpha"]);
    rig.type_text("zzz");
    assert!(rig.has_text("Nothing matches “zzz”."));
    assert!(rig.has_text("Try another filter, or app: and is: to narrow it down."));
}

#[test]
fn an_empty_filter() {
    let mut rig = Rig::with_clips(&["alpha"]);
    rig.press(key(Key::Tab));
    rig.press(key(Key::Tab));
    rig.press(key(Key::Tab));
    assert_eq!(rig.state().model.picker.filter, ClipFilter::Image);
    assert!(rig.has_text("Nothing here yet."));
    assert!(rig.has_text("Try another filter, or app: and is: to narrow it down."));
}

#[test]
fn paused() {
    let mut rig = Rig::with_clips(&["alpha"]);
    // A filter with no rows, then recording off: the panel hides, and what it drew is "paused".
    rig.press(key(Key::Tab));
    rig.press(key(Key::Tab));
    rig.press(key(Key::Tab));
    rig.kudzu
        .settings
        .update(|s| s.clipboard_enabled = false)
        .unwrap();
    rig.until("the panel to hide", |r| !r.open_now());
    rig.settle();
    assert!(rig.has_text("Clipboard history is paused."));
    assert!(rig.has_text("Turn it on in Kudzu's settings."));
}

// MARK: compact sizes

#[test]
fn rows_are_50_headers_28_and_tiles_32() {
    let rig = Rig::with_clips(&["one", "two"]);
    let list = &rig.state().model.picker.clips.list;
    let id = list.order()[0];
    let slot = rig.rect(&format!("clip-{id}")).unwrap();
    assert!(
        (slot[3] - 48.0).abs() < 0.6,
        "the row's own box inside its 50 pt slot: {slot:?}"
    );
    let header = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.role == "presentation" && e.rect[3] > 20.0 && e.rect[3] < 40.0)
        .unwrap();
    assert!((header.rect[3] - 28.0).abs() < 0.6, "{:?}", header.rect);
    assert_eq!(crate::clip::list::Density::Compact.row(), 50.0);
    assert_eq!(crate::clip::list::Density::Compact.header(), 28.0);
    assert_eq!(crate::clip::list::Density::Compact.tile(), 32.0);
    let numbered = rig.state().model.picker.clips.look.numbered;
    assert!(numbered, "⌘1–9 hints on the first nine rows");
    let _ = cmd('1');
}

fn is_checked(tree: &serde_json::Value, key: &str) -> bool {
    fn walk(value: &serde_json::Value, key: &str) -> Option<bool> {
        if value.get("key").and_then(|k| k.as_str()) == Some(key) {
            let states = value.get("states").and_then(|s| s.as_array());
            return Some(
                states.is_some_and(|items| items.iter().any(|s| s.as_str() == Some("checked"))),
            );
        }
        value
            .get("children")?
            .as_array()?
            .iter()
            .find_map(|child| walk(child, key))
    }
    walk(tree, key).unwrap_or(false)
}
