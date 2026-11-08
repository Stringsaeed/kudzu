//! `behavior.tsx`: when the snippet expands, how it treats capitals, how it's inserted, and in
//! which apps, each with what it inherits from its groups.

use kudzu_core::{AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod};
use rune::{col, icon, rich, row, text, Cx, El, Span};
use uuid::Uuid;

use super::{update, EditorState};
use crate::ui::icons;
use crate::ui::kit::app_picker::{app_name, installed, AppPicker, AppPickerState};
use crate::ui::kit::segmented::{Segmented, SegmentedSize};
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

#[derive(Default, Clone)]
pub struct BehaviorState {
    pub apps: AppPickerState,
}

/// `inherited`: the nearest ancestor group that sets `key`, with its value.
pub fn inherited<T: Clone>(groups: &[GroupRecord], group_id: Option<Uuid>, key: impl Fn(&GroupRecord) -> Option<T>) -> Option<(T, &GroupRecord)> {
    let mut seen = std::collections::HashSet::new();
    let mut g = group_id.and_then(|id| groups.iter().find(|g| g.id == id));
    while let Some(group) = g {
        if !seen.insert(group.id) {
            break;
        }
        if let Some(v) = key(group) {
            return Some((v, group));
        }
        g = group.parent_id.and_then(|p| groups.iter().find(|g| g.id == p));
    }
    None
}

fn insertion_label(m: InsertionMethod) -> &'static str {
    match m {
        InsertionMethod::Automatic => "Automatic",
        InsertionMethod::Type => "Typing",
        InsertionMethod::Paste => "Pasting",
    }
}

fn insertion_value(m: Option<InsertionMethod>) -> &'static str {
    match m {
        None => "default",
        Some(InsertionMethod::Type) => "type",
        Some(InsertionMethod::Paste) => "paste",
        Some(InsertionMethod::Automatic) => "automatic",
    }
}

fn parse_insertion(v: &str) -> Option<InsertionMethod> {
    match v {
        "type" => Some(InsertionMethod::Type),
        "paste" => Some(InsertionMethod::Paste),
        "automatic" => Some(InsertionMethod::Automatic),
        _ => None,
    }
}

/// `Row`: title and description on the left, the control on the right, `py-3`.
fn setting(th: &Theme, title: &str, description: El, control: El) -> El {
    row()
        .w_full()
        .py(12.0)
        .gap(24.0)
        .center_y()
        .child(
            col().grow(1.0).min_w(0.0)
                .child(text(title.to_string()).font(ty::BASE.weight_of(weight::MEDIUM)).color(th.t.foreground))
                .child(description),
        )
        .child(control)
}

fn desc(th: &Theme, s: impl Into<String>) -> El {
    text(s.into()).font(ty::XS).color(th.t.muted_foreground).wrap()
}

pub fn view(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let d = st.draft.as_ref().expect("draft");
    let groups = &s.library.groups;
    let apps = installed();
    let inherited_insertion = inherited(groups, d.group_id, |g| g.insertion);
    let inherited_rule = inherited(groups, d.group_id, |g| g.rule.clone());
    let rule_choice = match &d.rule {
        None => "inherit",
        Some(r) if r.mode == AppRuleMode::Allow => "allow",
        Some(_) => "deny",
    };
    let describe_rule = |rule: Option<&AppRule>| -> String {
        let Some(rule) = rule else { return "Works in every app".into() };
        if rule.bundle_ids.is_empty() {
            return if rule.mode == AppRuleMode::Allow { "Disabled everywhere".into() } else { "Works in every app".into() };
        }
        let mut names = rule.bundle_ids.iter().take(3).map(|b| app_name(&apps, b)).collect::<Vec<_>>().join(", ");
        if rule.bundle_ids.len() > 3 {
            names.push_str(&format!(" +{}", rule.bundle_ids.len() - 3));
        }
        if rule.mode == AppRuleMode::Allow { format!("Only in {names}") } else { format!("Everywhere except {names}") }
    };

    // Expand.
    let fire = Segmented::new("behavior-fire", "When to expand")
        .option("immediate", "Instantly")
        .option("wholeWord", "After the word")
        .selected(if d.fire_mode == FireMode::Immediate { "immediate" } else { "wholeWord" })
        .on_change(|s: &mut MainUi, v: &str| {
            let m = if v == "immediate" { FireMode::Immediate } else { FireMode::WholeWord };
            update(s, move |d| d.fire_mode = m);
        })
        .build(th, cx);
    let fire_desc = if d.fire_mode == FireMode::Immediate { "As soon as the last trigger character is typed." } else { "After you finish the word — space, return or punctuation." };

    // Capitalization.
    let case = Segmented::new("behavior-case", "Capitalization")
        .option("propagate", "Adapt")
        .option("exact", "Exact")
        .selected(if d.case_mode == CaseMode::Propagate { "propagate" } else { "exact" })
        .on_change(|s: &mut MainUi, v: &str| {
            let m = if v == "propagate" { CaseMode::Propagate } else { CaseMode::Exact };
            update(s, move |d| d.case_mode = m);
        })
        .build(th, cx);
    let case_desc = if d.case_mode == CaseMode::Propagate {
        let a = "Follows how you type the trigger: ";
        let s = format!("{a};sig → text, ;SIG → TEXT.");
        let i1 = a.len();
        let i2 = s.find(";SIG").unwrap_or(0);
        rich(s, vec![Span::new(i1, i1 + 4).mono(), Span::new(i2, i2 + 4).mono()]).font(ty::XS).color(t.muted_foreground).wrap()
    } else {
        desc(th, "The trigger must match exactly; text is never changed.")
    };

    // Insert by.
    let formatted = d.formatting.as_ref().is_some_and(|f| f.source == d.content);
    let insertion_desc = if formatted {
        if d.insertion.or(inherited_insertion.map(|x| x.0)) == Some(InsertionMethod::Type) {
            "Types plain text. Images use their text alternatives.".to_string()
        } else {
            "Pastes formatting and images. Apps that need plain text use the text alternative.".to_string()
        }
    } else {
        match d.insertion {
            Some(InsertionMethod::Type) => "Types the text key by key. Most compatible.".into(),
            Some(InsertionMethod::Paste) => "Pastes through the clipboard, then restores it. Best for long text.".into(),
            Some(InsertionMethod::Automatic) => "Types short text and pastes long text.".into(),
            None => match inherited_insertion {
                Some((m, g)) => format!("Inherited from “{}”: {}", g.name, insertion_label(m)),
                None => "Default — typing for short text, pasting for long or in terminals.".into(),
            },
        }
    };
    let insertion = Segmented::new("behavior-insertion", "Insertion method")
        .option("default", "Default")
        .option("type", "Type")
        .option("paste", "Paste")
        .selected(insertion_value(d.insertion))
        .on_change(|s: &mut MainUi, v: &str| {
            let m = parse_insertion(v);
            update(s, move |d| d.insertion = m);
        })
        .build(th, cx);

    // Apps.
    let apps_desc = if rule_choice == "inherit" {
        match &inherited_rule {
            Some((r, g)) => format!("Inherited from “{}”: {}", g.name, describe_rule(Some(r))),
            None => "Works in every app (except excluded apps in Settings).".into(),
        }
    } else {
        describe_rule(d.rule.as_ref())
    };
    let rule = Segmented::new("behavior-rule", "App rule")
        .option("inherit", if inherited_rule.is_some() { "Inherit" } else { "All apps" })
        .option("allow", "Only in…")
        .option("deny", "Except…")
        .selected(rule_choice)
        .on_change(|s: &mut MainUi, v: &str| {
            let v = v.to_string();
            update(s, move |d| {
                d.rule = match v.as_str() {
                    "inherit" => None,
                    mode => Some(AppRule {
                        mode: if mode == "allow" { AppRuleMode::Allow } else { AppRuleMode::Deny },
                        bundle_ids: d.rule.as_ref().map(|r| r.bundle_ids.clone()).unwrap_or_default(),
                        insertion: d.rule.as_ref().and_then(|r| r.insertion),
                    }),
                };
            });
        })
        .build(th, cx);
    let mut apps_block = col().w_full().py(12.0).child(setting(th, "Apps", desc(th, apps_desc), rule).py(0.0));
    if let Some(r) = &d.rule {
        let picker = AppPicker::new("behavior-apps", &r.bundle_ids, &st.behavior.apps, |s: &mut MainUi| &mut s.editor.behavior.apps)
            .add_label(if r.mode == AppRuleMode::Allow { "Allow an app" } else { "Block an app" })
            .on_change(|s: &mut MainUi, ids, _| {
                update(s, move |d| {
                    if let Some(r) = d.rule.as_mut() {
                        r.bundle_ids = ids;
                    }
                })
            })
            .build(cx, layer);
        let mut extra = col().key("behavior-apps-extra").gap(12.0).pb(4.0).pt(12.0).transition(super::rise()).child(picker);
        if r.mode == AppRuleMode::Allow && !r.bundle_ids.is_empty() {
            let seg = Segmented::new("behavior-rule-insertion", "Insertion in these apps")
                .size(SegmentedSize::Sm)
                .option("default", "Default")
                .option("type", "Type")
                .option("paste", "Paste")
                .selected(insertion_value(r.insertion))
                .on_change(|s: &mut MainUi, v: &str| {
                    let m = parse_insertion(v);
                    update(s, move |d| {
                        if let Some(r) = d.rule.as_mut() {
                            r.insertion = m;
                        }
                    });
                })
                .build(th, cx);
            extra = extra.child(
                row().gap(8.0).center_y()
                    .child(icon(&icons::SETTINGS_2, 14.0).color(t.muted_foreground))
                    .child(text("In these apps, insert by").font(ty::XS).color(t.muted_foreground))
                    .child(seg),
            );
        }
        apps_block = apps_block.child(extra);
    }

    let divider = || rune::el().w_full().h(1.0).fill(t.hairline);
    col()
        .key("behavior")
        .label("Behavior")
        .w_full()
        .radius(radius::XL)
        .fill(t.surface)
        .elevate(&t.elev_card)
        .px(16.0)
        .child(setting(th, "Expand", desc(th, fire_desc), fire))
        .child(divider())
        .child(setting(th, "Capitalization", case_desc, case))
        .child(divider())
        .child(setting(th, "Insert by", desc(th, insertion_desc), insertion))
        .child(divider())
        .child(apps_block)
}
