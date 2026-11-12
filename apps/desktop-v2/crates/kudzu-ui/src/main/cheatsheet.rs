//! Keyboard shortcuts cheat sheet (`⌘/` or `?`). A 720-point panel centred in the
//! 16-point window inset.
//!
//! Chords come from the action registry and from each screen's shortcut metadata.
//! This file only places a row in a section and keeps the label the sheet shows.
//! The body is `min(640, 75vh)`, which is 570 at a 760-point window, so the panel
//! stays 624.422 there. Rune's `gap` is one value for both axes, so the columns
//! are two fixed stacks with a 40-point row gap and a 24-point section gap.

use rune::{col, el, icon, row, text, El, ScrollSpec, TextStyle};

use crate::actions::main::MainAction;
use crate::actions::menu;
use crate::actions::Action;
use crate::main::shortcut_meta::LocalShortcut;
use crate::main::{clipboard, editor, library};
use crate::shell::MainState;
use crate::theme::icons;
use crate::theme::{radius, Theme};
use crate::widgets::shortcut::Shortcut;

const PANEL_W: f32 = 720.0;
const HEADER_H: f32 = 54.422;
const TITLE_ROW_H: f32 = 53.422;
const TITLE_LH: f32 = 21.422;
const BODY_CAP: f32 = 640.0;
const COL_W: f32 = 316.0;
const HEADING_H: f32 = 15.703;
const ROW_H: f32 = 28.0;
/// Navigate's eight rows. Everywhere has seven and stretches to this height.
const TALL: f32 = 247.703;
const MID: f32 = 219.703;
const SHORT: f32 = 191.703;

enum Entry {
    Action(MainAction),
    Local(&'static LocalShortcut),
}

struct Section {
    key: &'static str,
    title: &'static str,
    height: f32,
    rows: Vec<Entry>,
}

fn action_rows(actions: &[MainAction]) -> Vec<Entry> {
    actions.iter().copied().map(Entry::Action).collect()
}

fn sections() -> [Section; 5] {
    let everywhere = action_rows(&[
        MainAction::CommandMenu,
        MainAction::NewSnippet,
        MainAction::NewGroup,
        MainAction::FocusFilter,
        MainAction::TogglePause,
        MainAction::OpenSettings,
        MainAction::OpenShortcuts,
    ]);
    let mut navigate = action_rows(&[
        MainAction::ViewAll,
        MainAction::ViewFavorites,
        MainAction::ViewRecent,
        MainAction::ViewInsights,
        MainAction::ViewClipboard,
        MainAction::FocusList,
        MainAction::EditContent,
    ]);
    navigate.push(Entry::Local(&editor::BACK_TO_LIST));
    let mut snippet_list: Vec<_> = library::LIST_SHORTCUTS.iter().map(Entry::Local).collect();
    snippet_list.push(Entry::Action(MainAction::DeleteSnippet));
    let clipboard_rows = clipboard::SCREEN_SHORTCUTS
        .iter()
        .map(Entry::Local)
        .collect();
    let mut editor_rows = vec![Entry::Local(&editor::INSERT_VARIABLE)];
    editor_rows.extend(action_rows(&[
        MainAction::DuplicateSnippet,
        MainAction::ToggleFavorite,
        MainAction::ToggleEnabled,
    ]));
    editor_rows.extend(editor::EDITOR_SHORTCUTS.iter().map(Entry::Local));
    [
        Section {
            key: "everywhere",
            title: "Everywhere",
            height: TALL,
            rows: everywhere,
        },
        Section {
            key: "navigate",
            title: "Navigate",
            height: TALL,
            rows: navigate,
        },
        Section {
            key: "snippet-list",
            title: "Snippet list",
            height: MID,
            rows: snippet_list,
        },
        Section {
            key: "clipboard",
            title: "Clipboard history",
            height: MID,
            rows: clipboard_rows,
        },
        Section {
            key: "editor",
            title: "Editor",
            height: SHORT,
            rows: editor_rows,
        },
    ]
}

fn footer() -> Vec<Entry> {
    action_rows(&[
        MainAction::Back,
        MainAction::Forward,
        MainAction::ToggleSidebar,
    ])
}

fn entry_label(entry: &Entry) -> &'static str {
    match entry {
        Entry::Action(action) => action.label(),
        Entry::Local(shortcut) => shortcut.label,
    }
}

/// Every label the sheet renders, in section order.
pub fn shortcut_labels() -> Vec<&'static str> {
    let mut labels: Vec<_> = sections()
        .iter()
        .flat_map(|section| section.rows.iter().map(entry_label))
        .collect();
    labels.extend(footer().iter().map(entry_label));
    labels
}

/// `⌘/` and `?` only open the sheet. Its own `⌘/` and Escape close it.
pub fn open(s: &mut MainState) {
    s.frame.shortcuts_open = true;
}

pub fn view(s: &MainState, theme: &Theme, viewport_h: f32, overlays: &mut Vec<El>) {
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
        .child(panel(s, theme, viewport_h));
    overlays.push(scrim);
    overlays.push(layer);
}

fn body_height(viewport_h: f32) -> f32 {
    BODY_CAP.min(viewport_h * 0.75)
}

fn panel(s: &MainState, theme: &Theme, viewport_h: f32) -> El {
    let mut panel = col()
        .key("shortcuts-cheatsheet-dialog")
        .role("dialog")
        .label("Keyboard shortcuts")
        .description("Every shortcut in Kudzu.")
        .w(PANEL_W)
        .min_w(PANEL_W)
        .max_w(PANEL_W)
        .radius(radius::XL2)
        .fill(theme.c.popover)
        .child(header(s, theme))
        .child(body(theme, viewport_h))
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
                .tone(crate::widgets::shortcut::Tone::Accent)
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
                    .tone(crate::widgets::shortcut::Tone::Accent)
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

fn body(theme: &Theme, viewport_h: f32) -> El {
    let sections = sections();
    // Everywhere stretches beside Navigate. Snippet list sits beside Clipboard.
    // Editor is the third left-hand region. A uniform grid gap cannot do 40 by 24.
    let left = [&sections[0], &sections[2], &sections[4]];
    let right = [&sections[1], &sections[3]];
    let columns = row()
        .w_full()
        .gap(40.0)
        .items_start()
        .child(column(theme, &left))
        .child(column(theme, &right));
    col()
        .w_full()
        .h(body_height(viewport_h))
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
                .child(footer_block(theme)),
        )
}

fn column(theme: &Theme, sections: &[&Section]) -> El {
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
        .children(section.rows.iter().map(|entry| sheet_row(theme, entry)))
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

fn sheet_row(theme: &Theme, entry: &Entry) -> El {
    let (label, chord, key) = match entry {
        Entry::Action(action) => (
            action.label(),
            menu::shown_chord(*action),
            format!("shortcuts-action-{action:?}"),
        ),
        Entry::Local(shortcut) => (
            shortcut.label,
            shortcut.chord.to_string(),
            format!("shortcuts-local-{}", shortcut.id),
        ),
    };
    row()
        .h(ROW_H)
        .w_full()
        .center_y()
        .justify_between()
        .gap(16.0)
        .child(
            text(label)
                .font(theme.ty.body)
                .color(theme.c.foreground.alpha(0.85))
                .truncate(),
        )
        .child(Shortcut::combo(&chord).build(theme).key(key))
}

fn footer_block(theme: &Theme) -> El {
    col()
        .w(COL_W)
        .pt(24.0)
        .children(footer().iter().map(|entry| sheet_row(theme, entry)))
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
