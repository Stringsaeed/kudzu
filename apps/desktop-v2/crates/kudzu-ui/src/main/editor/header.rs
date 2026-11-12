//! The editor's 48 pt toolbar (EDT-1, `snippet-editor.tsx:198-302`): the group breadcrumb with
//! its "Move to" menu, the save status, Favorite, the Enabled switch and More actions (Copy
//! expanded snippet, Duplicate, Move to group, Delete).
//!
//! Favorite and Enabled edit the draft, which autosaves, as Tauri's `update` does. The star
//! bursts only when it turns on, and the Enabled/Off label fades in on a real swap, never when
//! a snippet opens. RUNE-GAP(icon fill): Rune draws Lucide icons as strokes only, so the
//! favorite star is coloured, not filled; delete the note when icons take a fill.

use kudzu_app::tasks::Task;
use kudzu_app::watch::Subscription;
use kudzu_core::library::groups::{build_tree, flatten, group_path};
use kudzu_core::SnippetRecord;
use rune::{el, icon, row, text, Curve, Easing, El, LiveRegion, Sequence, Tx};
use rune_kit::{MenuEntry, MenuItem, Placement};
use uuid::Uuid;

use super::autosave::SaveStatus;
use super::model::Timed;
use crate::main::library;
use crate::main::toasts::ToastSpec;
use crate::shell::MainState;
use crate::theme::{icons, motion, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::menu;
use crate::widgets::kit_ext::switch::{self, Switch};

pub const HEADER_KEY: &str = "editor-header";
pub const GROUP_KEY: &str = "editor-group";
pub const SAVE_KEY: &str = "editor-save-status";
pub const FAVORITE_KEY: &str = "editor-favorite";
pub const ENABLED_KEY: &str = "editor-enabled";
pub const MORE_KEY: &str = "editor-more";
/// `animate-star`: 420 ms; 125% and 6° at 55%.
pub const STAR_SECS: f64 = 0.420;
/// The star's poses: the start lands at once, then one frame later it eases to the peak.
const STAR_MARKS: [f64; 3] = [1.0 / 120.0, 1.0 / 120.0 + 0.55 * STAR_SECS, STAR_SECS];

/// Menus, motion and the copy in flight.
#[derive(Default)]
pub struct Header {
    pub group_open: bool,
    pub more_open: bool,
    pub move_open: bool,
    /// The favorite star's burst.
    pub star: Timed,
    star_phase: usize,
    /// Real Enabled/Off swaps since the snippet opened; the label fades in on each.
    pub enabled_swaps: u32,
    seen: Option<(Uuid, bool, bool)>,
    copy: Option<(Task<Result<(), String>>, Subscription)>,
}

impl Header {
    pub fn opened(&mut self, record: &SnippetRecord) {
        let copy = self.copy.take();
        *self = Header {
            seen: Some((record.id, record.is_favorite, record.is_enabled)),
            copy,
            ..Header::default()
        };
    }

    pub fn closed(&mut self) {
        let copy = self.copy.take();
        *self = Header {
            copy,
            ..Header::default()
        };
    }

    /// The draft as it is now: a favorite turning on bursts, an enabled swap fades.
    pub fn observe(&mut self, record: &SnippetRecord, now: f64) -> bool {
        let next = (record.id, record.is_favorite, record.is_enabled);
        let Some((id, favorite, enabled)) = self.seen.replace(next) else {
            return false;
        };
        if id != record.id {
            return false;
        }
        let mut changed = false;
        if !favorite && record.is_favorite {
            self.star.start(now, STAR_SECS);
            self.star_phase = 0;
            changed = true;
        }
        if favorite && !record.is_favorite {
            self.star.stop();
            changed = true;
        }
        if enabled != record.is_enabled {
            self.enabled_swaps += 1;
            changed = true;
        }
        changed
    }

    /// The star's pose now: scale, rotation in degrees, and the seconds to reach it.
    pub fn star_pose(&self, now: f64) -> (f32, f32, f32) {
        match self.star.elapsed(now) {
            Some(t) if t < STAR_MARKS[0] => (0.5, -24.0, 0.0),
            Some(t) if t < STAR_MARKS[1] => (1.25, 6.0, (STAR_MARKS[1] - STAR_MARKS[0]) as f32),
            Some(_) => (1.0, 0.0, (STAR_MARKS[2] - STAR_MARKS[1]) as f32),
            None => (1.0, 0.0, 0.0),
        }
    }

    /// Moves the star to the pose `now` reached and ends it at the last mark: whether the view
    /// must rebuild, and when the next mark is due.
    pub fn advance_star(&mut self, now: f64) -> (bool, Option<f64>) {
        let Some(t) = self.star.elapsed(now) else {
            return (false, None);
        };
        if self.star.expire(now) {
            self.star_phase = 0;
            return (true, None);
        }
        let phase = STAR_MARKS.iter().filter(|m| t + 1e-9 >= **m).count();
        let changed = phase != self.star_phase;
        self.star_phase = phase;
        (changed, STAR_MARKS.get(phase).map(|m| now - t + m))
    }

    /// The star's next pose, and the copy once it lands.
    pub fn poll(&mut self, now: f64, tx: &mut Tx) -> (bool, Option<Result<(), String>>) {
        let (changed, next) = self.advance_star(now);
        if let Some(next) = next {
            tx.wake_at(next);
        }
        let copied = self
            .copy
            .as_ref()
            .and_then(|(task, _)| task.take())
            .map(|done| {
                self.copy = None;
                done.unwrap_or_else(|error| Err(error.to_string()))
            });
        (changed, copied)
    }

    pub fn any_menu_open(&self) -> bool {
        self.group_open || self.more_open
    }

    pub fn close_menus(&mut self) {
        self.group_open = false;
        self.more_open = false;
        self.move_open = false;
    }
}

/// "Copy expanded snippet": renders the draft (no sample selection) and copies it, on a worker.
pub fn copy_expanded(s: &mut MainState) {
    let Some(draft) = s.editor.draft() else {
        return;
    };
    let context = kudzu_app::expansion::PreviewContext {
        snippet_id: Some(draft.id),
        formatting: draft.formatting.clone(),
        selection: None,
        form_values: Default::default(),
    };
    let (source, expansion) = (draft.content.clone(), s.svc.expansion.clone());
    let task = Task::spawn("kudzu-copy-expanded", move || {
        let rendered = expansion
            .preview(source, context)
            .map_err(|e| e.to_string())?
            .wait()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        expansion
            .copy_rendered(rendered)
            .map_err(|e| e.to_string())?
            .wait()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    });
    match task {
        Ok(task) => {
            let wake = s.editor.wake.clone();
            let sub = task.ready().subscribe(move || wake());
            s.editor.header.copy = Some((task, sub));
        }
        Err(error) => copied(s, Err(error.to_string())),
    }
}

/// The copy's toast: "Copied expanded snippet", or "Couldn't copy snippet" with the reason.
pub(crate) fn copied(s: &mut MainState, result: Result<(), String>) {
    let spec = match result {
        Ok(()) => ToastSpec::new("Copied expanded snippet"),
        Err(error) => ToastSpec::new("Couldn't copy snippet").description(error),
    };
    s.frame.toasts.show("editor-copy", spec, s.frame.clock_secs);
}

pub fn view(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) -> El {
    let Some(draft) = s.editor.draft() else {
        return row();
    };
    let status = s.editor.autosave.status(draft.id, s.editor.is_dirty());
    let bar = row()
        .key(HEADER_KEY)
        .role("toolbar")
        .label("Snippet")
        .h(47.0)
        .w_full()
        .px(16.0)
        .gap(8.0)
        .items_center()
        .window_drag()
        .child(group_menu(s, draft, theme, overlays))
        .child(el().grow(1.0).h(1.0))
        .child(
            row()
                .gap(4.0)
                .items_center()
                .child(save_status(&status, theme))
                .child(favorite(s, draft, theme))
                .child(enabled(s, draft, theme))
                .child(more_menu(s, draft, theme, overlays)),
        );
    rune::col()
        .w_full()
        .child(bar)
        .child(el().w_full().h(1.0).fill(theme.c.hairline).a11y_hidden())
}

fn regular(style: rune::TextStyle) -> rune::TextStyle {
    rune::TextStyle {
        weight: 400.0,
        ..style
    }
}

fn group_menu(s: &MainState, draft: &SnippetRecord, theme: &Theme, overlays: &mut Vec<El>) -> El {
    let c = &theme.c;
    let groups = &s.library_snapshot().groups;
    let path = group_path(groups, draft.group_id);
    let mut crumbs = row().gap(4.0).items_center().min_w(0.0).child(
        icon(&icons::FOLDER, 14.0)
            .color(c.muted_foreground)
            .a11y_hidden(),
    );
    if path.is_empty() {
        crumbs = crumbs.child(
            text("No group")
                .font(regular(theme.ty.control_xs))
                .color(c.muted_foreground),
        );
    }
    for (i, name) in path.iter().enumerate() {
        if i > 0 {
            crumbs = crumbs.child(
                icon(&icons::CHEVRON_RIGHT, 12.0)
                    .color(c.muted_foreground.alpha(0.5))
                    .a11y_hidden(),
            );
        }
        crumbs = crumbs.child(
            text(name.to_string())
                .font(regular(theme.ty.control_xs))
                .color(c.muted_foreground)
                .truncate(),
        );
    }
    let trigger = row()
        .key(GROUP_KEY)
        .role("button")
        .label("Move to group")
        .focusable(true)
        .expanded(s.editor.header.group_open)
        .px(6.0)
        .py(4.0)
        .radius(6.0)
        .min_w(0.0)
        .hover(c.accent)
        .no_window_drag()
        .child(crumbs)
        .on_press(|st: &mut MainState| {
            st.editor.header.group_open = !st.editor.header.group_open;
        });
    let mut m = rune_kit::Menu::new("editor-group-menu")
        .trigger(trigger)
        .open(s.editor.header.group_open)
        .placement(Placement::BottomStart)
        .min_width(224.0)
        .on_close(|st: &mut MainState| st.editor.header.group_open = false)
        .group_label("Move to")
        .item(
            MenuItem::new("No group")
                .checked(draft.group_id.is_none())
                .on_select(|st: &mut MainState| set_group(st, None)),
        );
    for node in flatten(&build_tree(groups), &Default::default()) {
        let id = node.group.id;
        m = m.item(
            MenuItem::new(node.group.name.clone())
                .lead(
                    row()
                        .child(el().w(node.depth as f32 * 14.0).h(1.0))
                        .child(icon(&icons::FOLDER, 14.0).color(c.muted_foreground.alpha(0.6))),
                )
                .checked(draft.group_id == Some(id))
                .on_select(move |st: &mut MainState| set_group(st, Some(id))),
        );
    }
    menu::build(m, theme, overlays)
}

fn set_group(s: &mut MainState, group: Option<Uuid>) {
    s.editor.header.group_open = false;
    super::edit_draft(s, |d| d.group_id = group);
}

/// `aria-live` status: transparent while idle, "Saving…", a check and "Saved", or "Couldn't
/// save" with the writer's sentence as its description.
fn save_status(status: &SaveStatus, theme: &Theme) -> El {
    let c = &theme.c;
    let font = theme.ty.meta_11;
    let mut out = row()
        .key(SAVE_KEY)
        .role("status")
        .live_region(LiveRegion::Polite)
        .label(status.label())
        .mr(8.0)
        .gap(4.0)
        .items_center()
        .opacity(if *status == SaveStatus::Idle {
            0.0
        } else {
            1.0
        })
        .opacity_motion(Curve::tween(motion::MODERATE, Easing::Linear));
    match status {
        SaveStatus::Saving => {
            out = out.child(
                text(status.label())
                    .key("editor-save-saving")
                    .font(font)
                    .color(c.faint),
            );
        }
        SaveStatus::Idle | SaveStatus::Saved => {
            out = out
                .child(
                    icon(&icons::CHECK, 12.0)
                        .key("editor-save-check")
                        .color(c.success)
                        .a11y_hidden()
                        .transition(crate::motion::check_in()),
                )
                .child(
                    text(status.label())
                        .key("editor-save-saved")
                        .font(font)
                        .color(c.faint),
                );
        }
        SaveStatus::Failed(message) => {
            out = out
                .description(message.clone())
                .help(message.clone())
                .child(
                    icon(&icons::TRIANGLE_ALERT, 12.0)
                        .color(c.destructive)
                        .a11y_hidden(),
                )
                .child(
                    text(status.label())
                        .key("editor-save-failed")
                        .font(font)
                        .color(c.destructive),
                );
        }
    }
    out
}

fn favorite(s: &MainState, draft: &SnippetRecord, theme: &Theme) -> El {
    let c = &theme.c;
    let on = draft.is_favorite;
    let (scale, degrees, seconds) = s.editor.header.star_pose(s.frame.clock_secs);
    let motion = if seconds > 0.0 {
        Curve::tween(seconds, Easing::EASE_OUT)
    } else {
        Curve::Instant
    };
    let star = icon(&icons::STAR, 16.0)
        .color(if on { c.favorite } else { c.foreground })
        .scale(scale)
        .scale_motion(motion)
        .rotate(degrees.to_radians())
        .rotate_motion(motion)
        .a11y_hidden();
    Button::new(if on {
        "Remove from favorites"
    } else {
        "Add to favorites"
    })
    .variant(Variant::Ghost)
    .size(Size::IconSm)
    .help("Favorite ⌘⇧F")
    .on_press(|st: &mut MainState| super::edit_draft(st, |d| d.is_favorite = !d.is_favorite))
    .build(theme)
    .key(FAVORITE_KEY)
    .checked(on)
    .no_window_drag()
    .child(el().absolute(6.0, 6.0).child(star))
}

fn enabled(s: &MainState, draft: &SnippetRecord, theme: &Theme) -> El {
    let c = &theme.c;
    let on = draft.is_enabled;
    let swaps = s.editor.header.enabled_swaps;
    let mut word = text(if on { "Enabled" } else { "Off" })
        .key(format!("editor-enabled-word-{swaps}"))
        .font(regular(theme.ty.control_xs))
        .color(c.muted_foreground);
    if swaps > 0 {
        word = word.sequence(Sequence::new(&motion::beats::FADE));
    }
    row()
        .h(28.0)
        .px(8.0)
        .gap(8.0)
        .items_center()
        .help("Turn this snippet on or off ⌘⇧E")
        .no_window_drag()
        .child(
            Switch::new(on)
                .size(switch::Size::Sm)
                .label("Enabled")
                .on_toggle(|st: &mut MainState| {
                    super::edit_draft(st, |d| d.is_enabled = !d.is_enabled)
                })
                .build(theme)
                .key(ENABLED_KEY),
        )
        .child(el().w(48.0).child(word))
}

fn more_menu(s: &MainState, draft: &SnippetRecord, theme: &Theme, overlays: &mut Vec<El>) -> El {
    let id = draft.id;
    let trigger = Button::new("More actions")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .icon(&icons::MORE_HORIZONTAL)
        .expanded(s.editor.header.more_open)
        .on_press(|st: &mut MainState| {
            st.editor.header.more_open = !st.editor.header.more_open;
            st.editor.header.move_open = false;
        })
        .build(theme)
        .key(MORE_KEY)
        .no_window_drag();
    let groups = &s.library_snapshot().groups;
    let mut moves = vec![MenuEntry::item(
        MenuItem::new("No group").on_select(move |st: &mut MainState| move_to(st, id, None)),
    )];
    for node in flatten(&build_tree(groups), &Default::default()) {
        let gid = node.group.id;
        moves.push(MenuEntry::item(
            MenuItem::new(node.group.name.clone())
                .lead(el().w(node.depth as f32 * 14.0).h(1.0))
                .on_select(move |st: &mut MainState| move_to(st, id, Some(gid))),
        ));
    }
    let m = rune_kit::Menu::new("editor-more-menu")
        .trigger(trigger)
        .open(s.editor.header.more_open)
        .placement(Placement::BottomEnd)
        .min_width(240.0)
        .on_close(|st: &mut MainState| st.editor.header.close_menus())
        .item(
            MenuItem::new("Copy expanded snippet")
                .icon(&icons::CLIPBOARD)
                .on_select(|st: &mut MainState| {
                    st.editor.header.close_menus();
                    copy_expanded(st);
                }),
        )
        .item(
            MenuItem::new("Duplicate")
                .icon(&icons::COPY)
                .shortcut("⌘D")
                .on_select(|st: &mut MainState| {
                    st.editor.header.close_menus();
                    library::actions::duplicate_selected(st);
                }),
        )
        .item(
            MenuItem::new("Move to group")
                .icon(&icons::FOLDER_INPUT)
                .submenu(moves, s.editor.header.move_open)
                .on_select(|st: &mut MainState| {
                    st.editor.header.move_open = !st.editor.header.move_open;
                }),
        )
        .separator()
        .item(
            MenuItem::new("Delete")
                .icon(&icons::TRASH_2)
                .danger(true)
                .shortcut("⌘⌫")
                .on_select_tx(move |st: &mut MainState, tx: &mut Tx| {
                    st.editor.header.close_menus();
                    library::actions::delete_with_undo(st, &[id], tx);
                }),
        );
    menu::build(m, theme, overlays)
}

/// `moveSnippets([id], group)`: the store's move, after the draft is written.
fn move_to(s: &mut MainState, id: Uuid, group: Option<Uuid>) {
    s.editor.header.close_menus();
    super::autosave::flush(s);
    match s.svc.library.move_snippets(&[id], group) {
        Ok(_) => s.refresh_library_snapshot(),
        Err(error) => {
            let spec = ToastSpec::new("Couldn't move snippet").description(error.to_string());
            s.frame.toasts.show("editor-move", spec, s.frame.clock_secs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::{CaseMode, FireMode};

    fn record(favorite: bool, enabled: bool) -> SnippetRecord {
        SnippetRecord {
            id: Uuid::from_u128(3),
            name: String::new(),
            triggers: vec![],
            content: String::new(),
            formatting: None,
            group_id: None,
            fire_mode: FireMode::default(),
            case_mode: CaseMode::default(),
            insertion: None,
            rule: None,
            is_favorite: favorite,
            is_enabled: enabled,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }

    #[test]
    fn the_star_bursts_only_when_turning_on_and_then_rests() {
        let mut h = Header::default();
        h.opened(&record(true, true));
        assert!(!h.star.running(), "opening a favorite stays still");
        h.observe(&record(false, true), 1.0);
        assert!(!h.star.running(), "turning off stays still");
        assert!(h.observe(&record(true, true), 2.0));
        assert_eq!(h.star_pose(2.0), (0.5, -24.0, 0.0));
        assert_eq!(h.star_pose(2.1).0, 1.25);
        assert_eq!(h.star_pose(2.3).0, 1.0);
        assert_eq!(h.advance_star(2.1), (true, Some(2.0 + STAR_MARKS[1])));
        assert_eq!(h.advance_star(2.2), (false, Some(2.0 + STAR_MARKS[1])));
        assert_eq!(h.advance_star(2.3), (true, Some(2.0 + STAR_SECS)));
        assert_eq!(h.advance_star(2.0 + STAR_SECS), (true, None));
        assert!(!h.star.running(), "the burst expires");
        assert_eq!(h.star_pose(3.0), (1.0, 0.0, 0.0));
    }

    #[test]
    fn the_enabled_label_counts_real_swaps_only() {
        let mut h = Header::default();
        h.opened(&record(false, true));
        assert_eq!(h.enabled_swaps, 0);
        h.observe(&record(false, true), 0.0);
        assert_eq!(h.enabled_swaps, 0);
        h.observe(&record(false, false), 0.0);
        assert_eq!(h.enabled_swaps, 1);
        h.opened(&record(false, false));
        assert_eq!(h.enabled_swaps, 0, "another snippet starts still");
    }
}
