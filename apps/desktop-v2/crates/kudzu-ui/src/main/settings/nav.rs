//! Settings sections in the reference order.
use crate::{
    main::{settings_pane_label, MainView},
    shell::{MainState, SettingsPane},
    theme::Theme,
};
use rune::{col, text, El, Key, KeyEvent, Tx};
pub const PANES: [SettingsPane; 8] = [
    SettingsPane::General,
    SettingsPane::Expansion,
    SettingsPane::Activity,
    SettingsPane::Clipboard,
    SettingsPane::Agents,
    SettingsPane::Apps,
    SettingsPane::Data,
    SettingsPane::About,
];
pub fn view(t: &Theme, current: SettingsPane) -> El {
    let mut nav = col()
        .key("settings-nav")
        .role("navigation")
        .label("Settings sections")
        .w(192.0)
        .h_full()
        .px(10.0)
        .gap(1.0)
        .child(
            col().h(48.0).px(10.0).justify_center().child(
                text("Settings")
                    .font(t.ty.body_strong)
                    .color(t.c.foreground),
            ),
        );
    for pane in PANES {
        nav = nav.child(
            col()
                .key(format!("settings-nav-{pane:?}"))
                .role("button")
                .label(settings_pane_label(pane))
                .h(28.0)
                .px(10.0)
                .justify_center()
                .radius(6.0)
                .fill(if pane == current {
                    t.c.accent
                } else {
                    rune::Color::TRANSPARENT
                })
                .on_press(move |s: &mut MainState| {
                    s.blur_all_inputs();
                    s.navigate_to(MainView::Settings(pane));
                })
                .child(
                    text(settings_pane_label(pane))
                        .font(t.ty.body)
                        .color(t.c.foreground),
                ),
        );
    }
    nav
}
pub fn handle_key(s: &mut MainState, e: &KeyEvent, _tx: &mut Tx) -> bool {
    if s.settings.is_composing() {
        return false;
    }
    if super::general::record_key(s, e) {
        return true;
    }
    if s.settings.is_editing_text() || e.mods.cmd || e.mods.ctrl || e.mods.alt || e.mods.shift {
        return false;
    }
    let MainView::Settings(pane) = s.frame.view else {
        return false;
    };
    let delta = match e.key {
        Key::Up => 7,
        Key::Down => 1,
        _ => return false,
    };
    let i = PANES.iter().position(|p| *p == pane).unwrap_or(0);
    s.navigate_to(MainView::Settings(PANES[(i + delta) % 8]));
    true
}
