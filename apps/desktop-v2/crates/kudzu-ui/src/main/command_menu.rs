//! Command menu (`⌘K`). A 600-point panel, 18% down the window, listing frequent or
//! matching snippets and the actions that can run right now.
//!
//! Rune has no `h_frac`, so the top gap is a `grow(0.18)` spacer over a `grow(0.82)`
//! column. The menu's own field is not `is_editing_text`. Opening snapshots the field
//! under the menu: once this query takes focus, Rune clears that field's `focused`.
//! The toolbar search button still sets `command_open` itself, so it skips the snapshot.

use chrono::Utc;
use kudzu_core::library::groups::group_path;
use kudzu_core::library::usage::usage_by_id;
use kudzu_core::search::{self, rank};
use kudzu_core::template::summary::{plain_summary, SEARCH_UNITS};
use kudzu_core::{KudzuLibrary, SnippetRecord};
use rune::{col, el, icon, row, text, El, Key, KeyEvent, ScrollSpec, TextEdit, TextStyle, Tx};
use uuid::Uuid;

use crate::actions::main::{ActionContext, MainAction};
use crate::actions::Action;
use crate::main::state::{LibraryFilter, MainView};
use crate::shell::MainState;
use crate::theme::icons;
use crate::theme::{radius, Theme};
use crate::widgets::shortcut::{Shortcut, Tone};
use crate::widgets::trigger_chip::trigger_chip;

const QUERY_KEY: &str = "command-menu-query";
const PLACEHOLDER: &str = "Search snippets, groups and actions…";
const PANEL_W: f32 = 600.0;
const PANEL_H: f32 = 489.0;
const SEARCH_H: f32 = 48.0;
const LIST_H: f32 = 440.0;
const FREQUENT: usize = 5;
const MATCHED: usize = 8;

/// The menu's query and which row Up/Down last landed on.
#[derive(Clone, Default)]
pub struct State {
    pub query: TextEdit,
    pub selected: usize,
    /// A text field under the menu was focused when the menu opened.
    underlying_editing: bool,
}

/// Opens the menu. A second call leaves it open: `⌘K` opens, it does not toggle.
pub fn open(s: &mut MainState, tx: &mut Tx) {
    if s.frame.command_open {
        return;
    }
    s.command_menu.underlying_editing = s.is_editing_text();
    s.frame.command_open = true;
    s.command_menu.selected = 0;
    s.command_menu.query.focused = true;
    tx.focus(QUERY_KEY, true);
}

/// Closes the menu and clears the query, the selection and the field focus.
pub fn close(s: &mut MainState) {
    s.frame.command_open = false;
    s.command_menu.query.clear();
    s.command_menu.query.focused = false;
    s.command_menu.selected = 0;
    s.command_menu.underlying_editing = false;
}

/// Arrows and Return while the menu is the top modal. Plain text and Escape return false
/// so the field inserts and the modal's own dismiss runs.
pub fn on_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.shift {
        return false;
    }
    match ev.key {
        Key::Up => {
            nudge(s, -1);
            true
        }
        Key::Down => {
            nudge(s, 1);
            true
        }
        Key::Enter => {
            activate(s, tx);
            true
        }
        _ => false,
    }
}

pub fn view(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) {
    if !s.frame.command_open {
        return;
    }
    let hits = hits(s);
    let scrim = el()
        .key("command-menu-scrim")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(theme.kit().c.scrim)
        .a11y_hidden();
    // 18% of the live window, then the panel at the top of the remaining space.
    let layer = el()
        .key("command-menu-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .child(
            col()
                .w_full()
                .h_full()
                .items_center()
                .child(el().grow(0.18).w_full())
                .child(
                    col()
                        .grow(0.82)
                        .w_full()
                        .min_h(0.0)
                        .items_center()
                        .child(panel(s, theme, &hits)),
                ),
        );
    overlays.push(scrim);
    overlays.push(layer);
}

fn panel(s: &MainState, theme: &Theme, hits: &[Hit]) -> El {
    let c = &theme.c;
    let mut panel = col()
        .key("command-menu-dialog")
        .role("dialog")
        .label("Search and commands")
        .description("Find snippets, jump anywhere, or run an action.")
        .w(PANEL_W)
        .min_w(PANEL_W)
        .max_w(PANEL_W)
        .h(PANEL_H)
        .min_h(PANEL_H)
        .max_h(PANEL_H)
        .radius(radius::XL2)
        .fill(c.popover)
        .child(sr_title(c.foreground))
        .child(search_row(s, theme))
        .child(el().h(1.0).w_full().fill(c.hairline))
        .child(list(s, theme, hits));
    for shadow in &theme.depth.float {
        panel = panel.box_shadow(*shadow);
    }
    panel.modal(true, close)
}

/// One-point heading sitting one point above and left of the panel, as the reference does.
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
        .key(QUERY_KEY)
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .single_line()
        .color(c.foreground)
        .placeholder(PLACEHOLDER, c.muted_foreground)
        .caret(c.foreground, theme.kit().selection())
        .label(PLACEHOLDER)
        .autofocus()
        .on_edit(|st: &mut MainState| &mut st.command_menu.query)
        .on_input_change(|st: &mut MainState, _| st.command_menu.selected = 0)
        .on_input_key(on_key);
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

fn list(s: &MainState, theme: &Theme, hits: &[Hit]) -> El {
    let selected = (s.command_menu.selected < hits.len()).then_some(s.command_menu.selected);
    let rows = rows(s, theme, hits, selected);
    col()
        .key("command-menu-list")
        .h(LIST_H)
        .max_h(LIST_H)
        .w_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .children(rows)
}

fn rows(s: &MainState, theme: &Theme, hits: &[Hit], selected: Option<usize>) -> Vec<El> {
    if hits.is_empty() {
        let message = format!("No results for “{}”", s.command_menu.query.text);
        return vec![text(message)
            .font(theme.ty.body)
            .color(theme.c.muted_foreground)
            .px(16.0)
            .py(12.0)];
    }
    let lib = s.library_snapshot();
    let querying = !s.command_menu.query.text.trim().is_empty();
    let mut out = Vec::new();
    let snippets: Vec<_> = hits
        .iter()
        .enumerate()
        .filter_map(|(i, hit)| match hit {
            Hit::Snippet(id) => Some((i, *id)),
            Hit::Action(_) => None,
        })
        .collect();
    if !snippets.is_empty() {
        let title = if querying {
            "Snippets"
        } else {
            "Frequently used"
        };
        out.push(section_label(title, theme));
        for (n, (index, id)) in snippets.into_iter().enumerate() {
            out.push(snippet_row(
                lib,
                theme,
                n,
                id,
                selected == Some(index),
                index,
            ));
        }
    }
    let actions: Vec<_> = hits
        .iter()
        .enumerate()
        .filter_map(|(i, hit)| match hit {
            Hit::Action(action) => Some((i, *action)),
            Hit::Snippet(_) => None,
        })
        .collect();
    if !actions.is_empty() {
        out.push(section_label("Actions", theme));
        for (n, (index, action)) in actions.into_iter().enumerate() {
            out.push(action_row(theme, n, action, selected == Some(index), index));
        }
    }
    out
}

fn section_label(title: &str, theme: &Theme) -> El {
    text(title)
        .font(TextStyle::new(11.0, 500.0))
        .color(theme.c.foreground)
        .px(16.0)
        .pt(10.0)
        .pb(4.0)
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
    let snippet = lib.snippets.iter().find(|sn| sn.id == id);
    let name = snippet
        .map(display_name)
        .unwrap_or_else(|| "Untitled".to_string());
    let path = snippet
        .map(|sn| group_path(&lib.groups, sn.group_id).join(" › "))
        .unwrap_or_default();
    let trigger = snippet.and_then(|sn| sn.triggers.iter().find(|t| !t.is_empty()).cloned());
    let mut line = row()
        .key(format!("command-snippet-{n}"))
        .label(name.clone())
        .focus_selected(on)
        .h(36.0)
        .w_full()
        .px(12.0)
        .gap(10.0)
        .center_y()
        .on_press_tx(move |st: &mut MainState, tx| {
            st.command_menu.selected = index;
            activate(st, tx);
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

fn action_row(theme: &Theme, n: usize, action: MainAction, on: bool, index: usize) -> El {
    let c = &theme.c;
    let label = action.label();
    let mut line = row()
        .key(format!("command-action-{n}"))
        .label(label)
        .focus_selected(on)
        .h(36.0)
        .w_full()
        .px(12.0)
        .gap(10.0)
        .center_y()
        .on_press_tx(move |st: &mut MainState, tx| {
            st.command_menu.selected = index;
            activate(st, tx);
        })
        .child(icon(action_icon(action), 16.0).color(c.muted_foreground))
        .child(
            text(label)
                .font(theme.ty.body)
                .color(c.foreground)
                .grow(1.0)
                .min_w(0.0)
                .truncate(),
        )
        .child(
            Shortcut::combo(&shown_chord(action))
                .tone(Tone::Ghost)
                .build(theme),
        );
    if on {
        line = line.fill(c.foreground.alpha(0.07));
    }
    line
}

fn nudge(s: &mut MainState, delta: isize) {
    let len = hits(s).len();
    if len == 0 {
        return;
    }
    let cur = if s.command_menu.selected < len {
        s.command_menu.selected
    } else {
        0
    };
    s.command_menu.selected = (cur as isize + delta).rem_euclid(len as isize) as usize;
}

fn activate(s: &mut MainState, tx: &mut Tx) {
    let Some(hit) = hits(s).get(s.command_menu.selected).copied() else {
        return;
    };
    match hit {
        Hit::Snippet(id) => {
            s.navigate_to(MainView::Library(LibraryFilter::All));
            s.select_snippet(Some(id));
            close(s);
        }
        // Closing and then running would open the menu again.
        Hit::Action(MainAction::CommandMenu) => close(s),
        Hit::Action(action) => {
            close(s);
            action.run(s, tx);
        }
    }
}

#[derive(Clone, Copy)]
enum Hit {
    Snippet(Uuid),
    Action(MainAction),
}

fn hits(s: &MainState) -> Vec<Hit> {
    let mut out = Vec::new();
    let lib = s.library_snapshot();
    let usage = usage_by_id(&lib.usage);
    let indexed = search::index(&lib.snippets, &usage, &|content| {
        plain_summary(content, None, SEARCH_UNITS)
    });
    let query = s.command_menu.query.text.as_str();
    let limit = if query.trim().is_empty() {
        FREQUENT
    } else {
        MATCHED
    };
    for hit in rank(&indexed, query, limit, Utc::now()) {
        if let Some(snippet) = lib.snippets.get(hit.position) {
            out.push(Hit::Snippet(snippet.id));
        }
    }
    let ctx = menu_context(s);
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    for action in MainAction::ALL.iter().copied() {
        if !action.available(ctx) {
            continue;
        }
        if !terms.is_empty() {
            let hay = format!(
                "{} {}",
                action.label().to_lowercase(),
                action.keywords().to_lowercase()
            );
            if !terms.iter().all(|term| hay.contains(term)) {
                continue;
            }
        }
        out.push(Hit::Action(action));
    }
    out
}

/// The menu is open, so `Action::enabled` would hide every action. Ask `available` as if
/// no modal were up, and ignore this menu's own field.
fn menu_context(s: &MainState) -> ActionContext {
    ActionContext {
        area: s.frame.view.area(),
        has_selected_snippet: s.frame.selected_snippet.is_some(),
        can_back: s.frame.history.can_go_back(),
        can_forward: s.frame.history.can_go_forward(),
        in_text_input: s.command_menu.underlying_editing,
        modal_open: false,
        composing: false,
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

fn shown_chord(action: MainAction) -> String {
    match action {
        MainAction::OpenSettings => "mod+,".to_string(),
        MainAction::OpenShortcuts => "mod+/".to_string(),
        other => other.chord().to_string(),
    }
}

fn action_icon(action: MainAction) -> &'static rune::Icon {
    match action {
        MainAction::CommandMenu => &icons::SEARCH,
        MainAction::NewSnippet => &icons::PLUS,
        MainAction::NewGroup => &icons::FOLDER_PLUS,
        MainAction::FocusFilter | MainAction::FocusList => &icons::SEARCH,
        MainAction::EditContent => &icons::PENCIL,
        MainAction::DuplicateSnippet => &icons::COPY,
        MainAction::ToggleFavorite => &icons::STAR,
        MainAction::ToggleEnabled => &icons::CIRCLE_OFF,
        MainAction::DeleteSnippet => &icons::TRASH_2,
        MainAction::TogglePause => &icons::PAUSE,
        MainAction::ViewAll => &icons::FILE_TEXT,
        MainAction::ViewFavorites => &icons::STAR,
        MainAction::ViewRecent => &icons::CLOCK_3,
        MainAction::ViewInsights => &icons::BAR_CHART_3,
        MainAction::ViewClipboard => &icons::CLIPBOARD_LIST,
        MainAction::OpenSettings => &icons::SETTINGS,
        MainAction::OpenShortcuts => &icons::KEYBOARD,
        MainAction::Back => &icons::ARROW_LEFT,
        MainAction::Forward => &icons::ARROW_RIGHT,
        MainAction::ToggleSidebar => &icons::PANEL_LEFT,
    }
}
