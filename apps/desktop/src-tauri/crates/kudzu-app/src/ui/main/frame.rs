//! The main window's frame (`MainApp` in main-app.tsx): onboarding until it's done, then the
//! toolbar over the sidebar and the main card (list + editor, or a full page), with the command
//! menu, shortcuts sheet, group settings and toasts above everything.
//!
//! ```text
//! ┌ vibrancy (bg-sidebar) ─────────────────────────────────────────┐
//! │ toolbar                                                        │
//! │ sidebar 224 ┌ main: my-2 mr-2, rounded-2xl, bg-background, shadow-card ┐
//! │             │ list clamp(260, 27vw, 320) │ editor                │
//! └─────────────┴────────────────────────────┴───────────────────────┘
//! ```

use std::sync::OnceLock;

use rune::{Chord, Cx, El, KeyEvent, Spring, Tx, col, el, row};

use super::{MainUi, View};
use crate::ui::theme::{radius, Utilities};
use crate::ui::{kit, theme_for};

pub const SIDEBAR_W: f32 = 224.0;

/// `w-[clamp(260px,27vw,320px)]` for a window `width` wide.
pub fn list_width(width: f32) -> f32 {
    (width * 0.27).clamp(260.0, 320.0)
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    if let Some(which) = kit::gallery::requested() {
        let page = kit::gallery::view(which, s, cx);
        return crate::ui::kit::root(page, kit::toast::region(&s.toasts, cx), cx);
    }
    let th = theme_for(cx);
    let t = &th.t;
    // `if (!ready) return <div className="h-full bg-background" />`.
    if !s.ready {
        return el().key("main-loading").w_full().h_full().fill(t.background);
    }
    if !s.settings.onboarding_completed {
        let mut page = el().key("onboarding-root").w_full().h_full().child(super::onboarding::view(s, cx));
        if s.dev {
            page = page.child(kit_dev_badge(cx));
        }
        return crate::ui::kit::root(page, kit::toast::region(&s.toasts, cx), cx);
    }
    let width = cx.layout_size("main-root").map_or(1180.0, |sz| sz[0]);
    // Tooltips, menus and dialogs the page opens, mounted above it.
    let mut layer: Vec<El> = Vec::new();
    let content = if s.view.in_library() {
        let editor = match s.selected_snippet() {
            Some(_) => crate::ui::editor::view(s, cx),
            None => crate::ui::editor::empty(s, cx),
        };
        row()
            .w_full()
            .h_full()
            .child(
                el().key("list-pane").w(list_width(width)).h_full().min_h(0.0).border_sides([0.0, 1.0, 0.0, 0.0], t.hairline)
                    .child(super::snippet_list::view(s, cx, &mut layer)),
            )
            .child(el().key("editor-pane").grow(1.0).min_w(0.0).h_full().min_h(0.0).child(editor))
    } else {
        match s.view {
            View::Insights => super::insights::view(s, cx),
            View::Clipboard(_) => crate::ui::clipboard::view::view_with(s, cx, &mut layer),
            _ => super::settings::view(s, cx),
        }
    };
    let main = el()
        .key("main-card")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .mt(8.0)
        .mb(8.0)
        .mr(8.0)
        .ml(if s.sidebar_open { 0.0 } else { 8.0 })
        .radius(radius::XXL)
        .clip_rounded(radius::XXL)
        .fill(t.background)
        .elevate(&t.elev_card)
        // The web lays the frame out without transitions (the sidebar toggles at once).
        .motion(Spring::NONE)
        .child(content);
    let mut body = row().key("main-body").w_full().grow(1.0).min_h(0.0);
    if s.sidebar_open {
        body = body.child(el().key("sidebar-pane").w(SIDEBAR_W).h_full().min_h(0.0).child(super::sidebar::view(s, cx, &mut layer)));
    }
    body = body.child(main);
    let page = col()
        .key("main-root")
        .w_full()
        .h_full()
        .min_h(0.0)
        .fill(th.sidebar_vibrant)
        .child(super::toolbar::view(s, cx, &mut layer))
        .child(body);
    // Sections' own overlays (selects, alerts, popovers inside the clipped card) mount at the
    // window root first; then the library's menus and tooltips, the dialogs, and toasts last.
    let sections = super::settings::overlays(cx);
    super::command_menu::view(s, cx, &mut layer);
    super::shortcuts::view(s, cx, &mut layer);
    super::group_settings::view(s, cx, &mut layer);
    let overlays = el()
        .key("main-overlays")
        .w_full()
        .h_full()
        .child(sections)
        .children(layer)
        .child(super::library::drag::ghost(th, s))
        .child(kit::toast::region(&s.toasts, cx));
    crate::ui::kit::root(page, overlays, cx)
}

fn kit_dev_badge(cx: &mut Cx<MainUi>) -> El {
    crate::ui::kit::badge::dev(cx)
}

struct Keys {
    command: Chord,
    new: Chord,
    new_group: Chord,
    find: Chord,
    list: Chord,
    editor: Chord,
    views: [Chord; 5],
    settings: Chord,
    help: Chord,
    pause: Chord,
    duplicate: Chord,
    favorite: Chord,
    enable: Chord,
    delete: Chord,
    quit: Chord,
}

fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| {
        let c = |s: &str| Chord::parse(s).expect("chord");
        Keys {
            command: c("mod+k"),
            new: c("mod+n"),
            new_group: c("mod+shift+n"),
            find: c("mod+f"),
            list: c("mod+l"),
            editor: c("mod+e"),
            views: [c("mod+1"), c("mod+2"), c("mod+3"), c("mod+4"), c("mod+5")],
            settings: c("mod+comma"),
            help: c("mod+slash"),
            pause: c("mod+shift+p"),
            duplicate: c("mod+d"),
            favorite: c("mod+shift+f"),
            enable: c("mod+shift+e"),
            delete: c("mod+backspace"),
            quit: c("mod+q"),
        }
    })
}

/// `useHotkeys` in main-app.tsx. Each binding there was `allowInInput` unless noted.
pub fn on_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let k = keys();
    // ⌘Q closes Kudzu's windows and keeps it running in the menu bar.
    if k.quit.matches_tx(ev, tx) {
        crate::backend::windows::hide_all();
        return true;
    }
    if !s.settings.onboarding_completed {
        return false;
    }
    // Clipboard history's own keys (⌘P, ⌘S, ⌘O, ⌘R, ⌘⌫) when nothing has focus.
    if matches!(s.view, View::Clipboard(_)) && crate::ui::clipboard::view::on_key(s, ev, tx) {
        return true;
    }
    let hit = |c: &Chord| c.matches_tx(ev, tx);
    let in_library = s.view.in_library();
    if hit(&k.command) {
        s.command_open = true;
    } else if hit(&k.new) {
        s.new_snippet(Default::default());
    } else if hit(&k.new_group) {
        s.create_group("New group", None);
    } else if hit(&k.find) {
        if matches!(s.view, View::Clipboard(_)) {
            crate::ui::clipboard::view::focus_search(s, tx);
        } else {
            if !in_library {
                s.view = View::All;
            }
            s.list.focus_filter(tx);
        }
    } else if hit(&k.list) {
        if matches!(s.view, View::Clipboard(_)) {
            crate::ui::clipboard::view::focus_list(s, tx);
        } else {
            s.list.focus_list(tx);
        }
    } else if hit(&k.editor) && in_library && s.selected.is_some() {
        crate::ui::editor::focus(s, tx);
    } else if let Some(i) = k.views.iter().position(hit) {
        s.view = [View::All, View::Favorites, View::Recent, View::Insights, View::Clipboard(Default::default())][i].clone();
    } else if hit(&k.settings) {
        s.view = View::Settings(None);
    } else if hit(&k.help) || (tx.focused().is_none() && ev.key == rune::Key::Text("?".into())) {
        s.shortcuts_open = true;
    } else if hit(&k.pause) {
        let paused = !s.status.paused;
        s.set_paused(paused);
    } else if hit(&k.duplicate) && in_library && s.selected.is_some() {
        if let Some(id) = s.selected {
            s.duplicate(id);
        }
    } else if hit(&k.favorite) && in_library && s.selected.is_some() {
        if let Some(id) = s.selected {
            s.toggle_favorite(id);
        }
    } else if hit(&k.enable) && in_library && s.selected.is_some() {
        if let Some(id) = s.selected {
            s.toggle_enabled(id);
        }
    } else if hit(&k.delete) && in_library && !in_field(tx) {
        let ids = s.list.selection(s);
        s.request_delete(ids, tx);
    } else if in_library && s.list.focused && tx.focused().is_none() {
        // ⌘L (or Escape from the editor) handed the list the keyboard before a click focused it.
        return super::snippet_list::on_key(s, ev, tx);
    } else {
        return false;
    }
    true
}

/// `mod+backspace` isn't `allowInInput`: a text field keeps ⌘⌫ (delete to line start).
fn in_field(tx: &Tx) -> bool {
    tx.focused().is_some_and(|k| k.ends_with("-input") || k == "input")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_column_clamps_like_the_css() {
        assert_eq!(list_width(900.0), 260.0);
        assert_eq!(list_width(1100.0), 297.0);
        assert_eq!(list_width(1600.0), 320.0);
    }
}
