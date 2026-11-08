//! The ⌘K command menu (`components/app/main/command-menu.tsx`): snippets, the selected
//! snippet's actions, app actions and places to go, in one cmdk list over a search field. Also
//! the parts of `data-actions.ts` it runs (import and export, with their toasts).

use rune::{el, text, Cx, El, FileDialogOptions, Icon, Tx};

use super::{MainUi, SettingsSection, View};
use crate::backend::commands;
use crate::ui::icons;
use crate::ui::kit::command::{Command, CommandItem, CommandStyle};
use crate::ui::kit::dialog::Dialog;
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::kit::toast::Toast;
use crate::ui::kit::trigger_chip::trigger_chip;
use crate::ui::theme::{radius, text as ty};
use crate::ui::theme_for;
use kudzu_core::client::groups::{build_tree, flatten, group_path};
use kudzu_core::client::search::{build_index, search};

pub type CommandState = crate::ui::kit::command::CommandState;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Selected,
    Actions,
    GoTo,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Section::Selected => "Selected snippet",
            Section::Actions => "Actions",
            Section::GoTo => "Go to",
        }
    }
}

type Run = std::rc::Rc<dyn Fn(&mut MainUi, &mut Tx)>;

struct Action {
    id: String,
    label: String,
    keywords: &'static str,
    icon: &'static Icon,
    shortcut: Option<&'static str>,
    section: Section,
    run: Run,
}

fn act(id: impl Into<String>, section: Section, icon: &'static Icon, label: impl Into<String>, keywords: &'static str, shortcut: Option<&'static str>, run: impl Fn(&mut MainUi, &mut Tx) + 'static) -> Action {
    Action { id: id.into(), label: label.into(), keywords, icon, shortcut, section, run: std::rc::Rc::new(run) }
}

/// `actions` in command-menu.tsx, in its order.
fn actions(s: &MainUi) -> Vec<Action> {
    use Section::*;
    let mut a = Vec::new();
    if let Some(sel) = s.selected_snippet() {
        let id = sel.id;
        let name = if sel.name.is_empty() { "Untitled" } else { &sel.name };
        a.push(act("fav", Selected, &icons::STAR, if sel.is_favorite { format!("Unfavorite “{name}”") } else { format!("Favorite “{name}”") }, "", Some("mod+shift+f"), move |s, _| s.toggle_favorite(id)));
        a.push(act("en", Selected, &icons::CIRCLE_OFF, if sel.is_enabled { format!("Turn off “{name}”") } else { format!("Turn on “{name}”") }, "", Some("mod+shift+e"), move |s, _| s.toggle_enabled(id)));
        a.push(act("dup", Selected, &icons::COPY, format!("Duplicate “{name}”"), "", Some("mod+d"), move |s, _| s.duplicate(id)));
        a.push(act("del", Selected, &icons::TRASH, format!("Delete “{name}”"), "", Some("mod+backspace"), move |s, tx| s.request_delete(vec![id], tx)));
    }
    let paused = s.status.paused;
    let dark = s.settings.theme == crate::settings::Theme::Dark;
    let clip_on = s.settings.clipboard_enabled;
    a.push(act("new", Actions, &icons::PLUS, "New snippet", "create add", Some("mod+n"), |s, _| {
        s.new_snippet(Default::default());
    }));
    a.push(act("newg", Actions, &icons::FOLDER_PLUS, "New group", "folder create", Some("mod+shift+n"), |s, _| {
        s.create_group("New group", None);
    }));
    a.push(act("pause", Actions, if paused { &icons::PLAY } else { &icons::PAUSE }, if paused { "Resume expanding" } else { "Pause expanding" }, "stop disable", Some("mod+shift+p"), move |s, _| s.set_paused(!paused)));
    a.push(act("imp", Actions, &icons::DOWNLOAD, "Import snippets…", "raycast textexpander espanso json yaml csv", None, run_import));
    a.push(act("exp", Actions, &icons::UPLOAD, "Export library…", "backup json espanso", None, run_export_json));
    a.push(act("theme", Actions, if dark { &icons::SUN } else { &icons::MOON }, if dark { "Switch to light appearance" } else { "Switch to dark appearance" }, "theme mode color", None, move |s, _| {
        s.update_settings(serde_json::json!({ "theme": if dark { "light" } else { "dark" } }))
    }));
    a.push(act("keys", Actions, &icons::KEYBOARD, "Keyboard shortcuts", "help keys", Some("mod+/"), |s, _| s.shortcuts_open = true));
    a.push(act("all", GoTo, &icons::LAYERS, "All snippets", "", Some("mod+1"), |s, _| s.set_view(View::All)));
    a.push(act("favs", GoTo, &icons::STAR, "Favorites", "", Some("mod+2"), |s, _| s.set_view(View::Favorites)));
    a.push(act("recent", GoTo, &icons::CLOCK_3, "Recently used", "", Some("mod+3"), |s, _| s.set_view(View::Recent)));
    a.push(act("ins", GoTo, &icons::CHART_COLUMN, "Insights", "stats statistics usage", Some("mod+4"), |s, _| s.set_view(View::Insights)));
    a.push(act("clip", GoTo, &icons::CLIPBOARD_LIST, "Clipboard history", "copy paste history copied", Some("mod+5"), |s, _| s.set_view(View::Clipboard(Default::default()))));
    a.push(act("clipset", GoTo, &icons::SETTINGS, "Settings › Clipboard", "history retention keep days privacy", None, |s, _| s.set_view(View::Settings(Some(SettingsSection::Clipboard)))));
    a.push(act("clippause", Actions, if clip_on { &icons::PAUSE } else { &icons::PLAY }, if clip_on { "Pause clipboard history" } else { "Resume clipboard history" }, "copy record privacy stop", None, move |s, _| {
        s.update_settings(serde_json::json!({ "clipboardEnabled": !clip_on }))
    }));
    a.push(act("set", GoTo, &icons::SETTINGS, "Settings", "preferences", Some("mod+,"), |s, _| s.set_view(View::Settings(None))));
    a.push(act("apps", GoTo, &icons::SETTINGS, "Settings › Apps", "exclude block terminal", None, |s, _| s.set_view(View::Settings(Some(SettingsSection::Apps)))));
    a.push(act("data", GoTo, &icons::SETTINGS, "Settings › Backups & data", "restore import export", None, |s, _| s.set_view(View::Settings(Some(SettingsSection::Data)))));
    let tree = build_tree(&s.library.groups);
    for n in flatten(&tree, None) {
        let id = n.group.id;
        let path = group_path(&s.library.groups, Some(id)).join(" › ");
        a.push(act(format!("g-{id}"), GoTo, &icons::FOLDER, path.clone(), "group folder", None, move |s, _| s.set_view(View::Group(id))));
        a.push(act(format!("gs-{id}"), Actions, &icons::SLIDERS_HORIZONTAL, format!("Group settings: {path}"), "rules apps", None, move |s, _| super::group_settings::open(s, id)));
    }
    a
}

/// The actions a query shows: every term in the label or keywords; with no query, groups stay
/// out.
fn filter_actions(all: Vec<Action>, query: &str) -> Vec<Action> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return all.into_iter().filter(|a| !a.id.starts_with("g-") && !a.id.starts_with("gs-")).collect();
    }
    all.into_iter()
        .filter(|a| {
            let hay = format!("{} {}", a.label, a.keywords).to_lowercase();
            q.split_whitespace().all(|t| hay.contains(t))
        })
        .collect()
}

/// `close`: the menu shuts and forgets what was typed.
pub fn close(s: &mut MainUi) {
    s.command_open = false;
    s.command.reset();
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) {
    if !s.command_open {
        return;
    }
    let th = theme_for(cx);
    let t = &th.t;
    let query = s.command.query().to_string();
    let q = query.trim().to_lowercase();

    // Snippets: the best matches, or the most used when nothing is typed.
    let index = build_index(&s.library.snippets, &s.usage);
    let now = chrono::Utc::now().timestamp_millis();
    let hits = search(&index, &query, if query.is_empty() { 5 } else { 8 }, now);
    let snippet_items: Vec<CommandItem<MainUi>> = hits
        .iter()
        .map(|h| {
            let x = h.snippet;
            let id = x.id;
            let mut item = CommandItem::new(format!("snippet-{id}"))
                .icon(&icons::FILE_TEXT)
                .text(if x.name.is_empty() { "Untitled".to_string() } else { x.name.clone() })
                .child(text(group_path(&s.library.groups, x.group_id).join(" › ")).font(ty::XS).color(t.faint).truncate().shrink().min_w(0.0));
            item = match x.triggers.first() {
                Some(tr) => item.child(el().grow(1.0).justify_end().child(trigger_chip(th, tr, false))),
                None => item,
            };
            item.on_select(move |s: &mut MainUi, _| {
                close(s);
                s.set_view(View::All);
                s.selected = Some(id);
            })
        })
        .collect();

    let filtered = filter_actions(actions(s), &query);
    let mut cmd = Command::new("cmdk", &s.command, |s: &mut MainUi| &mut s.command)
        .style(CommandStyle::Palette)
        .filter(false)
        .wrap(true)
        .in_dialog()
        .placeholder("Search snippets, groups and actions…")
        .trailing(Shortcut::new("escape").build(th))
        .empty(text(format!("No results for “{query}”.")).font(ty::SM).color(t.muted_foreground))
        .on_escape(|s: &mut MainUi, _, _| close(s));
    if !snippet_items.is_empty() {
        cmd = cmd.group(Some(if q.is_empty() { "Frequently used" } else { "Snippets" }), snippet_items);
    }
    for sec in [Section::Selected, Section::Actions, Section::GoTo] {
        let items: Vec<CommandItem<MainUi>> = filtered
            .iter()
            .filter(|a| a.section == sec)
            .map(|a| {
                let run = a.run.clone();
                let mut item = CommandItem::new(a.id.clone()).icon(a.icon).text(a.label.clone());
                if a.id == "del" {
                    item = item.destructive();
                }
                if let Some(sc) = a.shortcut {
                    item = item.child(el().grow(1.0).justify_end().child(Shortcut::new(sc).build(th)));
                }
                item.on_select(move |s: &mut MainUi, tx| {
                    close(s);
                    run(s, tx);
                })
            })
            .collect();
        if !items.is_empty() {
            cmd = cmd.group(Some(sec.title()), items);
        }
    }
    let body = cmd.build(cx);
    Dialog::new("command-menu")
        .open(true)
        .on_open_change(|s: &mut MainUi, open, _| {
            if !open {
                close(s);
            }
        })
        .close_button(false)
        .top(0.18)
        .max_width(600.0)
        .padding(0.0)
        .gap(0.0)
        .radius(radius::XXL)
        .float()
        .ring(false)
        .backdrop_blur(40.0)
        .label("Search and commands")
        .description("Find snippets, jump anywhere, or run an action.")
        .child(body)
        .build(cx, layer);
}

// MARK: data-actions.ts

/// `reportImport`.
fn report_import(s: &mut MainUi, r: &kudzu_core::interchange::coordinator::ImportSummary) {
    let plural = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    let mut what = plural(r.imported_snippets, "snippet");
    if r.imported_groups > 0 {
        what.push_str(&format!(" in {}", plural(r.imported_groups, "group")));
    }
    let description = match r.warnings.first() {
        Some(w) => format!("{} imported with changes — e.g. {}", plural(r.warnings.len(), "item"), w.message),
        None => "A backup was saved first, just in case.".to_string(),
    };
    s.toasts.push(Toast::success(format!("Imported {what}")).description(description));
}

/// `runImport("file")`: the file dialog, then the import on a worker.
pub fn run_import(_s: &mut MainUi, tx: &mut Tx) {
    let options = FileDialogOptions { title: "Import snippets".into(), ..Default::default() }.filter("Snippet files", &["json", "yml", "yaml", "textexpander", "csv"]);
    tx.open_file(options, |s: &mut MainUi, result, _| match result {
        Ok(Some(path)) => s.spawn(move || commands::import_file(path), |s, r, _| match r {
            Ok(summary) => report_import(s, &summary),
            Err(e) => s.toasts.push(Toast::error("Import failed").description(e)),
        }),
        Ok(None) => {}
        Err(e) => s.toasts.push(Toast::error("Import failed").description(e.to_string())),
    });
}

/// `runExport("json")`.
pub fn run_export_json(_s: &mut MainUi, tx: &mut Tx) {
    let stamp = chrono::Local::now().format("%Y-%m-%d");
    let options = FileDialogOptions { title: "Export library".into(), file_name: Some(format!("kudzu-{stamp}.json")), ..Default::default() }.filter("Kudzu library", &["json"]);
    tx.save_file(options, |s: &mut MainUi, result, _| match result {
        Ok(Some(path)) => {
            let shown = path.display().to_string();
            s.spawn(move || commands::export_json(path), move |s, r, _| match r {
                Ok(()) => s.toasts.push(Toast::success("Library exported").description(shown.clone())),
                Err(e) => s.toasts.push(Toast::error("Export failed").description(e)),
            });
        }
        Ok(None) => {}
        Err(e) => s.toasts.push(Toast::error("Export failed").description(e.to_string())),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(id: &str, label: &str, kw: &'static str) -> Action {
        act(id, Section::Actions, &icons::PLUS, label, kw, None, |_, _| {})
    }

    #[test]
    fn every_term_must_match_and_groups_hide_without_a_query() {
        let all = || vec![a("new", "New snippet", "create add"), a("g-1", "Work", "group folder"), a("gs-1", "Group settings: Work", "rules apps")];
        let ids = |v: Vec<Action>| v.into_iter().map(|a| a.id).collect::<Vec<_>>();
        assert_eq!(ids(filter_actions(all(), "")), ["new"]);
        assert_eq!(ids(filter_actions(all(), "new create")), ["new"]);
        assert_eq!(ids(filter_actions(all(), "work")), ["g-1", "gs-1"]);
        assert!(filter_actions(all(), "new folder").is_empty());
    }
}
