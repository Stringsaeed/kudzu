//! Reusable sidebar navigation rows and counts.

use crate::main::state::{ClipFilter, LibraryFilter, MainView};
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use rune::{icon, row, text, El};

pub(super) fn item(label: impl Into<String>, target: MainView, theme: &Theme) -> El {
    let label = label.into();
    let key = format!("sidebar-item-{}", label.to_lowercase().replace(' ', "-"));
    let glyph = match &target {
        MainView::Library(LibraryFilter::All) => &icons::LAYERS,
        MainView::Library(LibraryFilter::Favorites) => &icons::STAR,
        MainView::Library(LibraryFilter::Recent) => &icons::CLOCK_3,
        MainView::Library(LibraryFilter::Disabled) => &icons::CIRCLE_OFF,
        MainView::Library(LibraryFilter::Group(_)) => &icons::FOLDER,
        MainView::Clipboard(ClipFilter::All) => &icons::CLIPBOARD_LIST,
        MainView::Clipboard(ClipFilter::Pinned) => &icons::PIN,
        MainView::Clipboard(ClipFilter::Text | ClipFilter::Other) => &icons::ALIGN_LEFT,
        MainView::Clipboard(ClipFilter::Image) => &icons::IMAGE,
        MainView::Clipboard(ClipFilter::Link) => &icons::GLOBE,
        MainView::Clipboard(ClipFilter::File) => &icons::FILE,
        MainView::Clipboard(ClipFilter::Code) => &icons::BRACES,
        MainView::Clipboard(ClipFilter::Color) => &icons::PALETTE,
        MainView::Insights => &icons::BAR_CHART_3,
        MainView::Settings(_) => &icons::SETTINGS,
        MainView::Onboarding => &icons::LAYERS,
    };
    row()
        .key(key.clone())
        .role("button")
        .label(label.clone())
        .focusable(true)
        .w_full()
        .h(28.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .on_press_tx(move |st: &mut MainState, tx| {
            st.navigate_to(target.clone());
            st.blur_all_inputs();
            st.frame.focus.move_to(crate::main::MainPane::Sidebar);
            tx.focus(key.clone(), false);
        })
        .child(
            icon(glyph, 16.0)
                .color(theme.c.sidebar_foreground)
                .a11y_hidden(),
        )
        .child(
            text(label)
                .font(theme.ty.control_sm)
                .color(theme.c.sidebar_foreground)
                .grow(1.0),
        )
}

pub(super) fn count(key: String, value: usize, theme: &Theme) -> El {
    if value == 0 {
        return rune::el().size(0.0, 0.0).a11y_hidden();
    }
    text(value.to_string())
        .key(key)
        .font(theme.ty.control_sm)
        .color(theme.c.faint)
}

pub(super) fn empty_groups(theme: &Theme) -> El {
    row()
        .key("sidebar-empty-groups")
        .role("button")
        .label("Organize snippets into groups — like Work, Email or Code.")
        .w_full()
        .h(56.0)
        .padding(8.0)
        .on_press(super::actions::new_group_from_button)
        .child(
            text("Organize snippets into groups — like Work, Email or Code.")
                .font(theme.ty.control_sm)
                .color(theme.c.muted_foreground),
        )
}
