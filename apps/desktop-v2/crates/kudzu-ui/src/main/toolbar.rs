//! Main window 42pt titlebar/toolbar, sidebar toggle, breadcrumb trail, and `Search… ⌘K` trigger.

use kudzu_core::KudzuLibrary;
use rune::{el, icon, row, text, Cx, El};
use uuid::Uuid;

use crate::main::command_menu;
use crate::main::state::{
    clip_filter_key, clip_filter_label, settings_pane_label, ClipFilter, LibraryFilter, MainView,
};
use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::breadcrumbs::{fold, Breadcrumbs, Crumb as WidgetCrumb};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::shortcut::{Shortcut, Tone};

/// Outer toolbar height (`h-[42px] border-b border-hairline`).
pub const TOOLBAR_H: f32 = 42.0;
/// Left padding when macOS traffic lights are inset (`pl-[88px]`).
pub const TRAFFIC_LIGHT_PAD_LEFT: f32 = 88.0;
/// Left padding in fullscreen / headless mode (`pl-3` = 12pt).
pub const FULLSCREEN_PAD_LEFT: f32 = 12.0;
/// Right padding (`pr-4` = 16pt).
pub const TOOLBAR_PAD_RIGHT: f32 = 16.0;
/// Horizontal gap between toolbar items (`gap-3` = 12pt).
pub const TOOLBAR_GAP: f32 = 12.0;
/// Width of the `"Search commands and snippets"` trigger (`w-48` = 192pt).
pub const SEARCH_BUTTON_W: f32 = 192.0;
/// Its width in a window narrower than [`NARROW_WINDOW_W`] (`max-[1000px]:w-36` = 144pt).
pub const SEARCH_BUTTON_NARROW_W: f32 = 144.0;
/// Tailwind v4's `max-[1000px]` is `width < 1000px`.
pub const NARROW_WINDOW_W: f32 = 1000.0;
/// Maximum visible crumbs before middle groups collapse into `"Show parent groups"`.
pub const MAX_VISIBLE_CRUMBS: usize = 4;

/// A single breadcrumb item with a stable entity ID (`group:{uuid}`, `snippet:{uuid}`, etc.) so
/// renaming an entity updates its label without re-triggering arrival animations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crumb {
    /// Stable identity (`"root:snippets"`, `"group:{id}"`, `"snippet:{id}"`, etc.).
    pub id: String,
    /// Display text rendered in the breadcrumb trail.
    pub label: String,
    /// Navigation target when clicked (`None` for non-clickable/leaf crumbs).
    pub target: Option<MainView>,
}

/// Computes the full uncollapsed breadcrumb trail for `view` and `selected_snippet`.
pub fn compute_crumbs(
    view: &MainView,
    selected_snippet: Option<Uuid>,
    lib: &KudzuLibrary,
) -> Vec<Crumb> {
    let mut crumbs = Vec::new();
    match view {
        MainView::Onboarding => {}
        MainView::Insights => {
            crumbs.push(Crumb {
                id: "root:insights".into(),
                label: "Insights".into(),
                target: None,
            });
        }
        MainView::Settings(pane) => {
            crumbs.push(Crumb {
                id: "root:settings".into(),
                label: "Settings".into(),
                target: None,
            });
            crumbs.push(Crumb {
                id: format!("settings:{pane:?}"),
                label: settings_pane_label(*pane).into(),
                target: None,
            });
        }
        MainView::Clipboard(filter) => {
            crumbs.push(Crumb {
                id: "root:clipboard".into(),
                label: "Clipboard".into(),
                target: Some(MainView::Clipboard(ClipFilter::All)),
            });
            crumbs.push(Crumb {
                id: format!("clip:{}", clip_filter_key(*filter)),
                label: clip_filter_label(*filter).into(),
                target: None,
            });
        }
        MainView::Library(filter) => {
            let selected = selected_snippet.and_then(|id| lib.snippets.iter().find(|s| s.id == id));

            // Never the leaf: a filter, a group, a snippet or "All snippets" follows it.
            crumbs.push(Crumb {
                id: "root:snippets".into(),
                label: "Snippets".into(),
                target: Some(MainView::Library(LibraryFilter::All)),
            });

            // `toolbar.tsx`: the filter crumb is always a link (the widget draws the last crumb
            // as the page whatever its target).
            let filter_crumb = match filter {
                LibraryFilter::Favorites => Some(("filter:favorites", "Favorites")),
                LibraryFilter::Recent => Some(("filter:recent", "Recently used")),
                LibraryFilter::Disabled => Some(("filter:disabled", "Turned off")),
                LibraryFilter::All | LibraryFilter::Group(_) => None,
            };
            if let Some((id, label)) = filter_crumb {
                crumbs.push(Crumb {
                    id: id.into(),
                    label: label.into(),
                    target: Some(view.clone()),
                });
            }
            // The group trail comes from the selected snippet's group in every view, else
            // from the group the view shows.
            let group = selected.and_then(|sn| sn.group_id).or(match filter {
                LibraryFilter::Group(gid) => Some(*gid),
                _ => None,
            });
            if let Some(gid) = group {
                push_group_chain(&mut crumbs, gid, lib);
            }
            if selected.is_none() && *filter == LibraryFilter::All {
                // `toolbar.tsx:54`: a plain leaf, not a link.
                crumbs.push(Crumb {
                    id: "filter:all".into(),
                    label: "All snippets".into(),
                    target: None,
                });
            }

            if let Some(sn) = selected {
                let name = if sn.name.trim().is_empty() {
                    "Untitled"
                } else {
                    sn.name.as_str()
                };
                crumbs.push(Crumb {
                    id: format!("snippet:{}", sn.id),
                    label: name.to_owned(),
                    target: None,
                });
            }
        }
    }
    crumbs
}

/// The group and its ancestors, top first, each a link to its group.
fn push_group_chain(crumbs: &mut Vec<Crumb>, leaf_group_id: Uuid, lib: &KudzuLibrary) {
    let mut chain = Vec::new();
    let mut cur = Some(leaf_group_id);
    let mut guard = 0usize;
    while let Some(gid) = cur {
        if guard >= 64 {
            break;
        }
        guard += 1;
        let Some(group) = lib.groups.iter().find(|g| g.id == gid) else {
            break;
        };
        chain.push((group.id, group.name.clone()));
        cur = group.parent_id;
    }
    chain.reverse();
    for (gid, name) in chain {
        crumbs.push(Crumb {
            id: format!("group:{gid}"),
            label: name,
            target: Some(MainView::Library(LibraryFilter::Group(gid))),
        });
    }
}

/// Collapsed representation when `crumbs.len() > MAX_VISIBLE_CRUMBS`:
/// `[first, hidden(1..len-2), penult, leaf]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VisibleCrumb {
    Item(Crumb),
    Overflow(Vec<Crumb>),
}

pub fn collapse_crumbs(crumbs: &[Crumb]) -> Vec<VisibleCrumb> {
    let (shown, folded) = fold(crumbs.len());
    if folded.is_empty() {
        return shown
            .into_iter()
            .map(|i| VisibleCrumb::Item(crumbs[i].clone()))
            .collect();
    }
    vec![
        VisibleCrumb::Item(crumbs[shown[0]].clone()),
        VisibleCrumb::Overflow(folded.into_iter().map(|i| crumbs[i].clone()).collect()),
        VisibleCrumb::Item(crumbs[shown[1]].clone()),
        VisibleCrumb::Item(crumbs[shown[2]].clone()),
    ]
}

/// Builds the 42pt `<header>` toolbar (`toolbar.tsx`).
pub fn view(s: &MainState, cx: &Cx<MainState>, theme: &Theme, overlays: &mut Vec<El>) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let pad_left = if cx.titlebar_inset() > 0.0 {
        TRAFFIC_LIGHT_PAD_LEFT
    } else {
        FULLSCREEN_PAD_LEFT
    };

    let sidebar_label = if s.frame.sidebar_open {
        "Hide sidebar"
    } else {
        "Show sidebar"
    };
    let toggle_btn = Button::new("")
        .icon(&icons::PANEL_LEFT)
        .variant(Variant::Ghost)
        .size(Size::Icon)
        .name(sidebar_label)
        .expanded(s.frame.sidebar_open)
        .on_press(|st: &mut MainState| st.toggle_sidebar())
        .build(theme)
        .key("toolbar-sidebar-toggle")
        .no_window_drag();

    let divider = el()
        .key("toolbar-divider")
        .size(1.0, 17.0)
        .fill(c.border)
        .a11y_hidden();

    let crumbs = compute_crumbs(
        &s.frame.view,
        s.frame.selected_snippet,
        s.library_snapshot(),
    );
    let targets: Vec<Option<MainView>> = crumbs.iter().map(|cr| cr.target.clone()).collect();
    let widget_crumbs: Vec<WidgetCrumb> = crumbs
        .into_iter()
        .map(|cr| WidgetCrumb::new(cr.label).id(cr.id))
        .collect();

    let breadcrumb_nav = Breadcrumbs::new(widget_crumbs)
        .arrivals(s.frame.breadcrumb_arrivals.count())
        .menu_open(s.frame.breadcrumb_overflow_open)
        .build(
            theme,
            overlays,
            move |st: &mut MainState, idx: usize| {
                st.frame.breadcrumb_overflow_open = false;
                if let Some(Some(target)) = targets.get(idx) {
                    st.navigate_to(target.clone());
                }
            },
            |st: &mut MainState, open: bool| {
                st.frame.breadcrumb_overflow_open = open;
            },
        )
        .key("breadcrumb-trail")
        .h(20.0)
        .no_window_drag();

    let narrow = cx
        .layout_with("main-toolbar", |r| r[2] < NARROW_WINDOW_W)
        .unwrap_or(false);
    let search_w = if narrow {
        SEARCH_BUTTON_NARROW_W
    } else {
        SEARCH_BUTTON_W
    };
    let search_trigger = row()
        .key("toolbar-search-trigger")
        .role("button")
        .label("Search commands and snippets")
        .focusable(true)
        .no_window_drag()
        .size(search_w, 32.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(radius::LG)
        .fill(c.secondary.alpha(0.60))
        .hover(c.secondary)
        .border(1.0, c.hairline)
        .focus_ring(2.0, 2.0, c.ring)
        .on_press_tx(|st: &mut MainState, tx| {
            command_menu::open(st, tx);
        })
        .child(
            icon(&icons::SEARCH, 14.0)
                .color(c.muted_foreground)
                .a11y_hidden(),
        )
        .child(
            text("Search…")
                .font(ty.control_sm)
                .color(c.muted_foreground)
                .grow(1.0),
        )
        .child(Shortcut::combo("mod+k").tone(Tone::Ghost).build(theme));

    row()
        .key("main-toolbar")
        .role("banner")
        .w_full()
        .h(TOOLBAR_H)
        .pl(pad_left)
        .pr(TOOLBAR_PAD_RIGHT)
        .gap(TOOLBAR_GAP)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], c.hairline)
        .window_drag()
        .child(toggle_btn)
        .child(divider)
        .child(breadcrumb_nav)
        .child(search_trigger)
}
