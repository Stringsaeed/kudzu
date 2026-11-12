//! The editor pane (EDT-1, `snippet-editor.tsx:304-505`): the toolbar, then a scrolling body at
//! most 760 wide (padding 32, gaps 24): the off banner, name, triggers with the no-trigger note
//! and the collision banners, the content, the preview, behaviour and the usage line. With no
//! snippet open it is the "No snippet selected" empty state.

use std::collections::HashSet;

use kudzu_core::fmt::relative_time;
use kudzu_core::library::conflicts::ConflictKind;
use kudzu_core::library::groups::{group_path, is_group_enabled};
use kudzu_core::library::usage::usage_by_id;
use kudzu_core::SnippetRecord;
use rune::{col, el, icon, row, text, text_input, Cx, El, KeyEvent, ScrollSpec, TextStyle, Tx};
use uuid::Uuid;

use super::{behavior, content, header, preview, triggers};
use crate::main::library;
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::empty_state::EmptyState;
use crate::widgets::kit_ext::button::{Button, Size};

pub const BODY_KEY: &str = "editor-body";
pub const BANNER_KEY: &str = "editor-off-banner";
pub const NOTE_KEY: &str = "editor-no-trigger-note";
pub const CONTENT_REGION_KEY: &str = "editor-content-region";
pub const FOOTER_KEY: &str = "editor-footer";
pub const EMPTY_NEW_KEY: &str = "editor-empty-new";
pub const NO_TRIGGER: &str =
    "Add a trigger so this snippet can expand as you type. You can still insert it from search.";
pub const SNIPPET_OFF: &str =
    "This snippet is turned off. It still shows in search but won't expand.";

/// One collision of the open snippet's triggers with another snippet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub message: String,
    pub other: Uuid,
    /// The other snippet's name, or "another snippet" when it has none.
    pub other_name: String,
    /// The open snippet's trigger, lowercased (its chip turns amber).
    pub mine: String,
}

/// `myConflicts` with Tauri's sentences, from the library's last conflict check.
pub fn conflicts(s: &MainState) -> Vec<Conflict> {
    let Some(id) = s.editor.open_id() else {
        return Vec::new();
    };
    let snapshot = s.svc.library.conflicts().get();
    let library = s.library_snapshot();
    snapshot
        .items
        .iter()
        .filter(|c| c.snippet_id == id || c.other_snippet_id == id)
        .map(|c| {
            let mine = c.snippet_id == id;
            let other = if mine {
                c.other_snippet_id
            } else {
                c.snippet_id
            };
            let message = match (c.kind, mine) {
                (ConflictKind::Duplicate, true) => format!("“{}” is also used by", c.trigger),
                (ConflictKind::Duplicate, false) => {
                    format!("“{}” is also used by", c.other_trigger)
                }
                (ConflictKind::Shadowing, true) => format!(
                    "“{}” fires before “{}” can be finished in",
                    c.trigger, c.other_trigger
                ),
                (ConflictKind::Shadowing, false) => format!(
                    "“{}” can't be typed in full — “{}” fires first in",
                    c.other_trigger, c.trigger
                ),
            };
            let other_name = library
                .snippets
                .iter()
                .find(|sn| sn.id == other)
                .map(|sn| sn.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "another snippet".into());
            let trigger = if mine { &c.trigger } else { &c.other_trigger };
            Conflict {
                message,
                other,
                other_name,
                mine: trigger.to_lowercase(),
            }
        })
        .collect()
}

/// The open snippet's triggers that collide, lowercased.
pub fn conflicted(s: &MainState) -> HashSet<String> {
    conflicts(s).into_iter().map(|c| c.mine).collect()
}

/// The banner's sentence when the snippet or a group above it is off.
pub fn off_banner(d: &SnippetRecord, groups: &[kudzu_core::GroupRecord]) -> Option<String> {
    if !d.is_enabled {
        return Some(SNIPPET_OFF.into());
    }
    if is_group_enabled(groups, d.group_id) {
        return None;
    }
    let path = group_path(groups, d.group_id).join(" › ");
    Some(format!(
        "This snippet's group “{path}” is turned off, so it won't expand."
    ))
}

pub fn view(s: &MainState, cx: &Cx<MainState>, theme: &Theme, overlays: &mut Vec<El>) -> El {
    let Some(d) = s.editor.draft() else {
        return empty(theme);
    };
    let body = col()
        .key(format!("{BODY_KEY}-{}", d.id))
        .max_w(760.0)
        .w_full()
        .self_center()
        .px(32.0)
        .pt(28.0)
        .pb(64.0)
        .gap(24.0)
        .transition(crate::motion::rise());
    let mut body = match off_banner(d, &s.library_snapshot().groups) {
        Some(sentence) => body.child(banner(theme, sentence)),
        None => body,
    };
    body = body
        .child(identity(s, cx, theme, d))
        .child(content_section(s, theme))
        .child(preview::view(s, theme))
        .child(behavior::view(s, cx, theme, overlays))
        .child(footer(s, theme, d));
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(header::view(s, theme, overlays))
        .child(
            col()
                .key(BODY_KEY)
                .grow(1.0)
                .w_full()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(body),
        )
}

fn empty(theme: &Theme) -> El {
    let button = Button::new("New snippet")
        .icon(&icons::PLUS)
        .size(Size::Sm)
        .shortcut("⌘N")
        .on_press(|st: &mut MainState| {
            let _ = library::actions::new_snippet(st, Default::default());
        })
        .build(theme)
        .key(EMPTY_NEW_KEY)
        .no_window_drag();
    EmptyState::new(&icons::CORNER_DOWN_LEFT, "No snippet selected")
        .description("Pick one from the list, or start a new one.")
        .action(button)
        .build(theme)
        .window_drag()
}

fn small(theme: &Theme) -> TextStyle {
    let _ = theme;
    TextStyle::new(12.0, 400.0).lh(16.0)
}

fn banner(theme: &Theme, sentence: String) -> El {
    let c = &theme.c;
    row()
        .key(BANNER_KEY)
        .role("note")
        .label(sentence.clone())
        .w_full()
        .gap(8.0)
        .px(12.0)
        .py(8.0)
        .items_center()
        .radius(8.0)
        .fill(c.foreground.alpha(0.04))
        .transition(crate::motion::rise())
        .child(el().w(6.0).h(6.0).radius(3.0).fill(c.faint).a11y_hidden())
        .child(
            text(sentence)
                .font(small(theme))
                .color(c.muted_foreground)
                .wrap(),
        )
}

/// Name, then the triggers with their note and banners (`flex-col gap-4`).
fn identity(s: &MainState, cx: &Cx<MainState>, theme: &Theme, d: &SnippetRecord) -> El {
    let c = &theme.c;
    let name = text_input(&s.editor.name, theme.ty.name)
        .single_line()
        .key(super::NAME_KEY)
        .label("Snippet name")
        .w_full()
        .color(c.foreground)
        .placeholder("Untitled snippet", c.faint.alpha(0.7))
        .caret(c.iris, theme.kit().selection())
        .on_edit(|st: &mut MainState| &mut st.editor.name)
        .on_input_change(super::name_changed)
        .on_input_key(name_key);
    let mut triggers_block = col().w_full().gap(6.0).child(triggers::view(s, cx, theme));
    if d.triggers.is_empty() && !d.content.is_empty() {
        triggers_block = triggers_block.child(
            text(NO_TRIGGER)
                .key(NOTE_KEY)
                .font(small(theme))
                .color(c.muted_foreground)
                .wrap()
                .transition(crate::motion::rise()),
        );
    }
    for (i, conflict) in conflicts(s).into_iter().enumerate() {
        let other = conflict.other;
        triggers_block = triggers_block.child(
            row()
                .key(format!("editor-conflict-{i}"))
                .w_full()
                .wrap()
                .gap(4.0)
                .items_center()
                .transition(crate::motion::rise())
                .child(
                    icon(&icons::TRIANGLE_ALERT, 14.0)
                        .color(c.warning)
                        .a11y_hidden(),
                )
                .child(text(conflict.message).font(small(theme)).color(c.warning))
                .child(
                    text(conflict.other_name.clone())
                        .spans(vec![
                            rune::Span::new(0, conflict.other_name.len()).underline(c.warning)
                        ])
                        .key(format!("editor-conflict-{i}-other"))
                        .role("button")
                        .label(conflict.other_name)
                        .focusable(true)
                        .font(TextStyle {
                            weight: 500.0,
                            ..small(theme)
                        })
                        .color(c.warning)
                        .on_press(move |st: &mut MainState| st.select_snippet(Some(other))),
                ),
        );
    }
    col().w_full().gap(16.0).child(name).child(triggers_block)
}

/// Name: Return moves to the triggers; Escape returns to the list.
fn name_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.editor.name.is_composing() {
        return false;
    }
    let bare = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift;
    if ev.key == rune::Key::Enter && bare {
        s.editor.focus_trigger(tx);
        return true;
    }
    super::field_key(s, ev, tx)
}

/// "Expands to", the field, and whatever the content field puts beside the label.
fn content_section(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let mut label = row().w_full().h(24.0).gap(8.0).items_center().child(
        text("Expands to")
            .font(theme.ty.control_xs)
            .color(c.muted_foreground),
    );
    if let Some(tools) = content::toolbar(s, theme) {
        label = label.child(el().grow(1.0).h(1.0)).child(tools);
    }
    col()
        .key(CONTENT_REGION_KEY)
        .role("region")
        .label("Content")
        .w_full()
        .gap(6.0)
        .child(label)
        .child(content::view(s, theme))
}

/// "Used N times · Last used … · Edited … · Created …".
fn footer(s: &MainState, theme: &Theme, d: &SnippetRecord) -> El {
    let c = &theme.c;
    let now = crate::palette::model::utc_from(s.svc.clock.as_ref()).timestamp_millis();
    let usage = usage_by_id(&s.library_snapshot().usage)
        .remove(&d.id)
        .unwrap_or_default();
    let mut parts = vec![format!(
        "Used {} {}",
        usage.count,
        if usage.count == 1 { "time" } else { "times" }
    )];
    if usage.last_ms != 0 {
        parts.push(format!(
            "Last used {}",
            relative_time(usage.last_ms, now, 0).to_lowercase()
        ));
    }
    parts.push(format!(
        "Edited {}",
        relative_time(d.updated_at.timestamp_millis(), now, 0).to_lowercase()
    ));
    parts.push(format!("Created {}", d.created_at.format("%b %-d, %Y")));
    let font = theme.ty.meta_11.features("'tnum' 1");
    let mut line = row().key(FOOTER_KEY).w_full().wrap().gap(16.0);
    for (i, part) in parts.into_iter().enumerate() {
        line = line.child(
            text(part)
                .key(format!("{FOOTER_KEY}-{i}"))
                .font(font)
                .color(c.faint),
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::{CaseMode, FireMode, GroupRecord};

    fn group(id: u128, parent: Option<u128>, on: bool) -> GroupRecord {
        GroupRecord {
            id: Uuid::from_u128(id),
            name: format!("G{id}"),
            parent_id: parent.map(Uuid::from_u128),
            sort_index: 0,
            is_enabled: on,
            insertion: None,
            rule: None,
        }
    }

    fn snippet(enabled: bool, group: Option<u128>) -> SnippetRecord {
        SnippetRecord {
            id: Uuid::from_u128(9),
            name: String::new(),
            triggers: vec![],
            content: String::new(),
            formatting: None,
            group_id: group.map(Uuid::from_u128),
            fire_mode: FireMode::default(),
            case_mode: CaseMode::default(),
            insertion: None,
            rule: None,
            is_favorite: false,
            is_enabled: enabled,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }

    #[test]
    fn the_off_banner_names_the_snippet_or_its_group_path() {
        let groups = [group(1, None, false), group(2, Some(1), true)];
        assert_eq!(
            off_banner(&snippet(false, Some(2)), &groups).as_deref(),
            Some(SNIPPET_OFF)
        );
        assert_eq!(
            off_banner(&snippet(true, Some(2)), &groups).as_deref(),
            Some("This snippet's group “G1 › G2” is turned off, so it won't expand.")
        );
        assert_eq!(off_banner(&snippet(true, None), &groups), None);
    }
}
