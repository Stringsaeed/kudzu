//! Behaviour (RUL-6, `behavior.tsx`): when to expand, capitalisation, how to insert and which
//! apps, each row explaining the choice and what a group passes down. A row's description rises
//! in when it changes, never when a snippet opens.
//!
//! The app rule follows the S55 review (M2): changing the mode keeps the bundle ids and resets
//! the per-app insertion; picking apps keeps the rest of the loaded rule. The picker is the
//! shared `widgets::app_picker`, used through its current API inside the uniquely keyed
//! [`RULE_KEY`] container (S55's fixes change `AppPicker::new` to take an id; until then its
//! inner keys are the picker's fixed ones).

use std::sync::Arc;

use kudzu_core::library::groups::GroupResolver;
use kudzu_core::system::AppInfo;
use kudzu_core::{AppRule, AppRuleMode, CaseMode, FireMode, InsertionMethod, SnippetRecord};
use rune::{col, el, icon, row, text, Cx, El, Key, Span, TextEdit};
use uuid::Uuid;

use crate::shell::MainState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::app_icon::IconCache;
use crate::widgets::app_picker::{self, AppPicker};
use crate::widgets::selection_pill::{Segment, SegmentSize, Segmented};

pub const SECTION_KEY: &str = "editor-behavior";
pub const RULE_KEY: &str = "editor-app-rule";

/// The app picker's search, list and icons, and each row's change count.
pub struct Rules {
    pub picker_open: bool,
    pub query: TextEdit,
    pub icons: IconCache,
    seen: Option<(Uuid, Choices)>,
    pub bumps: [u32; 4],
}

/// What each row's description depends on (`swap` in `behavior.tsx`).
#[derive(Clone, Debug, PartialEq)]
struct Choices([String; 4]);

impl Choices {
    fn of(d: &SnippetRecord) -> Choices {
        Choices([
            format!("{:?}", d.fire_mode),
            format!("{:?}", d.case_mode),
            format!("{:?}", d.insertion),
            format!("{:?}", d.rule.as_ref().map(|r| r.mode)),
        ])
    }
}

impl Rules {
    pub fn new(wake: rune::Waker) -> Rules {
        Rules {
            picker_open: false,
            query: TextEdit::new(""),
            icons: IconCache::new(wake),
            seen: None,
            bumps: [0; 4],
        }
    }

    /// Another snippet opened: rows start still and the list is closed.
    pub fn opened(&mut self) {
        self.picker_open = false;
        self.query = TextEdit::new("");
        self.seen = None;
        self.bumps = [0; 4];
    }

    /// Counts real changes of each row's choice. True when one changed.
    pub fn observe(&mut self, d: &SnippetRecord) -> bool {
        let next = Choices::of(d);
        let changed = match &self.seen {
            Some((id, before)) if *id == d.id => {
                let mut any = false;
                for i in 0..4 {
                    if before.0[i] != next.0[i] {
                        self.bumps[i] += 1;
                        any = true;
                    }
                }
                any
            }
            _ => false,
        };
        self.seen = Some((d.id, next));
        changed
    }
}

/// `INSERTION_LABEL`.
fn insertion_label(m: InsertionMethod) -> &'static str {
    match m {
        InsertionMethod::Automatic => "Automatic",
        InsertionMethod::Type => "Typing",
        InsertionMethod::Paste => "Pasting",
    }
}

fn app_name(apps: &[AppInfo], id: &str) -> String {
    apps.iter()
        .find(|a| a.bundle_id == id)
        .map_or_else(|| id.to_owned(), |a| a.name.clone())
}

/// `describeRule`.
pub fn describe_rule(rule: Option<&AppRule>, apps: &[AppInfo]) -> String {
    let Some(rule) = rule else {
        return "Works in every app".into();
    };
    if rule.bundle_ids.is_empty() {
        return match rule.mode {
            AppRuleMode::Allow => "Disabled everywhere".into(),
            AppRuleMode::Deny => "Works in every app".into(),
        };
    }
    let names: Vec<String> = rule
        .bundle_ids
        .iter()
        .take(3)
        .map(|b| app_name(apps, b))
        .collect();
    let mut list = names.join(", ");
    if rule.bundle_ids.len() > 3 {
        list.push_str(&format!(" +{}", rule.bundle_ids.len() - 3));
    }
    match rule.mode {
        AppRuleMode::Allow => format!("Only in {list}"),
        AppRuleMode::Deny => format!("Everywhere except {list}"),
    }
}

/// The Insert by row's sentence.
pub fn insertion_text(d: &SnippetRecord, inherited: Option<(InsertionMethod, &str)>) -> String {
    let formatted = d.formatting.as_ref().is_some_and(|f| f.source == d.content);
    if formatted {
        let typed = d.insertion.or(inherited.map(|(m, _)| m)) == Some(InsertionMethod::Type);
        return if typed {
            "Types plain text. Images use their text alternatives."
        } else {
            "Pastes formatting and images. Apps that need plain text use the text alternative."
        }
        .into();
    }
    match (d.insertion, inherited) {
        (Some(InsertionMethod::Type), _) => "Types the text key by key. Most compatible.".into(),
        (Some(InsertionMethod::Paste), _) => {
            "Pastes through the clipboard, then restores it. Best for long text.".into()
        }
        (Some(InsertionMethod::Automatic), _) => "Types short text and pastes long text.".into(),
        (None, Some((m, from))) => format!("Inherited from “{from}”: {}", insertion_label(m)),
        (None, None) => "Default — typing for short text, pasting for long or in terminals.".into(),
    }
}

/// The Apps row's sentence.
pub fn apps_text(
    d: &SnippetRecord,
    inherited: Option<(&AppRule, &str)>,
    apps: &[AppInfo],
) -> String {
    match (&d.rule, inherited) {
        (Some(rule), _) => describe_rule(Some(rule), apps),
        (None, Some((rule, from))) => format!(
            "Inherited from “{from}”: {}",
            describe_rule(Some(rule), apps)
        ),
        (None, None) => "Works in every app (except excluded apps in Settings).".into(),
    }
}

/// M2: a new mode keeps the ids and drops the per-app insertion; the same mode keeps the rule.
pub fn with_mode(rule: Option<&AppRule>, mode: Option<AppRuleMode>) -> Option<AppRule> {
    let mode = mode?;
    Some(match rule {
        Some(r) if r.mode == mode => r.clone(),
        Some(r) => AppRule {
            mode,
            bundle_ids: r.bundle_ids.clone(),
            insertion: None,
        },
        None => AppRule {
            mode,
            bundle_ids: Vec::new(),
            insertion: None,
        },
    })
}

fn insertion_value(m: Option<InsertionMethod>) -> &'static str {
    match m {
        None => "default",
        Some(InsertionMethod::Type) => "type",
        Some(InsertionMethod::Paste) => "paste",
        Some(InsertionMethod::Automatic) => "automatic",
    }
}

fn insertion_from(v: &str) -> Option<InsertionMethod> {
    match v {
        "type" => Some(InsertionMethod::Type),
        "paste" => Some(InsertionMethod::Paste),
        _ => None,
    }
}

/// Icon fetches for the chips and the open list.
pub(crate) fn request_icons(s: &mut MainState) {
    let Some(rule) = s.editor.draft().and_then(|d| d.rule.clone()) else {
        return;
    };
    let installed = s.svc.apps.installed().get();
    let needed = app_picker::icons_needed(
        &rule.bundle_ids,
        &installed,
        &s.editor.rules.query.text,
        s.editor.rules.picker_open,
    );
    let apps = Arc::clone(&s.svc.apps);
    for (id, size) in needed {
        s.editor.rules.icons.request(&apps, &id, size);
    }
}

fn row_el(theme: &Theme, key: &str, title: &str, description: El, bump: u32, control: El) -> El {
    let mut description = description.key(format!("{key}-description-{bump}"));
    if bump > 0 {
        description = description.transition(crate::motion::rise());
    }
    row()
        .key(key.to_owned())
        .w_full()
        .py(12.0)
        .gap(24.0)
        .items_center()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(
                    text(title.to_owned())
                        .font(theme.ty.body_medium.lh(18.5625))
                        .color(theme.c.foreground),
                )
                .child(description),
        )
        .child(control)
}

fn sentence(theme: &Theme, s: String) -> El {
    text(s)
        .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
        .color(theme.c.muted_foreground)
        .wrap()
}

pub fn view(s: &MainState, cx: &Cx<MainState>, theme: &Theme, overlays: &mut Vec<El>) -> El {
    let Some(d) = s.editor.draft() else {
        return col();
    };
    let c = &theme.c;
    let groups = &s.library_snapshot().groups;
    let resolver = GroupResolver::new(groups);
    let inherited_insertion = resolver
        .inherited_insertion(d.group_id)
        .map(|(m, g)| (m, g.name.as_str()));
    let inherited_rule = resolver
        .inherited_rule(d.group_id)
        .map(|(r, g)| (r, g.name.as_str()));
    let installed = s.svc.apps.installed().get();
    let bumps = s.editor.rules.bumps;

    let expand = Segmented::new(
        "editor-fire",
        "When to expand",
        match d.fire_mode {
            FireMode::Immediate => "immediate",
            FireMode::WholeWord => "wholeWord",
        },
    )
    .option(Segment::new("immediate", "Instantly"))
    .option(Segment::new("wholeWord", "After the word"))
    .build(theme, cx, |st: &mut MainState, v| {
        let mode = if v == "wholeWord" {
            FireMode::WholeWord
        } else {
            FireMode::Immediate
        };
        super::edit_draft(st, |d| d.fire_mode = mode);
    });
    let expand_text = match d.fire_mode {
        FireMode::Immediate => "As soon as the last trigger character is typed.",
        FireMode::WholeWord => "After you finish the word — space, return or punctuation.",
    };

    let case = Segmented::new(
        "editor-case",
        "Capitalization",
        match d.case_mode {
            CaseMode::Propagate => "propagate",
            CaseMode::Exact => "exact",
        },
    )
    .option(Segment::new("propagate", "Adapt"))
    .option(Segment::new("exact", "Exact"))
    .build(theme, cx, |st: &mut MainState, v| {
        let mode = if v == "exact" {
            CaseMode::Exact
        } else {
            CaseMode::Propagate
        };
        super::edit_draft(st, |d| d.case_mode = mode);
    });
    let case_text = match d.case_mode {
        CaseMode::Propagate => {
            let words = "Follows how you type the trigger: ;sig → text, ;SIG → TEXT.";
            let (a, b) = (
                words.find(";sig").unwrap_or(0),
                words.find(";SIG").unwrap_or(0),
            );
            sentence(theme, words.into())
                .spans(vec![Span::new(a, a + 4).mono(), Span::new(b, b + 4).mono()])
        }
        CaseMode::Exact => sentence(
            theme,
            "The trigger must match exactly; text is never changed.".into(),
        ),
    };

    let insert = Segmented::new(
        "editor-insertion",
        "Insertion method",
        insertion_value(d.insertion),
    )
    .option(Segment::new("default", "Default"))
    .option(Segment::new("type", "Type"))
    .option(Segment::new("paste", "Paste"))
    .build(theme, cx, |st: &mut MainState, v| {
        let m = insertion_from(v);
        super::edit_draft(st, |d| d.insertion = m);
    });

    let choice = match d.rule.as_ref().map(|r| r.mode) {
        None => "inherit",
        Some(AppRuleMode::Allow) => "allow",
        Some(AppRuleMode::Deny) => "deny",
    };
    let rule = Segmented::new("editor-rule", "App rule", choice)
        .option(Segment::new(
            "inherit",
            if inherited_rule.is_some() {
                "Inherit"
            } else {
                "All apps"
            },
        ))
        .option(Segment::new("allow", "Only in…"))
        .option(Segment::new("deny", "Except…"))
        .build(theme, cx, |st: &mut MainState, v| {
            let mode = match v {
                "allow" => Some(AppRuleMode::Allow),
                "deny" => Some(AppRuleMode::Deny),
                _ => None,
            };
            if mode.is_none() {
                st.editor.rules.picker_open = false;
            }
            super::edit_draft(st, |d| d.rule = with_mode(d.rule.as_ref(), mode));
        });

    let mut apps = col()
        .key("editor-behavior-apps")
        .w_full()
        .py(12.0)
        .child(row_el(
            theme,
            "editor-row-apps",
            "Apps",
            sentence(theme, apps_text(d, inherited_rule, &installed)),
            bumps[3],
            rule,
        ));
    if let Some(r) = &d.rule {
        apps = apps.child(picker(s, cx, theme, r, &installed, overlays));
    }

    let rows = [
        row_el(
            theme,
            "editor-row-expand",
            "Expand",
            sentence(theme, expand_text.into()),
            bumps[0],
            expand,
        ),
        row_el(
            theme,
            "editor-row-case",
            "Capitalization",
            case_text,
            bumps[1],
            case,
        ),
        row_el(
            theme,
            "editor-row-insert",
            "Insert by",
            sentence(theme, insertion_text(d, inherited_insertion)),
            bumps[2],
            insert,
        ),
        apps,
    ];
    let mut list = col().w_full();
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            list = list.child(el().w_full().h(1.0).fill(c.hairline).a11y_hidden());
        }
        list = list.child(r);
    }
    let card = col()
        .key(SECTION_KEY)
        .role("region")
        .label("Behavior")
        .w_full()
        .px(16.0)
        .radius(radius::XL)
        .fill(c.surface)
        .child(
            el().role("heading")
                .label("Behavior")
                .absolute(15.0, -1.0)
                .w(1.0)
                .h(1.0),
        )
        .child(list);
    theme
        .depth
        .card
        .iter()
        .fold(card, |e, shadow| e.box_shadow(*shadow))
}

fn picker(
    s: &MainState,
    cx: &Cx<MainState>,
    theme: &Theme,
    rule: &AppRule,
    installed: &[AppInfo],
    overlays: &mut Vec<El>,
) -> El {
    let add_label = match rule.mode {
        AppRuleMode::Allow => "Allow an app",
        AppRuleMode::Deny => "Block an app",
    };
    let picker = AppPicker {
        add_label,
        ..AppPicker::new(
            &rule.bundle_ids,
            installed,
            &s.editor.rules.query,
            &s.editor.rules.icons,
        )
    }
    .open(s.editor.rules.picker_open)
    .build(
        theme,
        cx,
        |st: &mut MainState| &mut st.editor.rules.query,
        toggle_app,
        |st: &mut MainState, open| st.editor.rules.picker_open = open,
        overlays,
    );
    let mut out = col()
        .key(RULE_KEY)
        .w_full()
        .gap(12.0)
        .pb(4.0)
        .transition(crate::motion::rise())
        // Escape in the picker's search closes the list, not the editor (S55 moves this into
        // the picker).
        .on_key(|st: &mut MainState, ev, _| {
            let bare = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.shift;
            if ev.key == Key::Escape
                && bare
                && st.editor.rules.picker_open
                && !st.editor.rules.query.is_composing()
            {
                st.editor.rules.picker_open = false;
                return true;
            }
            false
        })
        .child(picker);
    if rule.mode == AppRuleMode::Allow && !rule.bundle_ids.is_empty() {
        let c = &theme.c;
        out = out.child(
            row()
                .key("editor-rule-insertion")
                .gap(8.0)
                .items_center()
                .transition(crate::motion::rise())
                .child(
                    icon(&icons::SETTINGS_2, 14.0)
                        .color(c.muted_foreground)
                        .a11y_hidden(),
                )
                .child(
                    text("In these apps, insert by")
                        .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                        .color(c.muted_foreground),
                )
                .child(
                    Segmented::new(
                        "editor-rule-insertion-choice",
                        "Insertion in these apps",
                        insertion_value(rule.insertion),
                    )
                    .size(SegmentSize::Sm)
                    .option(Segment::new("default", "Default"))
                    .option(Segment::new("type", "Type"))
                    .option(Segment::new("paste", "Paste"))
                    .build(theme, cx, |st: &mut MainState, v| {
                        let m = insertion_from(v);
                        super::edit_draft(st, |d| {
                            if let Some(r) = &mut d.rule {
                                r.insertion = m;
                            }
                        });
                    }),
                ),
        );
    }
    out
}

/// Adds or removes `id`, keeping the rest of the rule (M2).
fn toggle_app(s: &mut MainState, id: String) {
    super::edit_draft(s, |d| {
        if let Some(rule) = &mut d.rule {
            if let Some(i) = rule.bundle_ids.iter().position(|b| *b == id) {
                rule.bundle_ids.remove(i);
            } else {
                rule.bundle_ids.push(id);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str) -> AppInfo {
        AppInfo {
            bundle_id: id.into(),
            name: name.into(),
            pid: None,
            path: None,
        }
    }

    fn rule(mode: AppRuleMode, ids: &[&str], insertion: Option<InsertionMethod>) -> AppRule {
        AppRule {
            mode,
            bundle_ids: ids.iter().map(|s| s.to_string()).collect(),
            insertion,
        }
    }

    #[test]
    fn rules_read_as_tauri_describes_them() {
        let apps = [app("com.apple.mail", "Mail")];
        assert_eq!(describe_rule(None, &apps), "Works in every app");
        assert_eq!(
            describe_rule(Some(&rule(AppRuleMode::Allow, &[], None)), &apps),
            "Disabled everywhere"
        );
        assert_eq!(
            describe_rule(Some(&rule(AppRuleMode::Deny, &[], None)), &apps),
            "Works in every app"
        );
        assert_eq!(
            describe_rule(
                Some(&rule(AppRuleMode::Allow, &["com.apple.mail", "a.b"], None)),
                &apps
            ),
            "Only in Mail, a.b"
        );
        assert_eq!(
            describe_rule(
                Some(&rule(
                    AppRuleMode::Deny,
                    &["a.a", "b.b", "c.c", "d.d", "e.e"],
                    None
                )),
                &apps
            ),
            "Everywhere except a.a, b.b, c.c +2"
        );
    }

    #[test]
    fn a_new_mode_keeps_the_apps_and_drops_the_insertion_override() {
        let loaded = rule(
            AppRuleMode::Allow,
            &["com.apple.Terminal"],
            Some(InsertionMethod::Type),
        );
        assert_eq!(
            with_mode(Some(&loaded), Some(AppRuleMode::Allow)),
            Some(loaded.clone())
        );
        assert_eq!(
            with_mode(Some(&loaded), Some(AppRuleMode::Deny)),
            Some(rule(AppRuleMode::Deny, &["com.apple.Terminal"], None))
        );
        assert_eq!(with_mode(Some(&loaded), None), None);
        assert_eq!(
            with_mode(None, Some(AppRuleMode::Allow)),
            Some(rule(AppRuleMode::Allow, &[], None))
        );
    }
}
