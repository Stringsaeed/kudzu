//! Sidebar navigation; group management remains to be built.

pub mod actions;

use rune::{col, row, text, El};

use crate::main::nav;
use crate::main::state::{clip_filter_label, ClipFilter, LibraryFilter, MainView, CLIP_FOLDERS};
use crate::shell::{MainState, SettingsPane};
use crate::theme::Theme;

#[derive(Clone, Debug, Default)]
pub struct State {}

impl State {
    pub fn is_composing(&self) -> bool {
        false
    }

    pub fn is_editing_text(&self) -> bool {
        false
    }

    pub fn blur_inputs(&mut self) {}
}

fn item(label: &'static str, target: MainView, theme: &Theme) -> El {
    row()
        .role("button")
        .label(label)
        .focusable(true)
        .w_full()
        .h(28.0)
        .px(8.0)
        .center_y()
        .on_press_tx(move |st: &mut MainState, tx| nav::view(st, target.clone(), tx))
        .child(
            text(label)
                .font(theme.ty.control_sm)
                .color(theme.c.sidebar_foreground),
        )
}

pub fn view(s: &MainState, theme: &Theme) -> El {
    let history = s.svc.clipboard.watch().get();
    let mut clipboard = col();
    for filter in CLIP_FOLDERS {
        let active = s.frame.view == MainView::Clipboard(filter);
        if matches!(filter, ClipFilter::All | ClipFilter::Pinned)
            || active
            || history.entries.iter().any(|entry| filter.contains(entry))
        {
            clipboard = clipboard.child(item(
                clip_filter_label(filter),
                MainView::Clipboard(filter),
                theme,
            ));
        }
    }
    col()
        .key("sidebar-nav")
        .role("navigation")
        .label("Library")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(10.0)
        .py(8.0)
        .gap(4.0)
        .child(text("Snippets").font(theme.ty.label).color(theme.c.faint))
        .child(item(
            "All snippets",
            MainView::Library(LibraryFilter::All),
            theme,
        ))
        .child(item(
            "Favorites",
            MainView::Library(LibraryFilter::Favorites),
            theme,
        ))
        .child(item(
            "Recently used",
            MainView::Library(LibraryFilter::Recent),
            theme,
        ))
        .child(text("Clipboard").font(theme.ty.label).color(theme.c.faint))
        .child(clipboard)
        .child(item("Insights", MainView::Insights, theme))
        .child(item(
            "Settings",
            MainView::Settings(SettingsPane::General),
            theme,
        ))
}
