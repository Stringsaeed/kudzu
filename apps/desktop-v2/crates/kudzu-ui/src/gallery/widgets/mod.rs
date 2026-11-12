//! One gallery scene per S07 widget (`widgets/`), keyed for `cargo xtask parity` as the
//! component scenes are: `parity:` samples rebuild a reference landmark (the breadcrumb trails,
//! the segmented controls, the steppers, the settings group), `spec:` samples give the size the
//! Tauri classes set, `info:` samples are reported for side-by-side review.

mod apps;
mod controls;
mod panes;
mod text;

use rune::{col, text as label, Cx, El};

use super::{GalleryState, Page};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    Breadcrumb,
    SelectionPill,
    Slider,
    Stepper,
    Shortcut,
    TriggerChip,
    KeyHints,
    FormattedText,
    SecretText,
    EmptyState,
    SettingsCard,
    Wordmark,
    AppIcon,
    AppPicker,
}

macro_rules! scenes {
    ($($name:literal => $w:ident),* $(,)?) => {
        /// Scene name and setup (`--scene <name>`).
        pub const SCENES: &[(&str, fn(&mut GalleryState))] = &[
            $(($name, |s: &mut GalleryState| s.page = Page::Widget(Widget::$w)),)*
        ];

        impl Widget {
            pub fn scene(self) -> &'static str {
                match self { $(Widget::$w => $name,)* }
            }
        }
    };
}

scenes! {
    "breadcrumb" => Breadcrumb,
    "selection-pill" => SelectionPill,
    "slider" => Slider,
    "stepper" => Stepper,
    "shortcut" => Shortcut,
    "trigger-chip" => TriggerChip,
    "key-hints" => KeyHints,
    "formatted-text" => FormattedText,
    "secret-text" => SecretText,
    "empty-state" => EmptyState,
    "settings-card" => SettingsCard,
    "wordmark" => Wordmark,
    "app-icon" => AppIcon,
    "app-picker" => AppPicker,
}

pub fn view(w: Widget, s: &GalleryState, cx: &Cx<GalleryState>, theme: &Theme) -> El {
    let mut layer = Vec::new();
    let body = match w {
        Widget::Breadcrumb => controls::breadcrumb(theme, &mut layer),
        Widget::SelectionPill => controls::selection_pill(theme, cx),
        Widget::Slider => controls::slider(theme),
        Widget::Stepper => controls::stepper(theme, s),
        Widget::Shortcut => text::shortcut(theme),
        Widget::TriggerChip => text::trigger_chip(theme),
        Widget::KeyHints => text::key_hints(theme),
        Widget::FormattedText => text::formatted_text(theme),
        Widget::SecretText => panes::secret_text(theme),
        Widget::EmptyState => panes::empty_state(theme),
        Widget::SettingsCard => panes::settings_card(theme),
        Widget::Wordmark => text::wordmark(theme),
        Widget::AppIcon => apps::app_icon(theme),
        Widget::AppPicker => apps::app_picker(theme, s, cx, &mut layer),
    };
    let page = col()
        .key(format!("scene-{}", w.scene()))
        .w_full()
        .h_full()
        .padding(32.0)
        .gap(28.0)
        .fill(theme.c.background)
        .child(
            label(w.scene())
                .font(theme.ty.title)
                .color(theme.c.foreground),
        )
        .child(body);
    rune_kit::root(page, layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn scene(s: &GalleryState, cx: &mut Cx<GalleryState>) -> El {
        let Page::Widget(w) = s.page else {
            panic!("not a widget scene")
        };
        let theme = Theme::resolve(crate::theme::Appearance::System, cx.system_dark());
        view(w, s, cx, theme)
    }

    #[test]
    fn every_widget_scene_builds_in_both_themes() {
        for (name, setup) in SCENES {
            for dark in [false, true] {
                let mut s = GalleryState::default();
                setup(&mut s);
                let Page::Widget(w) = s.page else {
                    panic!("{name} is not a widget scene")
                };
                assert_eq!(w.scene(), *name);
                let mut rt = Runtime::new(s, scene, (1160.0, 900.0), Env::new(true, 1.0));
                rt.set_system_dark(dark);
                rt.frame(0.0);
                rt.frame(1.0 / 60.0);
            }
        }
    }
}
