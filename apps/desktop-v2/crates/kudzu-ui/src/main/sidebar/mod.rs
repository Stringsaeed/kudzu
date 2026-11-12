//! Sidebar navigation; group management remains to be built.

pub mod actions;
mod footer;
mod item;
mod lifecycle;
mod model;
mod overlays;
mod rename;
use item::{count, item};
pub use lifecycle::{handle_key, poll};
pub use model::State;

use kudzu_core::library::groups::{build_tree, flatten, recursive_counts};
use rune::{col, icon, row, text, El};
use std::collections::HashSet;

use crate::main::state::{
    clip_filter_key, clip_filter_label, ClipFilter, LibraryFilter, MainView, CLIP_FOLDERS,
};
use crate::shell::MainState;
use crate::theme::{icons, Theme};

pub fn view(s: &MainState, theme: &Theme) -> El {
    let history = s.svc.clipboard.watch().get();
    let mut clipboard = col().gap(1.0);
    let mut clip_keys = Vec::new();
    for filter in CLIP_FOLDERS {
        let active = s.frame.view == MainView::Clipboard(filter);
        if matches!(filter, ClipFilter::All | ClipFilter::Pinned)
            || active
            || history.entries.iter().any(|entry| filter.contains(entry))
        {
            clip_keys.push(format!(
                "sidebar-item-{}",
                clip_filter_label(filter).to_lowercase().replace(' ', "-")
            ));
            let entry_count = history
                .entries
                .iter()
                .filter(|entry| filter.contains(entry))
                .count();
            clipboard = clipboard.child(
                item(
                    clip_filter_label(filter),
                    MainView::Clipboard(filter),
                    theme,
                )
                .child(count(
                    format!("sidebar-clip-count-{}", clip_filter_key(filter)),
                    entry_count,
                    theme,
                )),
            );
        }
    }
    let lib = s.library_snapshot();
    let tree = build_tree(&lib.groups);
    let counts = recursive_counts(&lib.groups, &lib.snippets);
    let visible = flatten(&tree, &s.sidebar.collapsed);
    let group_keys: Vec<_> = visible.iter().map(|node| node.group.id).collect();
    let nodes = flatten(&tree, &HashSet::new());
    let orphan = s
        .sidebar
        .rename
        .is_some_and(|id| !lib.groups.iter().any(|group| group.id == id));
    let empty_groups = group_keys.is_empty() && !orphan;
    let group_height = if empty_groups {
        56.0
    } else {
        ((group_keys.len() + usize::from(orphan)) as f32 * 29.0 - 1.0).max(0.0)
    };
    let mut groups = col()
        .h(group_height)
        .min_h(0.0)
        .token_motion(crate::theme::motion::soft_spring(
            crate::theme::motion::MODERATE,
        ))
        .clip_corners([0.0; 4]);
    for node in nodes {
        let id = node.group.id;
        let shown = group_keys.contains(&id);
        let has_children = !node.children.is_empty();
        let group = if s.sidebar.rename == Some(id) {
            rename::view(s, theme)
        } else {
            let mut entry = item(
                node.group.name.clone(),
                MainView::Library(LibraryFilter::Group(id)),
                theme,
            )
            .pl(8.0 + node.depth as f32 * 14.0)
            .key(format!("sidebar-group-{id}"))
            .on_press_tx(move |st: &mut MainState, tx| {
                st.navigate_to(MainView::Library(LibraryFilter::Group(id)));
                st.frame.focus.move_to(crate::main::MainPane::Sidebar);
                tx.focus(format!("sidebar-group-{id}"), false);
            })
            .on_key(move |st: &mut MainState, ev, tx| {
                if st.is_composing() {
                    return false;
                }
                if ev.key == rune::Key::Function(2) || (ev.key == rune::Key::Enter && ev.mods.alt) {
                    actions::begin_rename(st, id);
                    return true;
                }
                if has_children && ev.key == rune::Key::Left {
                    st.sidebar.collapsed.insert(id);
                    tx.animated();
                    return true;
                }
                if has_children && ev.key == rune::Key::Right {
                    st.sidebar.collapsed.remove(&id);
                    tx.animated();
                    return true;
                }
                false
            })
            .on_drag(move |st: &mut MainState, drag, tx| {
                let [x, y] = drag.window_point();
                let moved = drag.dx.hypot(drag.dy) > 4.0;
                if drag.phase == rune::Phase::Move && moved {
                    crate::dnd::update_hover(st, crate::dnd::DragPayload::Group(id), x, y, tx);
                }
                if drag.phase == rune::Phase::Up {
                    st.sidebar.drag = None;
                    st.sidebar.drop_target = None;
                    if moved && !drag.cancelled() {
                        crate::dnd::finish_drag(st, crate::dnd::DragPayload::Group(id), x, y, tx);
                    }
                }
            })
            .on_double_press(move |st: &mut MainState| actions::begin_rename(st, id))
            .on_secondary_press(move |st: &mut MainState, point, _| {
                st.sidebar.menu = Some((id, point.x, point.y))
            });
            if shown
                && !s.svc.settings.sidebar_section_closed("snippets")
                && s.frame.view == MainView::Library(LibraryFilter::Group(id))
            {
                let look = crate::widgets::selection_pill::PillLook {
                    fill: theme.c.sidebar_accent,
                    radius: crate::theme::radius::MD,
                    depth: theme.depth.selected.clone(),
                    sheen: false,
                };
                entry = entry.child(
                    crate::widgets::selection_pill::pill([0.0, 0.0, 204.0, 28.0], &look)
                        .key("sidebar-selection-pill"),
                );
            }
            if s.sidebar.drop_target == Some(crate::dnd::DropTarget::Group(id)) {
                entry = entry.fill(theme.c.sidebar_accent).child(
                    rune::el()
                        .key("sidebar-drop-indicator")
                        .size(2.0, 20.0)
                        .fill(theme.c.sidebar_primary)
                        .inert(),
                );
            }
            entry = entry.child(count(
                format!("sidebar-count-{id}"),
                counts.get(&id).copied().unwrap_or(0),
                theme,
            ));
            if !node.children.is_empty() {
                let closed = s.sidebar.collapsed.contains(&id);
                entry = entry.expanded(!closed).child(
                    row()
                        .key(format!("sidebar-collapse-{id}"))
                        .role("button")
                        .label(if closed {
                            "Expand group"
                        } else {
                            "Collapse group"
                        })
                        .w(24.0)
                        .h(24.0)
                        .on_press_tx(move |st: &mut MainState, tx| {
                            tx.animated();
                            if !st.sidebar.collapsed.insert(id) {
                                st.sidebar.collapsed.remove(&id);
                            }
                        })
                        .child(text(if closed { "▸" } else { "▾" }).color(theme.c.faint)),
                );
            }
            entry
        };
        let height = if shown {
            if group_keys.last() == Some(&id) && !orphan {
                28.0
            } else {
                29.0
            }
        } else {
            0.0
        };
        let mut slot = col()
            .key(format!("sidebar-group-slot-{id}"))
            .w_full()
            .h(height)
            .min_h(0.0)
            .token_motion(crate::theme::motion::soft_spring(
                crate::theme::motion::MODERATE,
            ))
            .clip_corners([0.0; 4])
            .child(group);
        if !shown {
            slot = slot.clip_corners([0.0; 4]).inert().a11y_hidden();
        }
        groups = groups.child(slot);
    }
    if orphan {
        groups = groups.child(rename::view(s, theme));
    }
    if empty_groups {
        groups = groups.child(item::empty_groups(theme));
    }
    let snippets_open = !s.svc.settings.sidebar_section_closed("snippets");
    let clips_open = !s.svc.settings.sidebar_section_closed("clipboard");
    let header = |name: &'static str, label: &'static str| {
        let closed = s.svc.settings.sidebar_section_closed(name);
        let badge = if name == "clipboard" && !s.svc.settings.watch().get().clipboard_enabled {
            text("Paused")
                .key("sidebar-clipboard-paused")
                .font(theme.ty.label)
                .color(theme.c.faint)
        } else {
            text("")
        };
        row()
            .key(format!("sidebar-section-{name}"))
            .role("button")
            .label(label)
            .expanded(!closed)
            .focusable(true)
            .h(24.0)
            .w(196.0)
            .mx(4.0)
            .center_y()
            .on_key(move |st: &mut MainState, ev, tx| {
                if st.is_composing() {
                    return false;
                }
                match ev.key {
                    rune::Key::Left => {
                        tx.animated();
                        let _ = st.svc.settings.set_sidebar_section_closed(name, true);
                        true
                    }
                    rune::Key::Right => {
                        tx.animated();
                        let _ = st.svc.settings.set_sidebar_section_closed(name, false);
                        true
                    }
                    _ => false,
                }
            })
            .on_press_tx(move |st: &mut MainState, tx| {
                tx.animated();
                let closed = st.svc.settings.sidebar_section_closed(name);
                let _ = st.svc.settings.set_sidebar_section_closed(name, !closed);
            })
            .child(
                text(label)
                    .grow(1.0)
                    .font(theme.ty.label)
                    .color(theme.c.faint),
            )
            .child(badge)
            .child(
                text(if closed { "▸" } else { "▾" })
                    .font(theme.ty.label)
                    .color(theme.c.faint)
                    .a11y_hidden(),
            )
    };
    let mut keys = vec!["sidebar-section-snippets".to_owned()];
    if snippets_open {
        keys.extend(
            [
                "sidebar-item-all-snippets",
                "sidebar-item-favorites",
                "sidebar-item-recently-used",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        if lib.snippets.iter().any(|snippet| !snippet.is_enabled) {
            keys.push("sidebar-item-turned-off".into());
        }
        keys.push("sidebar-new-group".into());
        keys.extend(group_keys.iter().map(|id| format!("sidebar-group-{id}")));
    }
    keys.push("sidebar-section-clipboard".into());
    if clips_open {
        keys.extend(clip_keys.iter().cloned());
    }
    keys.extend(
        [
            "sidebar-item-insights",
            "sidebar-item-settings",
            "sidebar-pause",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    let mut snippets = col()
        .key("sidebar-region-snippets")
        .role("region")
        .label("Snippets")
        .w_full()
        .pt(2.0)
        .gap(1.0)
        .child(header("snippets", "Snippets"));
    let mut snippet_body = col().key("sidebar-snippets-body").w_full().gap(1.0);
    {
        let count =
            |key: &str, value: usize| item::count(format!("sidebar-count-{key}"), value, theme);
        let favorites = lib
            .snippets
            .iter()
            .filter(|snippet| snippet.is_favorite)
            .count();
        let recent = lib
            .usage
            .iter()
            .filter(|usage| usage.last_used_at.is_some())
            .map(|usage| usage.snippet_id)
            .collect::<HashSet<_>>()
            .len();
        snippet_body = snippet_body
            .child(
                item("All snippets", MainView::Library(LibraryFilter::All), theme)
                    .mt(2.0)
                    .child(count("all", lib.snippets.len())),
            )
            .child(
                item(
                    "Favorites",
                    MainView::Library(LibraryFilter::Favorites),
                    theme,
                )
                .child(count("favorites", favorites)),
            )
            .child(
                item(
                    "Recently used",
                    MainView::Library(LibraryFilter::Recent),
                    theme,
                )
                .child(count("recent", recent)),
            );
        if lib.snippets.iter().any(|snippet| !snippet.is_enabled) {
            snippet_body = snippet_body.child(
                item(
                    "Turned off",
                    MainView::Library(LibraryFilter::Disabled),
                    theme,
                )
                .child(count(
                    "disabled",
                    lib.snippets
                        .iter()
                        .filter(|snippet| !snippet.is_enabled)
                        .count(),
                )),
            );
        }
        snippet_body = snippet_body
            .child(
                row()
                    .w_full()
                    .h(24.0)
                    .mt(11.0)
                    .center_y()
                    .pl(8.0)
                    .pr(4.0)
                    .child(
                        text("Groups")
                            .key("sidebar-groups-root")
                            .grow(1.0)
                            .font(theme.ty.label)
                            .color(theme.c.faint),
                    )
                    .child(
                        if s.sidebar.drop_target == Some(crate::dnd::DropTarget::Root) {
                            rune::el()
                                .key("sidebar-root-drop-indicator")
                                .size(2.0, 20.0)
                                .fill(theme.c.sidebar_primary)
                                .inert()
                                .a11y_hidden()
                        } else {
                            rune::el().size(0.0, 0.0).a11y_hidden()
                        },
                    )
                    .child(
                        row()
                            .key("sidebar-new-group")
                            .focusable(true)
                            .role("button")
                            .label("New group")
                            .size(20.0, 20.0)
                            .center()
                            .on_press(actions::new_group_from_button)
                            .child(icon(&icons::PLUS, 14.0).color(theme.c.faint).a11y_hidden()),
                    ),
            )
            .child(groups);
    }
    let filter_count = 3 + usize::from(lib.snippets.iter().any(|snippet| !snippet.is_enabled));
    let snippet_height = filter_count as f32 * 29.0 + 38.0 + group_height;
    snippet_body = snippet_body
        .h(if snippets_open { snippet_height } else { 0.0 })
        .min_h(0.0)
        .token_motion(crate::theme::motion::soft_spring(
            crate::theme::motion::MODERATE,
        ))
        .clip_corners([0.0; 4]);
    if !snippets_open {
        snippet_body = snippet_body.inert().a11y_hidden();
    }
    snippets = snippets.child(snippet_body);
    if let Some(pill) = overlays::selection(s, theme, "sidebar-region-snippets") {
        snippets = snippets.child(pill);
    }
    let mut clip_section = col()
        .key("sidebar-region-clipboard")
        .role("region")
        .label("Clipboard")
        .w_full()
        .pt(2.0)
        .gap(1.0)
        .child(header("clipboard", "Clipboard"));
    let clip_height = (clip_keys.len() as f32 * 29.0 - 1.0).max(0.0);
    let mut clip_body = clipboard
        .mt(2.0)
        .h(if clips_open { clip_height } else { 0.0 })
        .min_h(0.0)
        .token_motion(crate::theme::motion::soft_spring(
            crate::theme::motion::MODERATE,
        ))
        .clip_corners([0.0; 4]);
    if !clips_open {
        clip_body = clip_body
            .h(0.0)
            .min_h(0.0)
            .clip_corners([0.0; 4])
            .inert()
            .a11y_hidden();
    }
    clip_section = clip_section.child(clip_body);
    if let Some(pill) = overlays::selection(s, theme, "sidebar-region-clipboard") {
        clip_section = clip_section.child(pill);
    }
    let content = col()
        .key("sidebar-nav")
        .role("navigation")
        .label("Library")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(10.0)
        .pb(12.0)
        .gap(12.0)
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        })
        .child(snippets)
        .child(clip_section);
    let footer = footer::view(s, theme);
    let wordmark = row().h(72.0).w_full().px(16.0).center_y().child(
        row()
            .key("sidebar-wordmark")
            .role("button")
            .label("Kudzu — all snippets")
            .size(192.0, 58.313)
            .center_y()
            .on_press(|st: &mut MainState| st.navigate_to(MainView::Library(LibraryFilter::All)))
            .child(crate::widgets::wordmark::wordmark(
                crate::widgets::wordmark::Variant::Full,
                52.0,
                theme.c.foreground,
            )),
    );
    let content = col()
        .key("sidebar-root")
        .w_full()
        .h_full()
        .min_h(0.0)
        .on_key(move |st: &mut MainState, ev, tx| {
            if st.is_composing() {
                return false;
            }
            let Some(index) = tx
                .focused()
                .and_then(|key| keys.iter().position(|item| item == key))
            else {
                return false;
            };
            let next = match ev.key {
                rune::Key::Down => (index + 1).min(keys.len() - 1),
                rune::Key::Up => index.saturating_sub(1),
                rune::Key::Home => 0,
                rune::Key::End => keys.len() - 1,
                _ => return false,
            };
            tx.focus(keys[next].clone(), true);
            tx.scroll_into_view(keys[next].clone());
            true
        })
        .child(wordmark)
        .child(content)
        .child(footer);
    overlays::attach(content, s, theme)
}
