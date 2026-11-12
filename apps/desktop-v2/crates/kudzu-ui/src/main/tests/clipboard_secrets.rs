//! Secret handling, the large preview and the rows of the clipboard previews (`PLAN.md` S59).

use kudzu_core::clipboard::uti;
use rune::Key;

use super::clipboard_preview::{active, copy, has_key, key, open_folder, png, text_clip, until};
use super::{esc, plain, Rig};
use crate::main::clipboard::actions;
use crate::main::state::ClipFilter;
use crate::shell::MainState;

// MARK: Secrets

fn secret_clip(rig: &mut Rig) -> uuid::Uuid {
    text_clip(rig, "hunter2-correct-horse-battery");
    open_folder(rig, ClipFilter::All);
    let id = active(rig).id;
    rig.kudzu.clipboard.set_secret(id, Some(true)).unwrap();
    rig.settle();
    id
}

#[test]
fn a_secret_preview_is_covered_keeps_its_layout_and_is_not_selectable_or_read() {
    let mut rig = Rig::new(true);
    secret_clip(&mut rig);
    let cover = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("clip-concealed"))
        .unwrap();
    assert!(
        cover.rect[3] >= 112.0,
        "the cover keeps a stable minimum height"
    );
    assert_eq!(
        cover.label.as_deref().map(|l| l.contains("hunter2")),
        Some(false)
    );
    assert!(cover.focusable, "keyboard focus reveals it");
    let body = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("clip-text"))
        .unwrap();
    assert!(body.inert, "the covered text cannot be selected");
    // The hidden text is never in the accessibility label of anything.
    assert!(rig
        .rt
        .elements()
        .iter()
        .all(|e| e.label.as_deref().is_none_or(|l| !l.contains("hunter2"))));
}

#[test]
fn show_secrets_uncovers_the_preview() {
    let mut rig = Rig::new(true);
    secret_clip(&mut rig);
    rig.kudzu
        .settings
        .update(|s| s.clipboard_show_secrets = true)
        .unwrap();
    rig.settle();
    assert!(!has_key(&rig, "clip-concealed"));
}

#[test]
fn not_a_secret_clears_a_redundant_override_and_undo_puts_it_back() {
    let mut rig = Rig::new(true);
    let id = secret_clip(&mut rig);
    rig.click_key("clip-not-a-secret");
    assert_eq!(
        rig.kudzu
            .clipboard
            .watch()
            .get()
            .get(id)
            .unwrap()
            .secret_override,
        None
    );
    assert!(rig
        .rt
        .state
        .frame
        .toasts
        .entries()
        .iter()
        .any(|t| t.options.title == "Marked as not a secret"));
    rig.click_text("Undo");
    assert_eq!(
        rig.kudzu
            .clipboard
            .watch()
            .get()
            .get(id)
            .unwrap()
            .secret_override,
        Some(true)
    );
}

#[test]
fn hide_as_a_secret_has_an_undo_too() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "plain note");
    open_folder(&mut rig, ClipFilter::All);
    let id = active(&rig).id;
    rig.rt
        .update(|st: &mut MainState, _| actions::toggle_secret(st));
    rig.settle();
    assert_eq!(
        rig.kudzu
            .clipboard
            .watch()
            .get()
            .get(id)
            .unwrap()
            .secret_override,
        Some(true)
    );
    assert!(rig
        .rt
        .state
        .frame
        .toasts
        .entries()
        .iter()
        .any(|t| t.options.title == "Hidden as a secret"));
    rig.click_text("Undo");
    assert_eq!(
        rig.kudzu
            .clipboard
            .watch()
            .get()
            .get(id)
            .unwrap()
            .secret_override,
        None
    );
}

// MARK: Large preview

#[test]
fn space_opens_the_large_preview_and_escape_and_space_close_it() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "one");
    text_clip(&rig, "two");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(plain(' ', false));
    assert!(rig.rt.state.clipboard.large_preview);
    rig.press(key(Key::Down));
    assert!(
        rig.rt.state.clipboard.large_preview,
        "arrows move inside it"
    );
    assert_eq!(active(&rig).text.as_deref(), Some("one"));
    rig.press(esc());
    assert!(!rig.rt.state.clipboard.large_preview);
    rig.press(plain(' ', false));
    rig.press(plain(' ', false));
    assert!(!rig.rt.state.clipboard.large_preview);
}

#[test]
fn the_large_preview_is_920_wide_in_a_wide_window() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "one");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(plain(' ', false));
    let panel = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("clip-large-body"))
        .unwrap();
    assert!(
        panel.rect[2] <= 920.0 && panel.rect[2] > 700.0,
        "{:?}",
        panel.rect
    );
}

// MARK: Rows

#[test]
fn an_image_row_shows_its_thumbnail_once_it_has_loaded() {
    let mut rig = Rig::new(true);
    *rig.fixtures.thumbnails.output.lock().unwrap() = Some(kudzu_app::ports::Thumbnail {
        png: png(16, 16),
        width: 16,
        height: 16,
    });
    copy(&rig, vec![(uti::PNG, png(16, 16))]);
    open_folder(&mut rig, ClipFilter::All);
    let id = active(&rig).id;
    until(&mut rig, "the thumbnail", |r| {
        r.rt.state.clipboard.clips.media.as_ref().is_some_and(|m| {
            m.pixels(id, crate::clip::content::loader::Want::Thumbnail)
                .is_some()
        })
    });
}

#[test]
fn hovering_the_cover_reveals_a_secret_preview_and_leaving_covers_it_again() {
    let mut rig = Rig::new(true);
    secret_clip(&mut rig);
    let opacity = |rig: &Rig, key: &str| {
        rig.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.opacity)
            .unwrap()
    };
    assert!(opacity(&rig, "clip-text") < 0.05, "covered");
    let r = rig.rect("clip-concealed").unwrap();
    rig.rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    for _ in 0..30 {
        rig.rt.frame(0.016);
    }
    assert!(
        opacity(&rig, "clip-text") > 0.95,
        "revealed under the pointer"
    );
    assert!(
        opacity(&rig, "clip-concealed-cover") < 0.05,
        "the cover steps aside"
    );
    rig.rt.pointer_move(5.0, 5.0);
    for _ in 0..30 {
        rig.rt.frame(0.016);
    }
    assert!(opacity(&rig, "clip-text") < 0.05, "covered again");
}
