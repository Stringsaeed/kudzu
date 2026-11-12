//! Command menu rows. Section titles are plain text: the dialog's heading is the title.

use kudzu_core::library::groups::group_path;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use rune::{col, el, icon, row, text, El, ScrollSpec, TextStyle};
use uuid::Uuid;

use super::catalog::{Command, Hits, Kind};
use crate::actions::main::MainAction;
use crate::actions::menu::{self, MenuSection};
use crate::shell::MainState;
use crate::theme::icons;
use crate::theme::{radius, Theme};
use crate::widgets::shortcut::{Shortcut, Tone};
use crate::widgets::trigger_chip::trigger_chip;

const PANEL_W: f32 = 600.0;
const SEARCH_H: f32 = 48.0;
const LIST_CAP: f32 = 440.0;
const SECTION_H: f32 = 29.703;
const ROW_H: f32 = 36.0;
const LIST_PAD: f32 = 8.0;
const EMPTY_H: f32 = 56.0;

pub(super) fn panel(s: &MainState, theme: &Theme, viewport_h: f32, hits: &Hits) -> El {
    let c = &theme.c;
    let mut panel = col()
        .key("command-menu-dialog")
        .role("dialog")
        .label("Search and commands")
        .description("Find snippets, jump anywhere, or run an action.")
        .w(PANEL_W)
        .min_w(PANEL_W)
        .max_w(PANEL_W)
        .radius(radius::XL2)
        .fill(c.popover)
        .child(sr_title(c.foreground))
        .child(search_row(s, theme))
        .child(el().h(1.0).w_full().fill(c.hairline))
        .child(list(s, theme, viewport_h, hits));
    for shadow in &theme.depth.float {
        panel = panel.box_shadow(*shadow);
    }
    panel.modal(true, super::close)
}

fn sr_title(ink: rune::Color) -> El {
    el().key("command-menu-title")
        .role("heading")
        .label("Search and commands")
        .absolute(-1.0, -1.0)
        .w(1.0)
        .h(1.0)
        .min_w(1.0)
        .max_w(1.0)
        .min_h(1.0)
        .max_h(1.0)
        .clip_rounded(0.0)
        .child(
            text("Search and commands")
                .font(TextStyle::new(16.0, 500.0))
                .color(ink),
        )
}

fn search_row(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let field = rune::text_input(&s.command_menu.query, theme.ty.command)
        .key(super::QUERY_KEY)
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .single_line()
        .color(c.foreground)
        .placeholder(super::PLACEHOLDER, c.muted_foreground)
        .caret(c.foreground, theme.kit().selection())
        .label(super::PLACEHOLDER)
        .autofocus()
        .on_edit(|st: &mut MainState| &mut st.command_menu.query)
        .on_input_change(|st: &mut MainState, _| st.command_menu.selected = 0)
        .on_input_key(super::on_key);
    row()
        .h(SEARCH_H)
        .max_h(SEARCH_H)
        .w_full()
        .px(16.0)
        .gap(10.0)
        .center_y()
        .child(icon(&icons::SEARCH, 16.0).color(c.muted_foreground))
        .child(field)
}

fn list(s: &MainState, theme: &Theme, viewport_h: f32, hits: &Hits) -> El {
    let cap = LIST_CAP.min(viewport_h * 0.6);
    let extent = list_extent(hits);
    let height = extent.min(cap);
    let selected = (s.command_menu.selected < hits.len()).then_some(s.command_menu.selected);
    col()
        .key("command-menu-list")
        .h(height)
        .max_h(height)
        .w_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .children(rows(s, theme, hits, selected))
}

fn list_extent(hits: &Hits) -> f32 {
    if hits.len() == 0 {
        return EMPTY_H;
    }
    let mut height = LIST_PAD;
    if !hits.snippets.is_empty() {
        height += SECTION_H + hits.snippets.len() as f32 * ROW_H;
    }
    let mut section = None;
    for command in &hits.commands {
        if section != Some(command.section) {
            height += SECTION_H;
            section = Some(command.section);
        }
        height += ROW_H;
    }
    height
}

fn rows(s: &MainState, theme: &Theme, hits: &Hits, selected: Option<usize>) -> Vec<El> {
    if hits.len() == 0 {
        let message = format!("No results for “{}”", s.command_menu.query.text);
        return vec![el().h(EMPTY_H).w_full().child(
            text(message)
                .font(theme.ty.body)
                .color(theme.c.muted_foreground)
                .px(16.0)
                .py(12.0),
        )];
    }
    let lib = s.library_snapshot();
    let querying = !s.command_menu.query.text.trim().is_empty();
    let mut out = Vec::new();
    if !hits.snippets.is_empty() {
        let title = if querying {
            "Snippets"
        } else {
            "Frequently used"
        };
        out.push(section_label("snippets", title, theme));
        for (n, id) in hits.snippets.iter().copied().enumerate() {
            out.push(snippet_row(lib, theme, n, id, selected == Some(n), n));
        }
    }
    let mut seen = None;
    for (n, command) in hits.commands.iter().enumerate() {
        if seen != Some(command.section) {
            seen = Some(command.section);
            out.push(section_label(
                section_key(command.section),
                command.section.title(),
                theme,
            ));
        }
        let index = hits.snippets.len() + n;
        out.push(command_row(
            s,
            theme,
            n,
            command,
            selected == Some(index),
            index,
        ));
    }
    out.push(el().h(LIST_PAD).w_full());
    out
}

fn section_key(section: MenuSection) -> &'static str {
    match section {
        MenuSection::SelectedSnippet => "selected",
        MenuSection::Actions => "actions",
        MenuSection::GoTo => "goto",
    }
}

fn section_label(id: &str, title: &str, theme: &Theme) -> El {
    text(title)
        .key(format!("command-section-{id}"))
        .font(TextStyle::new(11.0, 500.0).lh(15.703))
        .color(theme.c.foreground)
        .px(16.0)
        .pt(10.0)
        .pb(4.0)
        .h(SECTION_H)
        .max_h(SECTION_H)
}

fn snippet_row(
    lib: &KudzuLibrary,
    theme: &Theme,
    n: usize,
    id: Uuid,
    on: bool,
    index: usize,
) -> El {
    let c = &theme.c;
    let snippet = lib.snippets.iter().find(|snippet| snippet.id == id);
    let name = snippet
        .map(display_name)
        .unwrap_or_else(|| "Untitled".to_string());
    let path = snippet
        .map(|snippet| group_path(&lib.groups, snippet.group_id).join(" › "))
        .unwrap_or_default();
    let trigger =
        snippet.and_then(|snippet| snippet.triggers.iter().find(|t| !t.is_empty()).cloned());
    let mut line = row()
        .key(format!("command-snippet-{n}"))
        .label(name.clone())
        .focus_selected(on)
        .h(ROW_H)
        .w_full()
        .px(12.0)
        .gap(10.0)
        .center_y()
        .on_press_tx(move |st: &mut MainState, tx| {
            st.command_menu.selected = index;
            super::activate(st, tx);
        })
        .child(icon(&icons::FILE_TEXT, 16.0).color(c.muted_foreground))
        .child(snippet_text(theme, &name, &path));
    if let Some(trigger) = trigger.as_deref() {
        line = line.child(trigger_chip(theme, trigger, false));
    }
    if on {
        line = line.fill(c.foreground.alpha(0.07));
    }
    line
}

fn snippet_text(theme: &Theme, name: &str, path: &str) -> El {
    let c = &theme.c;
    let mut block = col().grow(1.0).min_w(0.0).child(
        text(name)
            .font(theme.ty.body)
            .color(c.foreground)
            .truncate(),
    );
    if !path.is_empty() {
        block = block.child(
            text(path)
                .font(TextStyle::new(11.0, 400.0))
                .color(c.faint)
                .truncate(),
        );
    }
    block
}

fn command_row(
    s: &MainState,
    theme: &Theme,
    n: usize,
    command: &Command,
    on: bool,
    index: usize,
) -> El {
    let c = &theme.c;
    let label = command.label.clone();
    let mut line = row()
        .key(format!("command-action-{n}"))
        .label(label.clone())
        .focus_selected(on)
        .h(ROW_H)
        .w_full()
        .px(12.0)
        .gap(10.0)
        .center_y()
        .on_press_tx(move |st: &mut MainState, tx| {
            st.command_menu.selected = index;
            super::activate(st, tx);
        })
        .child(icon(command_icon(s, &command.kind), 16.0).color(c.muted_foreground))
        .child(
            text(label.clone())
                .font(theme.ty.body)
                .color(label_ink(theme, &command.kind))
                .grow(1.0)
                .min_w(0.0)
                .truncate(),
        );
    if let Some(chord) = command_chord(&command.kind) {
        line = line.child(Shortcut::combo(&chord).tone(Tone::Ghost).build(theme));
    }
    if on {
        line = line.fill(c.foreground.alpha(0.07));
    }
    line
}

fn command_icon(s: &MainState, kind: &Kind) -> &'static rune::Icon {
    match kind {
        Kind::Action(action) => menu::menu_icon(*action, s),
        Kind::GoToGroup(_) => &icons::FOLDER,
        Kind::GroupSettings(_) => &icons::SLIDERS_HORIZONTAL,
    }
}

fn command_chord(kind: &Kind) -> Option<String> {
    let Kind::Action(action) = kind else {
        return None;
    };
    let chord = menu::shown_chord(*action);
    if chord.is_empty() {
        None
    } else {
        Some(chord)
    }
}

fn label_ink(theme: &Theme, kind: &Kind) -> rune::Color {
    if matches!(kind, Kind::Action(MainAction::DeleteSnippet)) {
        theme.c.destructive
    } else {
        theme.c.foreground
    }
}

fn display_name(snippet: &SnippetRecord) -> String {
    if !snippet.name.is_empty() {
        return snippet.name.clone();
    }
    snippet
        .triggers
        .iter()
        .find(|trigger| !trigger.is_empty())
        .cloned()
        .unwrap_or_else(|| "Untitled".to_string())
}
