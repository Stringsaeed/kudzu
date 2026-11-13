//! The S60 review's M4, M6, m7 and m8.

use super::{cmd, Rig};

// MARK: M4 — the file actions act on the file they sit beside

fn two_files(rig: &mut Rig) {
    rig.fixtures.pasteboard.external_copy(vec![
        vec![(
            kudzu_core::clipboard::uti::FILE_URL.into(),
            b"file:///Users/me/one.pdf".to_vec(),
        )],
        vec![(
            kudzu_core::clipboard::uti::FILE_URL.into(),
            b"file:///Users/me/two.pdf".to_vec(),
        )],
    ]);
    rig.kudzu.clipboard.poll().unwrap();
    rig.open();
}

fn press_button(rig: &mut Rig, label: &str) {
    let row = rig
        .rect(if label.contains("two") {
            "clip-file-1"
        } else {
            "clip-file-0"
        })
        .unwrap();
    // The buttons appear with the pointer on their row.
    rig.rt.pointer_move(row[0] + 20.0, row[1] + row[3] / 2.0);
    rig.settle();
    let button = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.role == "button" && e.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no button `{label}`"));
    let (x, y) = (
        button.rect[0] + button.rect[2] / 2.0,
        button.rect[1] + button.rect[3] / 2.0,
    );
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
    rig.settle();
}

#[test]
fn open_on_the_second_file_opens_the_second_file() {
    let mut rig = Rig::new();
    two_files(&mut rig);
    press_button(&mut rig, "Open two.pdf");
    rig.until("the open", |r| {
        !r.fixtures.files.opened.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.files.opened.lock().unwrap()[0].to_string_lossy(),
        "/Users/me/two.pdf"
    );
}

#[test]
fn show_in_finder_reveals_the_file_beside_it() {
    let mut rig = Rig::new();
    two_files(&mut rig);
    press_button(&mut rig, "Show two.pdf in Finder");
    rig.until("the reveal", |r| {
        !r.fixtures.files.revealed.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.files.revealed.lock().unwrap()[0].to_string_lossy(),
        "/Users/me/two.pdf"
    );
    assert!(rig.fixtures.files.opened.lock().unwrap().is_empty());
}

#[test]
fn a_file_that_cannot_be_shown_says_so_in_the_footer() {
    let mut rig = Rig::new();
    two_files(&mut rig);
    *rig.fixtures.files.failure.lock().unwrap() = Some(kudzu_app::error::PortError::TargetGone);
    press_button(&mut rig, "Show one.pdf in Finder");
    rig.until("the sentence", |r| r.flash().is_some());
    assert!(
        rig.flash().is_some_and(|f| !f.is_empty()),
        "the failure reaches the footer"
    );
    assert!(rig.open_now(), "the picker stays so it can be read");
}

// MARK: M6 — keyboard focus reveals a concealed preview

const SECRET: &str = "hunter2-correct-horse-battery";

fn element(rig: &Rig, key: &str) -> rune::ElementInfo {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element keyed `{key}`"))
}

fn readable(rig: &Rig) -> bool {
    rig.rt.accessibility_json().to_string().contains(SECRET)
}

fn frames(rig: &mut Rig) {
    for _ in 0..30 {
        rig.rt.frame(0.016);
    }
}

fn with_secret() -> Rig {
    let mut rig = Rig::new();
    rig.copy(SECRET);
    let id = rig.kudzu.clipboard.watch().get().entries[0].id;
    rig.kudzu.clipboard.set_secret(id, Some(true)).unwrap();
    rig.open();
    frames(&mut rig);
    rig
}

#[test]
fn keyboard_focus_on_a_concealed_preview_makes_it_readable_and_selectable_and_blur_covers_it() {
    let mut rig = with_secret();
    assert!(element(&rig, "clip-text").inert, "covered: not selectable");
    assert!(!readable(&rig), "covered: silent");
    rig.rt
        .update(|_, tx| tx.focus(crate::clip::secret::COVER_KEY, true));
    rig.settle();
    frames(&mut rig);
    let text = element(&rig, "clip-text");
    assert!(!text.inert, "revealed by focus: selectable");
    assert_eq!(text.text.as_deref(), Some(SECRET));
    assert!(readable(&rig), "VoiceOver can read it");
    rig.rt.update(|_, tx| tx.blur());
    rig.settle();
    frames(&mut rig);
    assert!(element(&rig, "clip-text").inert, "covered again");
    assert!(!readable(&rig));
}

// MARK: m8 — composition comes before every chord, ⌘Q included

#[test]
fn a_chord_is_not_the_pickers_while_an_input_method_is_composing() {
    let mut rig = Rig::with_clips(&["one", "two"]);
    let quit = cmd('q');
    let mut took = true;
    rig.rt.ime(rune::Ime::Preedit("に".into(), None));
    rig.settle();
    rig.rt
        .update(|s, tx| took = crate::picker::on_key(s, &quit, tx));
    assert!(!took, "⌘Q is the input method's while it is composing");
}
