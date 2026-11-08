//! The window's top bar (`components/app/main/toolbar.tsx`): the sidebar toggle, where you are
//! (breadcrumbs), and the ⌘K search button. The bar drags the window, empty space between the
//! crumbs included; its controls keep their presses. It clears the traffic lights (at (18, 14),
//! centred in its 42 pt height) with an 88 pt inset, which full screen (no lights) drops to 12.

use std::rc::Rc;

use rune::{el, icon, row, text, Align, Cx, El, Side, Spring};

use super::{ClipFilter, MainUi, SettingsSection, View};
use crate::ui::icons;
use crate::ui::kit::breadcrumbs::{Breadcrumbs, Entry, MORE_KEY};
use crate::ui::kit::button::{button_skin, hover_fast, ink, is_pressed, paint, track_press, Button, Look, Size, State, Variant};
use crate::ui::kit::dropdown_menu::{self, mount_menu, Flavor, MenuItem, MenuState};
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::theme::{radius, text as ty};
use crate::ui::theme_for;

#[derive(Default)]
pub struct ToolbarState {
    /// The breadcrumbs' "…" menu of hidden parent groups.
    pub more: MenuState,
}

/// `SETTINGS_SECTIONS` labels.
pub fn settings_label(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::General => "General",
        SettingsSection::Expansion => "Expansion",
        SettingsSection::Clipboard => "Clipboard",
        SettingsSection::Agents => "AI agents",
        SettingsSection::Apps => "Apps",
        SettingsSection::Data => "Backups & data",
        SettingsSection::About => "About",
        SettingsSection::Shortcuts => "Settings",
    }
}

/// `FILTERS` labels.
pub fn clip_label(f: ClipFilter) -> &'static str {
    match f {
        ClipFilter::All => "All",
        ClipFilter::Pinned => "Pinned",
        ClipFilter::Text => "Text",
        ClipFilter::Image => "Images",
        ClipFilter::Link => "Links",
        ClipFilter::File => "Files",
        ClipFilter::Code => "Code",
        ClipFilter::Color => "Colors",
        ClipFilter::Other => "History",
    }
}

/// A crumb: its label and where it goes (`None` for the current page).
#[derive(Clone, Debug, PartialEq)]
pub struct Crumb {
    pub label: String,
    pub to: Option<View>,
}

/// The trail for the current view and selection, as toolbar.tsx builds `items`.
pub fn crumbs(s: &MainUi) -> Vec<Crumb> {
    let mut items = Vec::new();
    let mut add = |label: &str, to: Option<View>| items.push(Crumb { label: label.to_string(), to });
    match &s.view {
        View::Settings(section) => {
            add("Settings", Some(View::Settings(None)));
            add(settings_label(section.unwrap_or(SettingsSection::General)), None);
        }
        View::Insights => add("Insights", None),
        View::Clipboard(f) => {
            add("Clipboard", Some(View::Clipboard(ClipFilter::All)));
            add(if *f != ClipFilter::All { clip_label(*f) } else { "History" }, None);
        }
        view => {
            add("Snippets", Some(View::All));
            match view {
                View::Favorites => add("Favorites", Some(View::Favorites)),
                View::Recent => add("Recently used", Some(View::Recent)),
                View::Disabled => add("Turned off", Some(View::Disabled)),
                _ => {}
            }
            let snippet = s.selected_snippet();
            let mut ancestors = Vec::new();
            let mut seen = std::collections::HashSet::new();
            let start = snippet.and_then(|x| x.group_id).or(match view {
                View::Group(id) => Some(*id),
                _ => None,
            });
            let mut group = start.and_then(|id| s.group(id));
            while let Some(g) = group {
                if !seen.insert(g.id) {
                    break;
                }
                ancestors.insert(0, Crumb { label: g.name.clone(), to: Some(View::Group(g.id)) });
                group = g.parent_id.and_then(|p| s.group(p));
            }
            items.extend(ancestors);
            if let Some(x) = snippet {
                items.push(Crumb { label: if x.name.is_empty() { "Untitled".into() } else { x.name.clone() }, to: None });
            } else if *view == View::All {
                items.push(Crumb { label: "All snippets".into(), to: None });
            }
        }
    }
    items
}

/// `onNavigate`: go there, then hand the keyboard to the list.
fn navigate(s: &mut MainUi, to: &View, tx: &mut rune::Tx) {
    s.set_view(to.clone());
    s.list.focus_list(tx);
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let width = cx.layout_size("main-root").map_or(1180.0, |sz| sz[0]);
    // macOS hides the traffic lights in full screen, so their inset would just be empty space.
    let inset = if cx.titlebar_inset() > 0.0 { 88.0 } else { 12.0 };

    // The sidebar toggle: a ghost icon button in muted ink.
    let open = s.sidebar_open;
    let toggle_label = if open { "Hide sidebar" } else { "Show sidebar" };
    let toggle = Button::icon_only(&icons::PANEL_LEFT, toggle_label)
        .variant(Variant::Ghost)
        .size(Size::Icon)
        .color(t.muted_foreground)
        .id("toolbar-sidebar-toggle")
        .on_press_tx(|s: &mut MainUi, tx| {
            s.sidebar_open = !s.sidebar_open;
            // `hidden`: the sidebar comes and goes at once.
            tx.instant();
        })
        .build(th)
        .expanded(open);
    let toggle = Tooltip::new("toolbar-sidebar-tip").side(Side::Bottom).text(toggle_label).build(cx, toggle, layer);

    // Breadcrumbs, past four collapsing the middle into a menu.
    let items = crumbs(s);
    let hidden = Breadcrumbs::hidden(items.len());
    let entries: Vec<Entry> = items
        .iter()
        .map(|c| {
            let mut e = Entry::new(c.label.clone());
            if let Some(to) = c.to.clone() {
                e = e.on_select_tx(move |s: &mut MainUi, tx| navigate(s, &to, tx));
            }
            e
        })
        .collect();
    let more_open = s.toolbar.more.open;
    let trail = Breadcrumbs::new(entries)
        .more_open(more_open)
        .on_more(|s: &mut MainUi| {
            let m = &mut s.toolbar.more;
            if m.open { m.close() } else { m.open_with(None) }
        })
        .build(th);
    if more_open && !hidden.is_empty() {
        let menu: Vec<dropdown_menu::Entry<MainUi>> = items[hidden]
            .iter()
            .map(|c| {
                let to = c.to.clone();
                dropdown_menu::Entry::Item(MenuItem::new(c.label.clone()).on_select(move |s: &mut MainUi, tx| {
                    if let Some(to) = &to {
                        navigate(s, to, tx);
                    }
                }))
            })
            .collect();
        mount_menu(th, "toolbar-more", Rc::new(menu), &s.toolbar.more, Rc::new(|s: &mut MainUi| &mut s.toolbar.more), Flavor::Dropdown, None, 128.0, (MORE_KEY, Side::Bottom, Align::Start, 4.0), Some(MORE_KEY.to_string()), layer);
    }

    // `variant="outline" h-8 w-48 justify-start gap-2 text-xs text-muted-foreground max-[1000px]:w-36`.
    let id = "toolbar-search";
    let look = Look::of(State::Rest, is_pressed(id), true);
    let (skin, _, hover_ink) = button_skin(th, Variant::Outline, radius::LG, false);
    let search = paint(hover_fast(row()), &skin, look)
        .key("toolbar-search")
        .role("button")
        .label("Search commands and snippets")
        .h(32.0)
        .w(if width <= 1000.0 { 144.0 } else { 192.0 })
        .pl(11.0)
        .pr(11.0)
        .gap(8.0)
        .center_y()
        .radius(radius::LG)
        .child(ink(icon(&icons::SEARCH, 14.0), t.muted_foreground, hover_ink, look).a11y_hidden())
        .child(ink(text("Search…").font(ty::XS), t.muted_foreground, hover_ink, look).grow(1.0).a11y_hidden())
        .child(Shortcut::new("mod+k").build(th))
        .focusable(true)
        .press_scale(0.97)
        .no_window_drag()
        .on_press(|s: &mut MainUi| s.command_open = true);
    let search = track_press(search, id.to_string());

    row()
        .key("toolbar")
        .w_full()
        .h(42.0)
        .gap(12.0)
        .pl(inset)
        .pr(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .motion(Spring::NONE)
        .window_drag()
        .child(toggle)
        .child(el().w(1.0).h(18.0).fill(t.border).a11y_hidden())
        .child(el().key("toolbar-crumbs").grow(1.0).min_w(0.0).center_y().child(trail))
        .child(search)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_and_clipboard_labels_match_the_web() {
        assert_eq!(settings_label(SettingsSection::Data), "Backups & data");
        assert_eq!(clip_label(ClipFilter::Image), "Images");
    }
}
