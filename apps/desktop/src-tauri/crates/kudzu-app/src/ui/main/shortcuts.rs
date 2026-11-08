//! The keyboard shortcuts sheet (`components/app/main/shortcuts.tsx`, ⌘/ or ?): every shortcut in
//! Kudzu, two columns of sections, with the system-wide hotkeys in the header.

use rune::{col, el, row, text, Cx, El, ScrollSpec};

use super::MainUi;
use crate::ui::kit::dialog::Dialog;
use crate::ui::kit::popover::window_size;
use crate::ui::kit::shortcut::{accelerator_labels, Shortcut, Tone};
use crate::ui::theme::{radius, text as ty, weight, Theme, WeightOf};
use crate::ui::theme_for;

/// `SECTIONS`.
pub const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Everywhere",
        &[
            ("mod+k", "Search & commands"),
            ("mod+n", "New snippet"),
            ("mod+shift+n", "New group"),
            ("mod+f", "Filter snippets"),
            ("mod+shift+p", "Pause / resume expanding"),
            ("mod+,", "Settings"),
            ("mod+/", "This cheat sheet"),
        ],
    ),
    (
        "Navigate",
        &[
            ("mod+1", "All snippets"),
            ("mod+2", "Favorites"),
            ("mod+3", "Recently used"),
            ("mod+4", "Insights"),
            ("mod+5", "Clipboard history"),
            ("mod+l", "Focus the list"),
            ("mod+e", "Edit content"),
            ("escape", "Back to the list"),
        ],
    ),
    (
        "Snippet list",
        &[
            ("up", "Previous (or K)"),
            ("down", "Next (or J)"),
            ("shift+down", "Extend selection"),
            ("mod+a", "Select all"),
            ("enter", "Open in editor"),
            ("/", "Filter"),
            ("mod+backspace", "Delete (undoable)"),
        ],
    ),
    (
        "Clipboard history",
        &[
            ("enter", "Copy"),
            ("shift+enter", "Copy as plain text"),
            ("space", "Large preview"),
            ("mod+p", "Pin / unpin"),
            ("mod+s", "Save as snippet"),
            ("mod+o", "Open link or file"),
            ("mod+backspace", "Delete (undoable)"),
        ],
    ),
    (
        "Editor",
        &[
            ("/", "Insert a variable"),
            ("mod+d", "Duplicate snippet"),
            ("mod+shift+f", "Toggle favorite"),
            ("mod+shift+e", "Turn on / off"),
            ("mod+z", "Undo"),
            ("mod+s", "Save now (auto-saves anyway)"),
        ],
    ),
];

fn section(th: &Theme, title: &str, items: &[(&str, &str)]) -> El {
    let t = &th.t;
    let mut list = col().role("list");
    for (combo, label) in items {
        list = list.child(
            row()
                .h(28.0)
                .gap(16.0)
                .center_y()
                .justify_between()
                .child(text(*label).font(ty::BASE).color(Theme::tint(t.foreground, 0.85)))
                .child(Shortcut::new(*combo).build(th)),
        );
    }
    col()
        .role("group")
        .label(title.to_string())
        .grow(1.0)
        .min_w(0.0)
        .child(text(title.to_string()).font(ty::XS2.weight_of(weight::MEDIUM)).color(t.faint).mb(8.0).role("heading"))
        .child(list)
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) {
    if !s.shortcuts_open {
        return;
    }
    let th = theme_for(cx);
    let t = &th.t;

    let mut hotkeys = row()
        .ml_auto()
        .gap(8.0)
        .center_y()
        .child(text("From any app: search").font(ty::XS).color(t.muted_foreground))
        .child(Shortcut::keys(accelerator_labels(&s.settings.palette_hotkey)).tone(Tone::Accent).build(th));
    if s.settings.clipboard_enabled {
        hotkeys = hotkeys
            .child(text("·").font(ty::XS).color(t.faint))
            .child(text("clipboard").font(ty::XS).color(t.muted_foreground))
            .child(Shortcut::keys(accelerator_labels(&s.settings.clipboard_hotkey)).tone(Tone::Accent).build(th));
    }
    let header = row()
        .key("shortcuts-header")
        .gap(12.0)
        .px(24.0)
        .py(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .child(text("Keyboard shortcuts").font(ty::MD.weight_of(weight::SEMIBOLD)).color(t.foreground).role("heading"))
        .child(hotkeys);

    // `grid grid-cols-2 gap-x-10 gap-y-6`.
    let mut grid = col().key("shortcuts-grid").w_full().gap(24.0).px(24.0).py(20.0);
    for pair in SECTIONS.chunks(2) {
        let mut r = row().w_full().gap(40.0).items_start();
        for (title, items) in pair {
            r = r.child(section(th, title, items));
        }
        if pair.len() == 1 {
            r = r.child(el().grow(1.0).min_w(0.0));
        }
        grid = grid.child(r);
    }
    let max_h = 640f32.min(window_size(cx).1 * 0.75);
    let body = col().key("shortcuts-scroll").w_full().max_h(max_h).min_h(0.0).shrink().scroll(ScrollSpec { stick_bottom: false }).child(grid);

    Dialog::new("shortcuts")
        .open(true)
        .on_open_change(|s: &mut MainUi, open, _| s.shortcuts_open = open)
        .max_width(720.0)
        .padding(0.0)
        .gap(0.0)
        .radius(radius::XXL)
        .float()
        .ring(false)
        .label("Keyboard shortcuts")
        .description("Every shortcut in Kudzu.")
        .child(header)
        .child(body)
        .build(cx, layer);
}

/// `ml-auto`.
trait MlAuto {
    fn ml_auto(self) -> Self;
}

impl MlAuto for El {
    fn ml_auto(self) -> El {
        // A row child pushed to the end: grow into the free space and pack to the right.
        self.grow(1.0).justify_end()
    }
}
