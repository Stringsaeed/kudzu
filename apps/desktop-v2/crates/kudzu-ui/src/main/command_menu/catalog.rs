//! Snippet hits and the command catalog. Group rows follow Tauri's empty-query rule:
//! a blank query drops ids that start with `g-` or `gs-`.

use std::collections::HashSet;

use chrono::Utc;
use kudzu_core::library::groups::{build_tree, flatten, group_path};
use kudzu_core::library::usage::usage_by_id;
use kudzu_core::search::{self, rank};
use kudzu_core::template::summary::{plain_summary, SEARCH_UNITS};
use uuid::Uuid;

use crate::actions::main::{ActionContext, MainAction};
use crate::actions::menu::{self, MenuSection};
use crate::shell::MainState;

const FREQUENT: usize = 5;
const MATCHED: usize = 8;
/// Group rows sort after the static commands in their section.
const GROUP_ORDER: u16 = 100;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Action(MainAction),
    GoToGroup(Uuid),
    GroupSettings(Uuid),
}

pub(super) struct Command {
    pub label: String,
    pub section: MenuSection,
    pub order: u16,
    pub kind: Kind,
}

pub(super) struct Hits {
    pub snippets: Vec<Uuid>,
    pub commands: Vec<Command>,
}

pub(super) enum Flat {
    Snippet(Uuid),
    Command(Kind),
}

impl Hits {
    pub fn len(&self) -> usize {
        self.snippets.len() + self.commands.len()
    }

    pub fn get(&self, index: usize) -> Option<Flat> {
        if index < self.snippets.len() {
            return Some(Flat::Snippet(self.snippets[index]));
        }
        self.commands
            .get(index - self.snippets.len())
            .map(|command| match command.kind {
                Kind::Action(action) => Flat::Command(Kind::Action(action)),
                Kind::GoToGroup(id) => Flat::Command(Kind::GoToGroup(id)),
                Kind::GroupSettings(id) => Flat::Command(Kind::GroupSettings(id)),
            })
    }
}

pub(super) fn hits(s: &MainState) -> Hits {
    Hits {
        snippets: snippets(s),
        commands: commands(s),
    }
}

fn snippets(s: &MainState) -> Vec<Uuid> {
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
    rank(&indexed, query, limit, Utc::now())
        .into_iter()
        .filter_map(|hit| lib.snippets.get(hit.position).map(|snippet| snippet.id))
        .collect()
}

fn commands(s: &MainState) -> Vec<Command> {
    let query = s.command_menu.query.text.as_str();
    let querying = !query.trim().is_empty();
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.to_lowercase())
        .collect();
    let mut commands = Vec::new();
    let ctx = menu_context(s);
    for action in MainAction::ALL.iter().copied() {
        let Some(section) = menu::menu_section(action) else {
            continue;
        };
        if !action.available(ctx) {
            continue;
        }
        let label = menu::menu_label(action, s);
        if !matches_terms(&terms, &label, action.keywords()) {
            continue;
        }
        commands.push(Command {
            label,
            section,
            kind: Kind::Action(action),
            order: menu::menu_order(action),
        });
    }
    let lib = s.library_snapshot();
    let tree = build_tree(&lib.groups);
    // An empty query drops `g-` and `gs-` rows. Static Go to rows stay.
    if querying {
        for (index, node) in flatten(&tree, &HashSet::new()).into_iter().enumerate() {
            let path = group_path(&lib.groups, Some(node.group.id)).join(" › ");
            let order = GROUP_ORDER + index as u16;
            if matches_terms(&terms, &path, "group folder") {
                commands.push(Command {
                    label: path.clone(),
                    section: MenuSection::GoTo,
                    kind: Kind::GoToGroup(node.group.id),
                    order,
                });
            }
            let settings_label = format!("Group settings: {path}");
            if matches_terms(&terms, &settings_label, "rules apps") {
                commands.push(Command {
                    label: settings_label,
                    section: MenuSection::Actions,
                    kind: Kind::GroupSettings(node.group.id),
                    order,
                });
            }
        }
    }
    commands.sort_by_key(|command| (command.section, command.order));
    commands
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

fn matches_terms(terms: &[String], label: &str, keywords: &str) -> bool {
    if terms.is_empty() {
        return true;
    }
    let hay = format!("{} {}", label.to_lowercase(), keywords.to_lowercase());
    terms.iter().all(|term| hay.contains(term))
}
