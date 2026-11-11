//! Headless proof that `El::drag_out` of an `.app` is a drag, a click is a press, and a drag
//! that never leaves the source is not a click. Live pasteboard types are measured in the probe.

use std::path::Path;

use rune::DragItem;

pub const ICON_X: f32 = 170.0;
pub const ICON_Y: f32 = 64.0;
pub const ICON: f32 = 80.0;
pub const DRAG_KEY: &str = "app-icon";

#[cfg(test)]
#[derive(Default)]
struct DragState {
    bundle: Option<std::path::PathBuf>,
    presses: usize,
    starts: usize,
    ended: Vec<rune::DragOutcome>,
}

pub fn drag_item(bundle: Option<&Path>) -> Option<DragItem> {
    let path = bundle?;
    if path.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return None;
    }
    Some(DragItem::new().file(path))
}

#[cfg(test)]
use rune::{col, el, text, Cx, El, Env, Runtime};
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
fn view(_state: &DragState, _: &mut Cx<DragState>) -> El {
    col().w_full().h_full().child(
        el().key(DRAG_KEY)
            .size(ICON, ICON)
            .fill(rune::oklch(0.546, 0.245, 262.881))
            .child(text("app"))
            .on_press(|state: &mut DragState| state.presses += 1)
            .drag_out(|state: &DragState| drag_item(state.bundle.as_deref()))
            .on_drag_start(|state: &mut DragState, _| state.starts += 1)
            .on_drag_end(|state: &mut DragState, outcome, _| state.ended.push(outcome)),
    )
}

#[cfg(test)]
fn runtime(bundle: Option<PathBuf>) -> Runtime<DragState> {
    let mut rt = Runtime::new(
        DragState {
            bundle,
            ..DragState::default()
        },
        view,
        (420.0, 660.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::drag_out::DRAG_THRESHOLD;
    use rune::DragOutcome;
    use std::path::PathBuf;

    fn icon_point(rt: &Runtime<DragState>) -> (f32, f32) {
        let icon = rt
            .elements()
            .into_iter()
            .find(|el| el.key.as_deref() == Some(DRAG_KEY))
            .expect("icon");
        (icon.rect[0] + 10.0, icon.rect[1] + 10.0)
    }

    #[test]
    fn only_an_app_bundle_is_offered() {
        assert!(drag_item(Some(Path::new("/tmp/Kudzu Permissions Spike.app"))).is_some());
        assert!(drag_item(Some(Path::new("/tmp/permissions-spike"))).is_none());
        assert!(drag_item(None).is_none());
        let item = drag_item(Some(Path::new("/tmp/Kudzu.app"))).unwrap();
        assert_eq!(item.paths(), [PathBuf::from("/tmp/Kudzu.app")]);
        assert!(!item.drops_within_app());
    }

    #[test]
    fn the_tree_marks_the_icon_as_a_drag_out() {
        let rt = runtime(Some(PathBuf::from("/tmp/Spike.app")));
        let icon = rt
            .elements()
            .into_iter()
            .find(|el| el.key.as_deref() == Some(DRAG_KEY))
            .expect("icon");
        assert!(icon.drag_out, "El::drag_out is on the app icon");
        assert!((icon.rect[2] - ICON).abs() < 0.5);
        assert!((icon.rect[3] - ICON).abs() < 0.5);
    }

    #[test]
    fn a_click_under_the_threshold_is_a_press_not_a_drag() {
        let mut rt = runtime(Some(PathBuf::from("/tmp/Spike.app")));
        let (x, y) = icon_point(&rt);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_move(x + DRAG_THRESHOLD, y);
        rt.pointer_up(x + DRAG_THRESHOLD, y);
        rt.frame(0.0);
        assert_eq!(rt.state.presses, 1, "exactly the threshold stays a click");
        assert_eq!(rt.state.starts, 0);
        assert!(rt.state.ended.is_empty());
    }

    #[test]
    fn crossing_the_threshold_starts_a_drag_out_and_is_not_a_press() {
        let mut rt = runtime(Some(PathBuf::from("/tmp/Spike.app")));
        let (x, y) = icon_point(&rt);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_move(x + DRAG_THRESHOLD + 1.0, y);
        rt.frame(0.0);
        assert_eq!(rt.state.presses, 0, "a drag-out is not a click-to-reveal");
        assert_eq!(
            rt.state.starts, 1,
            "headless hands the session over on the next frame"
        );
        assert_eq!(
            rt.state.ended,
            [DragOutcome::Cancelled],
            "with no OS host, the next frame ends Cancelled (take_drag_out is crate-private)"
        );
    }

    #[test]
    fn a_loose_binary_is_not_draggable() {
        let mut rt = runtime(None);
        let icon = rt
            .elements()
            .into_iter()
            .find(|el| el.key.as_deref() == Some(DRAG_KEY))
            .expect("icon");
        assert!(icon.drag_out);
        let (x, y) = (icon.rect[0] + 10.0, icon.rect[1] + 10.0);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_move(x + 20.0, y);
        rt.pointer_up(x + 20.0, y);
        rt.frame(0.0);
        assert_eq!(
            rt.state.presses, 1,
            "no .app means the gesture stays a press"
        );
    }
}
