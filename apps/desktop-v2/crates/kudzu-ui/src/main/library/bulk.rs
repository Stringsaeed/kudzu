//! The selection bar (`snippet-list.tsx`, LIB-5): shown while more than one row is selected,
//! with "N selected", Favorite all, Turn off all (it toggles each), Move to group and Delete
//! selected.
//!
//! When the selection drops to one row the bar leaves the tree and plays its exit; Rune keeps
//! drawing the last build (its count) and makes it inert while it sinks, as the Tauri bar does.

use std::collections::HashSet;

use kudzu_core::library::groups::{build_tree, flatten};
use kudzu_core::SnippetRecord;
use rune::{el, row, text, El, Tx};
use rune_kit::{Menu, MenuItem, Placement};
use uuid::Uuid;

use super::{actions, sync};
use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::menu;

pub const BAR_KEY: &str = "library-bulk";
pub const COUNT_KEY: &str = "bulk-count";
pub const FAVORITE_KEY: &str = "bulk-favorite";
pub const TOGGLE_KEY: &str = "bulk-toggle";
pub const MOVE_KEY: &str = "bulk-move";
pub const DELETE_KEY: &str = "bulk-delete";

/// The bar, while more than one row is selected.
pub(super) fn bar(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) -> Option<El> {
    let ids = s.library.selection.bulk();
    if ids.is_empty() {
        return None;
    }
    let c = &theme.c;
    let count = ids.len();
    let tool = |key: &str, name: &str, glyph| {
        Button::new("")
            .icon(glyph)
            .variant(Variant::Ghost)
            .size(Size::IconSm)
            .name(name)
            .build(theme)
            .key(key.to_owned())
    };
    let favorite = Button::new("")
        .icon(&icons::STAR)
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .name("Favorite all")
        .on_press(|s: &mut MainState| favorite_all(s))
        .build(theme)
        .key(FAVORITE_KEY);
    let toggle = Button::new("")
        .icon(&icons::CIRCLE_OFF)
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .name("Turn off all")
        .on_press(|s: &mut MainState| toggle_each(s))
        .build(theme)
        .key(TOGGLE_KEY);
    let move_to = move_menu(
        s,
        theme,
        overlays,
        tool(MOVE_KEY, "Move to group", &icons::FOLDER_INPUT),
    );
    let delete = Button::new("")
        .icon(&icons::TRASH_2)
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .name("Delete selected")
        .on_press_tx(|s: &mut MainState, tx| delete_all(s, tx))
        .build(theme)
        .key(DELETE_KEY);

    let label = row()
        .grow(1.0)
        .min_w(0.0)
        .center_y()
        .child(
            text(count.to_string())
                .key(COUNT_KEY)
                .font(theme.ty.control_xs.features("'tnum' 1"))
                .color(c.foreground),
        )
        .child(
            text(" selected")
                .font(theme.ty.control_xs)
                .color(c.foreground),
        );
    let bar = row()
        .key(BAR_KEY)
        .role("toolbar")
        .label("Selection actions")
        .mx(8.0)
        .my(8.0)
        .padding(6.0)
        .pl(12.0)
        .gap(4.0)
        .center_y()
        .radius(radius::XL)
        .fill(c.surface_raised)
        .transition(crate::motion::rise())
        .child(label)
        .child(favorite)
        .child(toggle)
        .child(move_to)
        .child(delete);
    Some(
        theme
            .depth
            .float
            .iter()
            .fold(bar, |e, shadow| e.box_shadow(*shadow)),
    )
}

/// "Move N to": No group, then every group in tree order, indented by depth.
fn move_menu(s: &MainState, theme: &Theme, overlays: &mut Vec<El>, trigger: El) -> El {
    let count = s.library.selection.bulk().len();
    let trigger = trigger.on_press(|s: &mut MainState| s.library.move_open = !s.library.move_open);
    let mut m = Menu::new("bulk-move-menu")
        .trigger(trigger)
        .open(s.library.move_open)
        .placement(Placement::TopEnd)
        .min_width(208.0)
        .on_close(|s: &mut MainState| s.library.move_open = false)
        .group_label(format!("Move {count} to"))
        .item(
            MenuItem::new("No group")
                .on_select_tx(|s: &mut MainState, tx: &mut Tx| move_all(s, None, tx)),
        );
    let groups = &s.library_snapshot().groups;
    let tree = build_tree(groups);
    for node in flatten(&tree, &Default::default()) {
        let id = node.group.id;
        m = m.item(
            MenuItem::new(node.group.name.clone())
                .lead(el().w(node.depth as f32 * 14.0).h(1.0))
                .on_select_tx(move |s: &mut MainState, tx: &mut Tx| move_all(s, Some(id), tx)),
        );
    }
    menu::build(m, theme, overlays)
}

fn selected(s: &MainState) -> Vec<SnippetRecord> {
    let ids: HashSet<Uuid> = s.library.selection.bulk().iter().copied().collect();
    s.library_snapshot()
        .snippets
        .iter()
        .filter(|sn| ids.contains(&sn.id))
        .cloned()
        .collect()
}

fn save(s: &mut MainState, edits: Vec<SnippetRecord>) {
    if edits.is_empty() {
        return;
    }
    if let Err(error) = s.svc.library.update_snippets(edits) {
        log::warn!("the selected snippets could not be saved: {error}");
    }
    s.refresh_library_snapshot();
    sync(s);
}

/// Favorite all: the ones not yet favorites.
fn favorite_all(s: &mut MainState) {
    let edits = selected(s)
        .into_iter()
        .filter(|sn| !sn.is_favorite)
        .map(|mut sn| {
            sn.is_favorite = true;
            sn
        })
        .collect();
    save(s, edits);
}

/// "Turn off all" toggles each snippet's own flag (FEATURES §7 keeps the label).
fn toggle_each(s: &mut MainState) {
    let edits = selected(s)
        .into_iter()
        .map(|mut sn| {
            sn.is_enabled = !sn.is_enabled;
            sn
        })
        .collect();
    save(s, edits);
}

fn move_all(s: &mut MainState, group: Option<Uuid>, _: &mut Tx) {
    s.library.move_open = false;
    let ids = s.library.selection.bulk().to_vec();
    if let Err(error) = s.svc.library.move_snippets(&ids, group) {
        log::warn!("the selected snippets could not be moved: {error}");
    }
    s.refresh_library_snapshot();
    sync(s);
}

/// Delete selected: deletes with Undo and drops the multi-selection.
fn delete_all(s: &mut MainState, tx: &mut Tx) {
    let ids = s.library.selection.bulk().to_vec();
    s.library.selection.clear();
    actions::delete_with_undo(s, &ids, tx);
}
