//! Main window frame: 42pt toolbar, 224pt vibrancy sidebar slot, 8pt gutters, elevated `<main>` card,
//! modal overlays, and centered toast host (`PLAN.md` S41).

use rune::{col, row, Curve, Cx, El, Tx};

use crate::main::state::{MainPane, MainView};
use crate::main::{
    cheatsheet, clipboard, command_menu, editor, group_settings, insights, library, onboarding,
    settings, sidebar, toolbar,
};
use crate::shell::MainState;
use crate::theme::{motion, radius, Theme};

/// Sidebar width when expanded (`w-56` = 224pt).
pub const SIDEBAR_W: f32 = 224.0;
/// Outer content card gutter (`pt-2 pb-2 pr-2`, plus `pl-2` when sidebar is collapsed = 8pt).
pub const CONTENT_GUTTER: f32 = 8.0;

/// Builds the full `main` window element tree (`AppSpec::view`).
pub fn view(s: &MainState, cx: &mut Cx<MainState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    let c = &theme.c;
    let mut overlays: Vec<El> = Vec::new();

    // First-run Onboarding occupies the full window without sidebar or toolbar (`main-app.tsx`).
    let base = if matches!(s.frame.view, MainView::Onboarding) {
        onboarding::view(s, theme)
    } else {
        let header = toolbar::view(s, cx, theme, &mut overlays);
        let sidebar_slot = build_sidebar_slot(s, theme);
        let main_card = build_main_card(s, theme, cx.is_reduced_motion());

        let body = row()
            .key("main-body")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(sidebar_slot)
            .child(main_card);

        col()
            .key("main-window-frame")
            .w_full()
            .h_full()
            .child(header)
            .child(body)
    };

    command_menu::view(s, theme, &mut overlays);
    cheatsheet::view(s, theme, &mut overlays);
    group_settings::view(s, theme, &mut overlays);
    if let Some(toast_layer) = s.frame.toasts.view(theme, |st: &mut MainState| {
        let now = st.frame.clock_secs;
        (&mut st.frame.toasts, now)
    }) {
        overlays.push(toast_layer);
    }

    let bg = if s.frame.sidebar_open && !matches!(s.frame.view, MainView::Onboarding) {
        c.sidebar
    } else {
        c.background
    };

    rune_kit::root(base.fill(bg), overlays)
}

fn build_sidebar_slot(s: &MainState, theme: &Theme) -> El {
    let open = s.frame.sidebar_open;
    let width = if open { SIDEBAR_W } else { 0.0 };
    let pane_focused = open && s.frame.focus.is_active(MainPane::Sidebar);

    let mut aside = col()
        .key("main-sidebar")
        .role("complementary")
        .label("Sidebar")
        .w(width)
        .motion(motion::soft_spring(motion::MODERATE))
        .h_full()
        .min_h(0.0)
        .clip_corners([0.0; 4])
        .opacity(if open { 1.0 } else { 0.0 })
        .opacity_motion(Curve::ease_in_out(motion::FAST))
        .on_drawn_layout_tx(|st: &mut MainState, r, tx: &mut Tx| {
            st.sync_sidebar_material([r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64]);
            tx.live_only();
        })
        .child(sidebar::view(s, theme));

    if !open {
        aside = aside.inert().a11y_hidden();
    } else if pane_focused {
        aside = aside.focus_selected(true);
    }
    aside
}

fn build_main_card(s: &MainState, theme: &Theme, reduced_motion: bool) -> El {
    let c = &theme.c;
    let pad_left = if s.frame.sidebar_open {
        0.0
    } else {
        CONTENT_GUTTER
    };

    let list_focused = s.frame.focus.is_active(MainPane::List);
    let editor_focused = s.frame.focus.is_active(MainPane::Editor);

    let screen = match &s.frame.view {
        MainView::Library(filter) => row()
            .key("library-split")
            .w_full()
            .h_full()
            .min_w(0.0)
            .min_h(0.0)
            .child(library::view(s, theme, filter, list_focused))
            .child(editor::view(s, theme, editor_focused)),
        MainView::Clipboard(filter) => clipboard::view(s, theme, *filter, list_focused),
        MainView::Settings(pane) => settings::view(s, theme, *pane),
        MainView::Insights => insights::view(s, theme, reduced_motion),
        MainView::Onboarding => onboarding::view(s, theme),
    };

    let card = theme.depth.card.iter().fold(
        col()
            .key("main-content")
            .role("main")
            .w_full()
            .h_full()
            .min_w(0.0)
            .min_h(0.0)
            .radius(radius::XL2)
            .clip_rounded(radius::XL2)
            // `bg-background shadow-card` (`main-app.tsx:190`): `depth.card` already draws the
            // 1 pt ring, so no border.
            .fill(c.background)
            .child(screen),
        |el, shadow| el.box_shadow(*shadow),
    );

    col()
        .key("main-content-gutter")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .pt(CONTENT_GUTTER)
        .pb(CONTENT_GUTTER)
        .pr(CONTENT_GUTTER)
        .pl(pad_left)
        .child(card)
}
