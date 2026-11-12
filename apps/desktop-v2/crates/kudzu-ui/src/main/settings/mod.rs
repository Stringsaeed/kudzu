//! Settings frame and pane-owned UI drafts. Services remain the source of truth.
pub mod about;
pub mod activity;
pub mod agents;
pub mod apps;
pub mod clipboard;
pub mod data;
pub mod expansion;
pub mod general;
pub mod nav;
pub mod scenes;
use crate::{
    shell::{MainState, SettingsPane},
    theme::Theme,
};
use rune::{col, row, text, El, ScrollSpec};
#[derive(Default)]
pub struct State {
    pub error: Option<String>,
    pub prefix: rune::TextEdit,
    pub recording: bool,
    pub captured_until: Option<std::time::Duration>,
}
impl State {
    pub fn is_composing(&self) -> bool {
        self.prefix.is_composing()
    }
    pub fn is_editing_text(&self) -> bool {
        self.prefix.focused || self.recording
    }
    pub fn blur_inputs(&mut self) {
        self.prefix.focused = false;
        self.recording = false;
    }
}
pub fn view(s: &MainState, t: &Theme, pane: SettingsPane) -> El {
    let body = match pane {
        SettingsPane::General => general::view(s, t),
        SettingsPane::Expansion => expansion::view(s, t),
        SettingsPane::Apps => apps::view(s, t),
        SettingsPane::Activity => activity::view(s, t),
        SettingsPane::Clipboard => clipboard::view(s, t),
        SettingsPane::Agents => agents::view(s, t),
        SettingsPane::Data => data::view(s, t),
        SettingsPane::About => about::view(s, t),
    };
    row()
        .key("settings-view-slot")
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(nav::view(t, pane))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .h_full()
                .min_h(0.0)
                .child(
                    row()
                        .key("settings-heading")
                        .w_full()
                        .h(48.0)
                        .px(24.0)
                        .items_center()
                        .child(
                            text(crate::main::settings_pane_label(pane))
                                .font(t.ty.body_strong)
                                .color(t.c.foreground),
                        ),
                )
                .child(
                    col()
                        .key("settings-scroll")
                        .grow(1.0)
                        .w_full()
                        .min_h(0.0)
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(
                            col()
                                .key("settings-body")
                                .w_full()
                                .max_w(680.0)
                                .padding(32.0)
                                .gap(32.0)
                                .child(body),
                        ),
                ),
        )
}
pub(super) fn group(t: &Theme, title: &str, rows: Vec<El>) -> El {
    let mut card = col().w_full().px(16.0).radius(12.0).fill(t.c.surface);
    for r in rows {
        card = card.child(r);
    }
    col()
        .w_full()
        .gap(8.0)
        .child(
            text(title.to_owned())
                .font(t.ty.body_strong)
                .color(t.c.foreground),
        )
        .child(card)
}
pub(super) fn setting(t: &Theme, title: &str, hint: &str, control: El) -> El {
    row()
        .w_full()
        .min_h(56.0)
        .py(12.0)
        .gap(24.0)
        .items_center()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(
                    text(title.to_owned())
                        .font(rune::TextStyle::new(13.0, 500.0).lh(18.5625))
                        .color(t.c.foreground),
                )
                .child(
                    text(hint.to_owned())
                        .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                        .color(t.c.muted_foreground)
                        .wrap(),
                ),
        )
        .child(control)
}
pub(super) fn update(s: &mut MainState, edit: impl FnOnce(&mut kudzu_app::settings::Settings)) {
    if let Err(e) = s.svc.settings.update(edit) {
        s.settings.error = Some(e.to_string());
    }
}

pub fn sync(s: &mut MainState) -> bool {
    let mut changed = false;
    if s.settings
        .captured_until
        .is_some_and(|end| s.svc.clock.monotonic() >= end)
    {
        s.settings.captured_until = None;
        changed = true;
    }
    if !s.settings.prefix.is_composing() {
        if s.settings.prefix.focused {
            let prefix = expansion::limited_prefix(&s.settings.prefix.text);
            if prefix != s.settings.prefix.text {
                s.settings.prefix.text = prefix.clone();
                changed = true;
            }
            if s.svc.settings.watch().get().default_trigger_prefix != prefix {
                update(s, |p| p.default_trigger_prefix = prefix);
                changed = true;
            }
        } else {
            let prefix = s.svc.settings.watch().get().default_trigger_prefix.clone();
            if s.settings.prefix.text != prefix {
                s.settings.prefix = rune::TextEdit::new(prefix);
                changed = true;
            }
        }
    }
    changed
}
