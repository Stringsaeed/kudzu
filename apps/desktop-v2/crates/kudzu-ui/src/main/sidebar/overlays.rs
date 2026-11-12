//! Selection, context-menu and drag overlays for the sidebar.

use super::actions;
use crate::main::state::{clip_filter_label, LibraryFilter, MainView};
use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use rune::{col, icon, row, text, El};

pub(super) fn selection(s: &MainState, theme: &Theme, scope: &str) -> Option<El> {
    let (section, key) = match &s.frame.view {
        MainView::Library(_) if s.svc.settings.sidebar_section_closed("snippets") => return None,
        MainView::Clipboard(_) if s.svc.settings.sidebar_section_closed("clipboard") => {
            return None
        }
        MainView::Library(LibraryFilter::Group(_)) => return None,
        MainView::Library(LibraryFilter::All) => (
            "sidebar-region-snippets",
            "sidebar-item-all-snippets".to_owned(),
        ),
        MainView::Library(LibraryFilter::Favorites) => (
            "sidebar-region-snippets",
            "sidebar-item-favorites".to_owned(),
        ),
        MainView::Library(LibraryFilter::Recent) => (
            "sidebar-region-snippets",
            "sidebar-item-recently-used".to_owned(),
        ),
        MainView::Library(LibraryFilter::Disabled) => (
            "sidebar-region-snippets",
            "sidebar-item-turned-off".to_owned(),
        ),
        MainView::Clipboard(filter) => (
            "sidebar-region-clipboard",
            format!(
                "sidebar-item-{}",
                clip_filter_label(*filter).to_lowercase().replace(' ', "-")
            ),
        ),
        MainView::Insights => ("sidebar-footer", "sidebar-item-insights".to_owned()),
        MainView::Settings(_) => ("sidebar-footer", "sidebar-item-settings".to_owned()),
        MainView::Onboarding => return None,
    };
    if section != scope {
        return None;
    }
    let parent = rune::memo::layout(section);
    let target = rune::memo::layout(&key);
    let (parent, target) = parent.zip(target)?;
    let look = crate::widgets::selection_pill::PillLook {
        fill: theme.c.sidebar_accent,
        radius: crate::theme::radius::MD,
        depth: theme.depth.selected.clone(),
        sheen: false,
    };
    let rect = [
        target[0] - parent[0],
        target[1] - parent[1],
        target[2],
        target[3],
    ];
    Some(crate::widgets::selection_pill::pill(rect, &look).key("sidebar-selection-pill"))
}

pub(super) fn attach(mut content: El, s: &MainState, theme: &Theme) -> El {
    let lib = s.library_snapshot();
    if let Some((id, x, y)) = s.sidebar.menu {
        if let Some(group) = lib.groups.iter().find(|group| group.id == id) {
            let root = rune::memo::layout("sidebar-root").unwrap_or([0.0, 42.0, 224.0, 718.0]);
            let window =
                rune::memo::layout("main-window-frame").unwrap_or([0.0, 0.0, 1180.0, 760.0]);
            content = content.child(
                row()
                    .key("sidebar-menu-scrim")
                    .absolute(window[0] - root[0], window[1] - root[1])
                    .size(window[2], window[3])
                    .paint_order(99)
                    .on_press(|st: &mut MainState| st.sidebar.menu = None)
                    .a11y_hidden(),
            );
            let height = if group.parent_id.is_some() {
                196.0
            } else {
                168.0
            };
            let left = x.min(window[0] + window[2] - 208.0).max(window[0]) - root[0];
            let top = y.min(root[1] + root[3] - height).max(root[1]) - root[1];
            let mut menu = col()
                .key("sidebar-group-menu")
                .role("menu")
                .focus_group()
                .on_key(|st: &mut MainState, _, _| st.is_composing())
                .modal(true, |st: &mut MainState| st.sidebar.menu = None)
                .absolute(left, top)
                .w(208.0)
                .radius(radius::LG)
                .fill(theme.c.popover)
                .paint_order(100);
            for shadow in &theme.depth.float {
                menu = menu.box_shadow(*shadow);
            }
            for (label, action) in [
                ("Rename", 0),
                ("New subgroup", 1),
                ("Group settings…", 2),
                (
                    if group.is_enabled {
                        "Turn off group"
                    } else {
                        "Turn on group"
                    },
                    3,
                ),
                ("Move to top level", 4),
                ("Delete group, keep snippets", 5),
                ("Delete group and snippets", 6),
            ] {
                if action == 4 && group.parent_id.is_none() {
                    continue;
                }
                let glyph = match action {
                    0 => &icons::PENCIL,
                    1 => &icons::FOLDER_PLUS,
                    2 => &icons::SLIDERS_HORIZONTAL,
                    3 if group.is_enabled => &icons::CIRCLE_OFF,
                    3 => &icons::PLAY,
                    4 => &icons::LAYERS,
                    _ => &icons::TRASH_2,
                };
                menu = menu.child(
                    row()
                        .role("menuitem")
                        .label(label)
                        .focusable(true)
                        .h(28.0)
                        .px(8.0)
                        .gap(8.0)
                        .center_y()
                        .on_press(move |st: &mut MainState| match action {
                            0 => actions::begin_rename(st, id),
                            1 => actions::subgroup(st, id),
                            2 => {
                                st.sidebar.menu = None;
                                crate::main::group_settings::open(st, id);
                            }
                            3 => actions::toggle_group(st, id),
                            4 => actions::unparent(st, id),
                            5 => actions::delete(st, id, false),
                            _ => actions::delete(st, id, true),
                        })
                        .child(
                            icon(glyph, 14.0)
                                .color(theme.c.muted_foreground)
                                .a11y_hidden(),
                        )
                        .child(
                            text(label)
                                .font(theme.ty.control_sm)
                                .color(theme.c.foreground),
                        ),
                );
            }
            content = content.child(menu);
        }
    }
    if let Some((payload, x, y)) = &s.sidebar.drag {
        let label = match payload {
            crate::dnd::DragPayload::Group(id) => lib
                .groups
                .iter()
                .find(|group| group.id == *id)
                .map(|group| group.name.clone())
                .unwrap_or_default(),
            crate::dnd::DragPayload::Snippets(ids) if ids.len() == 1 => lib
                .snippets
                .iter()
                .find(|snippet| snippet.id == ids[0])
                .map(|snippet| snippet.name.clone())
                .unwrap_or_else(|| "1 snippet".into()),
            crate::dnd::DragPayload::Snippets(ids) => format!("{} snippets", ids.len()),
        };
        content = content.child(
            row()
                .key("sidebar-drag-ghost")
                .absolute(*x + 8.0, *y - 34.0)
                .h(28.0)
                .px(8.0)
                .fill(theme.c.card)
                .paint_order(101)
                .inert()
                .a11y_hidden()
                .child(
                    text(label)
                        .font(theme.ty.control_sm)
                        .color(theme.c.foreground),
                ),
        );
    }
    content
}
