//! Dragging snippets (list rows) and groups (sidebar rows) onto the sidebar's groups, as the web
//! did with HTML5 drag and drop (`application/x-kudzu-snippets`, `application/x-kudzu-group`).
//!
//! A row's `on_drag` turns into a drag once the pointer travels 4 pt (the browser's threshold).
//! While it moves, the element under the pointer inside the sidebar is the drop target: a group
//! row (`drop-group-{id}`, highlighted as `onDragOver` did) or the "Groups" heading
//! (`drop-top`, which moves to the top level / out of any group). Releasing drops; Escape or
//! losing the pointer cancels. The ghost under the pointer is what `setDragImage` showed.

use rune::{el, icon, row, text, Drag, El, Phase, Tx};
use uuid::Uuid;

use crate::ui::icons;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, Theme, Utilities};

/// The browser's drag threshold.
const THRESHOLD: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    Snippets(Vec<Uuid>),
    Group(Uuid),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Group(Uuid),
    /// The "Groups" heading.
    Top,
}

#[derive(Clone, Debug)]
pub struct DragState {
    pub payload: Payload,
    /// The ghost's text (`3 snippets`, a snippet's name) or the dragged group's name.
    pub label: String,
    /// The pointer, in window points.
    pub at: [f32; 2],
    pub over: Option<Target>,
}

/// The group rows' context menu key; the row (its trigger) is keyed `{this}-area`.
pub fn group_menu_key(id: Uuid) -> String {
    format!("drop-group-{id}")
}

/// Key of a group row (its drop zone and context-menu area).
pub fn group_key(id: Uuid) -> String {
    format!("{}-area", group_menu_key(id))
}

pub const TOP_KEY: &str = "drop-top";

/// The target an element path lies in.
fn target_of(path: &str) -> Option<Target> {
    for seg in path.split('/').rev() {
        if seg == TOP_KEY {
            return Some(Target::Top);
        }
        if let Some(id) = seg.strip_prefix("drop-group-").and_then(|s| s.strip_suffix("-area")).and_then(|s| s.parse().ok()) {
            return Some(Target::Group(id));
        }
    }
    None
}

/// Feeds one phase of a row's pointer gesture. `start` names what a drag from here carries,
/// read when the drag begins (so a multi-selection made by this press is included).
pub fn track(s: &mut MainUi, d: &Drag, tx: &mut Tx, start: impl Fn(&MainUi) -> Option<(Payload, String)>) {
    match d.phase {
        Phase::Down => s.sidebar.drag = None,
        Phase::Move => {
            let at = d.window_point();
            if s.sidebar.drag.is_none() {
                if d.dx.hypot(d.dy) < THRESHOLD {
                    tx.live_only();
                    return;
                }
                let Some((payload, label)) = start(s) else {
                    tx.live_only();
                    return;
                };
                s.sidebar.drag = Some(DragState { payload, label, at, over: None });
            }
            if let Some(drag) = &mut s.sidebar.drag {
                drag.at = at;
            }
            tx.element_at_in("sidebar-pane", at[0], at[1], |s: &mut MainUi, picked, _| {
                let over = picked.and_then(|p| target_of(p.path()));
                if let Some(drag) = &mut s.sidebar.drag {
                    drag.over = over;
                }
            });
        }
        Phase::Up => {
            let Some(drag) = s.sidebar.drag.take() else { return };
            if d.cancelled() {
                return;
            }
            if let Some(target) = drag.over {
                drop(s, &drag.payload, target);
            }
        }
    }
}

/// `onDrop` on a group row, or on the "Groups" heading.
pub fn drop(s: &mut MainUi, payload: &Payload, target: Target) {
    match (payload, target) {
        (Payload::Snippets(ids), Target::Group(g)) => s.move_snippets(ids, Some(g)),
        (Payload::Snippets(ids), Target::Top) => s.move_snippets(ids, None),
        (Payload::Group(moved), Target::Group(g)) => {
            if kudzu_core::client::groups::descendant_ids(&s.library.groups, *moved).contains(&g) {
                return;
            }
            if let Some(mut group) = s.group(*moved).cloned() {
                group.parent_id = Some(g);
                s.save_group(group);
            }
        }
        (Payload::Group(moved), Target::Top) => {
            if let Some(mut group) = s.group(*moved).cloned() {
                group.parent_id = None;
                s.save_group(group);
            }
        }
    }
}

/// Whether `target` is where the live drag would land (the row's drop highlight).
pub fn is_over(s: &MainUi, target: Target) -> bool {
    s.sidebar.drag.as_ref().is_some_and(|d| d.over == Some(target))
}

/// The drag image under the pointer, offset as `setDragImage(ghost, 10, 10)` placed it. Snippets
/// show the web's ghost chip (`rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground
/// shadow`); a group shows its row, translucent, as the browser's default snapshot does.
pub fn ghost(th: &Theme, s: &MainUi) -> El {
    let t = &th.t;
    let Some(d) = &s.sidebar.drag else { return el().key("drag-ghost-none") };
    let body = match d.payload {
        Payload::Snippets(_) => row()
            .radius(radius::MD)
            .fill(t.popover)
            .px(8.0)
            .py(4.0)
            .elevate(&shadow(th))
            .child(text(d.label.clone()).font(ty::XS).color(t.popover_foreground)),
        Payload::Group(_) => row()
            .h(28.0)
            .w(204.0)
            .gap(8.0)
            .pl(8.0)
            .pr(8.0)
            .center_y()
            .radius(radius::MD)
            .opacity(0.6)
            .child(icon(&icons::FOLDER, 16.0).color(t.muted_foreground))
            .child(text(d.label.clone()).font(ty::BASE).color(Theme::tint(t.sidebar_foreground, 0.8)).truncate().shrink()),
    };
    el().key("drag-ghost").absolute(d.at[0] - 10.0, d.at[1] - 10.0).inert().a11y_hidden().motion(rune::Spring::NONE).child(body)
}

/// Tailwind's `shadow`: `0 1px 3px 0 rgb(0 0 0 / .1), 0 1px 2px -1px rgb(0 0 0 / .1)`.
fn shadow(_th: &Theme) -> Vec<rune::Shadow> {
    let c = rune::Color::BLACK.alpha(0.1);
    vec![
        rune::Shadow { x: 0.0, y: 1.0, blur: 3.0, spread: 0.0, color: c, inset: false },
        rune::Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: -1.0, color: c, inset: false },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_come_from_the_nearest_drop_zone_on_the_path() {
        let id = Uuid::new_v4();
        assert_eq!(target_of(&format!("main-root/sidebar-pane/nav/{}/#2/#0", group_key(id))), Some(Target::Group(id)));
        assert_eq!(target_of("main-root/sidebar-pane/nav/drop-top/#0"), Some(Target::Top));
        assert_eq!(target_of("main-root/sidebar-pane/nav/nav-all"), None);
    }
}
