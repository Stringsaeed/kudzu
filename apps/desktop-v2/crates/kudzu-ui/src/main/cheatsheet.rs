//! Keyboard shortcuts cheat sheet (`⌘/` or `?`). A 720-point panel centred in the
//! 16-point window inset, listing every shortcut the registry publishes.
//!
//! Rune's `gap` is one value for both axes, so the columns are two fixed stacks with
//! a 40-point row gap and a 24-point section gap. The sheet is the captured 624.422
//! points tall (header plus a 570-point body), which is `min(640, 75vh)` at 760.

use rune::{col, el, icon, row, text, El, ScrollSpec, TextStyle};

use crate::shell::MainState;
use crate::theme::icons;
use crate::theme::{radius, Theme};
use crate::widgets::shortcut::{Shortcut, Tone};

const PANEL_W: f32 = 720.0;
const PANEL_H: f32 = 624.422;
const HEADER_H: f32 = 54.422;
const TITLE_ROW_H: f32 = 53.422;
const TITLE_LH: f32 = 21.422;
const BODY_H: f32 = 570.0;
const COL_W: f32 = 316.0;
const HEADING_H: f32 = 15.703;
const ROW_H: f32 = 28.0;
/// Navigate's eight rows. Everywhere has seven and stretches to this height.
const TALL: f32 = 247.703;
const MID: f32 = 219.703;
const SHORT: f32 = 191.703;

struct Item {
    combo: &'static str,
    label: &'static str,
}

#[derive(Clone, Copy)]
struct Section {
    key: &'static str,
    title: &'static str,
    height: f32,
    rows: &'static [Item],
}

const EVERYWHERE: &[Item] = &[
    Item {
        combo: "mod+k",
        label: "Search & commands",
    },
    Item {
        combo: "mod+n",
        label: "New snippet",
    },
    Item {
        combo: "mod+shift+n",
        label: "New group",
    },
    Item {
        combo: "mod+f",
        label: "Filter snippets",
    },
    Item {
        combo: "mod+shift+p",
        label: "Pause / resume expanding",
    },
    Item {
        combo: "mod+,",
        label: "Settings",
    },
    Item {
        combo: "mod+/",
        label: "Keyboard shortcuts",
    },
];

const NAVIGATE: &[Item] = &[
    Item {
        combo: "mod+1",
        label: "All snippets",
    },
    Item {
        combo: "mod+2",
        label: "Favorites",
    },
    Item {
        combo: "mod+3",
        label: "Recently used",
    },
    Item {
        combo: "mod+4",
        label: "Insights",
    },
    Item {
        combo: "mod+5",
        label: "Clipboard history",
    },
    Item {
        combo: "mod+l",
        label: "Focus the list",
    },
    Item {
        combo: "mod+e",
        label: "Edit content",
    },
    Item {
        combo: "escape",
        label: "Back to the list",
    },
];

const SNIPPET_LIST: &[Item] = &[
    Item {
        combo: "up",
        label: "Previous (or K)",
    },
    Item {
        combo: "down",
        label: "Next (or J)",
    },
    Item {
        combo: "shift+down",
        label: "Extend selection",
    },
    Item {
        combo: "mod+a",
        label: "Select all",
    },
    Item {
        combo: "enter",
        label: "Open in editor",
    },
    Item {
        combo: "/",
        label: "Filter",
    },
    Item {
        combo: "mod+backspace",
        label: "Delete snippet",
    },
];

const CLIPBOARD: &[Item] = &[
    Item {
        combo: "enter",
        label: "Copy",
    },
    Item {
        combo: "shift+enter",
        label: "Copy as plain text",
    },
    Item {
        combo: "space",
        label: "Large preview",
    },
    Item {
        combo: "mod+p",
        label: "Pin / unpin",
    },
    Item {
        combo: "mod+s",
        label: "Save as snippet",
    },
    Item {
        combo: "mod+o",
        label: "Open link or file",
    },
    Item {
        combo: "mod+backspace",
        label: "Delete (undoable)",
    },
];

const EDITOR: &[Item] = &[
    Item {
        combo: "/",
        label: "Insert a variable",
    },
    Item {
        combo: "mod+d",
        label: "Duplicate snippet",
    },
    Item {
        combo: "mod+shift+f",
        label: "Toggle favorite",
    },
    Item {
        combo: "mod+shift+e",
        label: "Turn on / off",
    },
    Item {
        combo: "mod+z",
        label: "Undo",
    },
    Item {
        combo: "mod+s",
        label: "Save now (auto-saves anyway)",
    },
];

const FOOTER: &[Item] = &[
    Item {
        combo: "mod+[",
        label: "Back",
    },
    Item {
        combo: "mod+]",
        label: "Forward",
    },
    Item {
        combo: "mod+\\",
        label: "Toggle sidebar",
    },
];

fn sections() -> [Section; 5] {
    [
        Section {
            key: "everywhere",
            title: "Everywhere",
            height: TALL,
            rows: EVERYWHERE,
        },
        Section {
            key: "navigate",
            title: "Navigate",
            height: TALL,
            rows: NAVIGATE,
        },
        Section {
            key: "snippet-list",
            title: "Snippet list",
            height: MID,
            rows: SNIPPET_LIST,
        },
        Section {
            key: "clipboard",
            title: "Clipboard history",
            height: MID,
            rows: CLIPBOARD,
        },
        Section {
            key: "editor",
            title: "Editor",
            height: SHORT,
            rows: EDITOR,
        },
    ]
}

/// Every label the sheet renders, from the same tables as the view.
pub fn shortcut_labels() -> Vec<&'static str> {
    let mut labels: Vec<_> = sections()
        .iter()
        .flat_map(|section| section.rows.iter().map(|item| item.label))
        .collect();
    labels.extend(FOOTER.iter().map(|item| item.label));
    labels
}

/// `⌘/` and `?` only open the sheet. Its own `⌘/` and Escape close it.
pub fn open(s: &mut MainState) {
    s.frame.shortcuts_open = true;
}

pub fn view(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) {
    if !s.frame.shortcuts_open {
        return;
    }
    let scrim = el()
        .key("shortcuts-cheatsheet-scrim")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(theme.kit().c.scrim)
        .a11y_hidden();
    let layer = el()
        .key("shortcuts-cheatsheet-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .padding(16.0)
        .center()
        .child(panel(s, theme));
    overlays.push(scrim);
    overlays.push(layer);
}

fn panel(s: &MainState, theme: &Theme) -> El {
    let mut panel = col()
        .key("shortcuts-cheatsheet-dialog")
        .role("dialog")
        .label("Keyboard shortcuts")
        .description("Every shortcut in Kudzu.")
        .w(PANEL_W)
        .min_w(PANEL_W)
        .max_w(PANEL_W)
        .h(PANEL_H)
        .min_h(PANEL_H)
        .max_h(PANEL_H)
        .radius(radius::XL2)
        .fill(theme.c.popover)
        .child(header(s, theme))
        .child(body(theme))
        .child(close_button(theme));
    for shadow in &theme.depth.float {
        panel = panel.box_shadow(*shadow);
    }
    panel.modal(true, |st: &mut MainState| st.frame.shortcuts_open = false)
}

fn header(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let settings = s.svc.settings.watch().get();
    let title = text("Keyboard shortcuts")
        .key("shortcuts-title")
        .role("heading")
        .label("Keyboard shortcuts")
        .font(TextStyle::new(15.0, 600.0).lh(TITLE_LH))
        .color(c.foreground);
    let mut hints = row()
        .gap(8.0)
        .center_y()
        .min_w(0.0)
        .shrink()
        .child(
            text("From any app: search")
                .font(TextStyle::new(12.0, 400.0))
                .color(c.muted_foreground),
        )
        .child(
            Shortcut::accelerator(&settings.palette_hotkey)
                .tone(Tone::Accent)
                .build(theme),
        );
    if settings.clipboard_enabled {
        hints = hints
            .child(text("·").font(TextStyle::new(12.0, 400.0)).color(c.faint))
            .child(
                text("clipboard")
                    .font(TextStyle::new(12.0, 400.0))
                    .color(c.muted_foreground),
            )
            .child(
                Shortcut::accelerator(&settings.clipboard_hotkey)
                    .tone(Tone::Accent)
                    .build(theme),
            );
    }
    col()
        .w_full()
        .h(HEADER_H)
        .child(
            row()
                .w_full()
                .h(TITLE_ROW_H)
                .px(24.0)
                .center_y()
                .child(title)
                .child(el().grow(1.0))
                .child(hints),
        )
        .child(el().h(1.0).w_full().fill(c.hairline))
}

fn body(theme: &Theme) -> El {
    let sections = sections();
    // Everywhere stretches beside Navigate. Snippet list sits beside Clipboard.
    // Editor is the third left-hand region. A uniform grid gap cannot do 40 by 24.
    let left = [sections[0], sections[2], sections[4]];
    let right = [sections[1], sections[3]];
    let columns = row()
        .w_full()
        .gap(40.0)
        .items_start()
        .child(column(theme, &left))
        .child(column(theme, &right));
    col()
        .w_full()
        .h(BODY_H)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .w_full()
                .px(24.0)
                .py(20.0)
                .child(columns)
                .child(footer(theme)),
        )
}

fn column(theme: &Theme, sections: &[Section]) -> El {
    col()
        .w(COL_W)
        .gap(24.0)
        .children(sections.iter().map(|section| region(theme, section)))
}

fn region(theme: &Theme, section: &Section) -> El {
    col()
        .key(format!("shortcuts-{}", section.key))
        .role("region")
        .label(section.title)
        .w(COL_W)
        .h(section.height)
        .max_h(section.height)
        .child(heading(theme, section))
        .child(el().h(8.0))
        .children(section.rows.iter().map(|item| sheet_row(theme, item)))
}

fn heading(theme: &Theme, section: &Section) -> El {
    el().key(format!("shortcuts-h-{}", section.key))
        .role("heading")
        .label(section.title)
        .w_full()
        .h(HEADING_H)
        .max_h(HEADING_H)
        .child(
            text(section.title)
                .font(TextStyle::new(11.0, 500.0).lh(HEADING_H))
                .color(theme.c.faint),
        )
}

fn sheet_row(theme: &Theme, item: &Item) -> El {
    row()
        .h(ROW_H)
        .w_full()
        .center_y()
        .justify_between()
        .gap(16.0)
        .child(
            text(item.label)
                .font(theme.ty.body)
                .color(theme.c.foreground.alpha(0.85))
                .truncate(),
        )
        .child(Shortcut::combo(item.combo).build(theme))
}

fn footer(theme: &Theme) -> El {
    col()
        .w(COL_W)
        .pt(24.0)
        .children(FOOTER.iter().map(|item| sheet_row(theme, item)))
}

fn close_button(theme: &Theme) -> El {
    row()
        .key("shortcuts-close")
        .role("button")
        .label("Close")
        .absolute(684.0, 8.0)
        .size(28.0, 28.0)
        .center()
        .radius(8.0)
        .on_press(|st: &mut MainState| st.frame.shortcuts_open = false)
        .child(icon(&icons::X, 16.0).color(theme.c.foreground))
}
