//! One gallery scene per shadcn primitive the Tauri app renders, in the variants, sizes and
//! states S02's captures show, each drawn by its rune-kit component with Kudzu's `Kit` (or by
//! `widgets::kit_ext` where the matrix says so).
//!
//! Samples carry the keys `cargo xtask parity` measures (`xtask/src/parity.rs`):
//! `parity:<window>:<scene>:<role>:<occurrence>:<name>` reproduces a reference landmark,
//! `spec:<what>:<w>x<h>` gives the size the component's classes set, `info:<what>` is reported
//! only (rune-kit's own metrics, §6 D6). `docs/migration/PARITY_MATRIX.md` reads the report.

pub mod actions;
pub mod display;
pub mod forms;
pub mod overlays;

use rune::{col, row, text, El};

use super::{GalleryState, Page};
use crate::theme::Theme;

/// The shadcn primitives (`components/ui`) the Tauri app renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Button,
    Toggle,
    Switch,
    Checkbox,
    Input,
    Textarea,
    Label,
    Select,
    Popover,
    Tooltip,
    Dialog,
    AlertDialog,
    DropdownMenu,
    ContextMenu,
    Command,
    Sonner,
    Badge,
    Separator,
    Kbd,
    Item,
}

/// `components/ui` files no Tauri component imports: listed in the matrix, given no scene.
pub const UNUSED: [&str; 6] = [
    "collapsible",
    "radio-group",
    "scroll-area",
    "slider",
    "tabs",
    "toggle-group",
];

/// Rendered by a Kudzu composition (S07's `widgets::breadcrumbs`), which has its own scene.
pub const COMPOSED: [&str; 1] = ["breadcrumb"];

macro_rules! scenes {
    ($($name:literal => $p:ident),* $(,)?) => {
        /// Scene name and setup, one per primitive (`--scene <name>`).
        pub const SCENES: &[(&str, fn(&mut GalleryState))] = &[
            $(($name, |s: &mut GalleryState| s.page = Page::Component(Primitive::$p)),)*
        ];

        impl Primitive {
            pub fn scene(self) -> &'static str {
                match self { $(Primitive::$p => $name,)* }
            }
        }
    };
}

scenes! {
    "button" => Button,
    "toggle" => Toggle,
    "switch" => Switch,
    "checkbox" => Checkbox,
    "input" => Input,
    "textarea" => Textarea,
    "label" => Label,
    "select" => Select,
    "popover" => Popover,
    "tooltip" => Tooltip,
    "dialog" => Dialog,
    "alert-dialog" => AlertDialog,
    "dropdown-menu" => DropdownMenu,
    "context-menu" => ContextMenu,
    "command" => Command,
    "sonner" => Sonner,
    "badge" => Badge,
    "separator" => Separator,
    "kbd" => Kbd,
    "item" => Item,
}

/// The scene's view: the page and, for overlays, what is mounted above it.
pub fn view(p: Primitive, theme: &Theme) -> El {
    let mut layer = Vec::new();
    let body = match p {
        Primitive::Button => actions::button(theme),
        Primitive::Toggle => actions::toggle(theme),
        Primitive::Switch => forms::switch(theme),
        Primitive::Checkbox => forms::checkbox(theme),
        Primitive::Input => forms::input(theme),
        Primitive::Textarea => forms::textarea(theme),
        Primitive::Label => forms::label(theme),
        Primitive::Select => forms::select(theme, &mut layer),
        Primitive::Popover => overlays::popover(theme, &mut layer),
        Primitive::Tooltip => overlays::tooltip(theme, &mut layer),
        Primitive::Dialog => overlays::dialog(theme, &mut layer),
        Primitive::AlertDialog => overlays::alert_dialog(theme, &mut layer),
        Primitive::DropdownMenu => overlays::dropdown_menu(theme, &mut layer),
        Primitive::ContextMenu => overlays::context_menu(theme, &mut layer),
        Primitive::Command => overlays::command(theme, &mut layer),
        Primitive::Sonner => overlays::sonner(theme),
        Primitive::Badge => display::badge(theme),
        Primitive::Separator => display::separator(theme),
        Primitive::Kbd => display::kbd(theme),
        Primitive::Item => display::item(theme),
    };
    let page = col()
        .key(format!("scene-{}", p.scene()))
        .w_full()
        .h_full()
        .padding(32.0)
        .gap(28.0)
        .fill(theme.c.background)
        .child(
            text(p.scene())
                .font(theme.ty.title)
                .color(theme.c.foreground),
        )
        .child(body);
    rune_kit::root(page, layer)
}

/// A labelled line of samples.
pub(crate) fn line(theme: &Theme, caption: &str, samples: impl IntoIterator<Item = El>) -> El {
    col()
        .gap(10.0)
        .child(
            text(caption.to_uppercase())
                .font(theme.ty.label)
                .color(theme.c.muted_foreground),
        )
        .child(row().gap(14.0).center_y().children(samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_used_primitive_has_one_scene() {
        let names: Vec<&str> = SCENES.iter().map(|(n, _)| *n).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
        // 28 files in components/ui: 20 scenes, 6 unused, 1 composed by a Kudzu widget, and
        // `input-group`, which shares the `input` scene.
        assert_eq!(names.len() + UNUSED.len() + COMPOSED.len() + 1, 28);
        for (name, setup) in SCENES {
            let mut s = GalleryState::default();
            setup(&mut s);
            let Page::Component(p) = s.page else {
                panic!("{name} is not a component scene")
            };
            assert_eq!(p.scene(), *name);
            let _ = view(p, Theme::light());
        }
    }
}
