//! S51 behaviour (RUL-6): each control flips the draft, which the library then holds; every
//! description's copy; inherited sources; and the app rule with the shared picker (M2: a new
//! mode keeps the apps and drops the per-app insertion).

use kudzu_core::formatting::Formatting;
use kudzu_core::{AppRule, AppRuleMode, CaseMode, FireMode, InsertionMethod};

use super::editor::{draft, group, has_text, id, rig_with, snippet, stored};
use super::{cmd, Rig};
use crate::main::editor::behavior::{self, describe_rule, with_mode};
use kudzu_core::system::AppInfo;

/// Presses the control named `label` inside the editor's rule container (the picker's own
/// keys are its business; S55 is changing them).
fn click_in_rule(rig: &mut Rig, label: &str) {
    let elements = rig.rt.elements();
    let scope = elements
        .iter()
        .find(|e| e.key.as_deref() == Some(behavior::RULE_KEY))
        .map(|e| format!("{}/", e.path))
        .expect("the rule container");
    let r = elements
        .iter()
        .find(|e| e.path.starts_with(&scope) && e.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no “{label}” in the rule container"))
        .rect;
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
    rig.settle();
}

/// Tall enough that every behaviour row is on screen without scrolling.
fn tall(mut rig: Rig) -> Rig {
    rig.rt.set_viewport((1180.0, 1400.0), 1.0);
    rig.settle();
    rig
}

fn one() -> Rig {
    tall(rig_with(vec![snippet(1, "Sig", &[";s"], "Hello")], vec![]))
}

#[test]
fn each_choice_flips_the_draft_and_reads_as_tauri() {
    let mut rig = one();
    assert!(has_text(
        &rig,
        "As soon as the last trigger character is typed."
    ));
    assert!(has_text(
        &rig,
        "Default — typing for short text, pasting for long or in terminals."
    ));
    assert!(has_text(
        &rig,
        "Works in every app (except excluded apps in Settings)."
    ));
    assert!(has_text(&rig, "All apps"));

    rig.click_key("editor-fire:wholeWord");
    assert_eq!(draft(&rig).fire_mode, FireMode::WholeWord);
    assert!(has_text(
        &rig,
        "After you finish the word — space, return or punctuation."
    ));
    assert_eq!(
        rig.rt.state.editor.rules.bumps[0], 1,
        "the description rises in"
    );

    assert!(has_text(
        &rig,
        "Follows how you type the trigger: ;sig → text, ;SIG → TEXT."
    ));
    rig.click_key("editor-case:exact");
    assert_eq!(draft(&rig).case_mode, CaseMode::Exact);
    assert!(has_text(
        &rig,
        "The trigger must match exactly; text is never changed."
    ));

    rig.click_key("editor-insertion:type");
    assert_eq!(draft(&rig).insertion, Some(InsertionMethod::Type));
    assert!(has_text(
        &rig,
        "Types the text key by key. Most compatible."
    ));
    rig.click_key("editor-insertion:paste");
    assert!(has_text(
        &rig,
        "Pastes through the clipboard, then restores it. Best for long text."
    ));

    rig.press(cmd('s'));
    let saved = stored(&rig, 1);
    assert_eq!(saved.fire_mode, FireMode::WholeWord);
    assert_eq!(saved.case_mode, CaseMode::Exact);
    assert_eq!(saved.insertion, Some(InsertionMethod::Paste));

    rig.click_key("editor-insertion:default");
    assert_eq!(draft(&rig).insertion, None);
}

#[test]
fn inherited_settings_name_their_group() {
    let mut email = group(20, "Email", None);
    email.insertion = Some(InsertionMethod::Type);
    email.rule = Some(AppRule {
        mode: AppRuleMode::Allow,
        bundle_ids: vec!["com.apple.mail".into()],
        insertion: None,
    });
    let mut mine = snippet(1, "Sig", &[";s"], "Hello");
    mine.group_id = Some(id(20));
    let rig = rig_with(vec![mine], vec![email]);
    assert!(has_text(&rig, "Inherited from “Email”: Typing"));
    assert!(has_text(&rig, "Inherited from “Email”: Only in Mail"));
    assert!(
        has_text(&rig, "Inherit"),
        "the rule segment says what it does"
    );
}

#[test]
fn a_formatted_snippet_explains_formatted_insertion() {
    let mut rich = snippet(1, "Rich", &[";r"], "Hello");
    rich.formatting = Some(Formatting {
        source: "Hello".into(),
        spans: Vec::new(),
    });
    let mut typed = rich.clone();
    typed.id = id(2);
    typed.insertion = Some(InsertionMethod::Type);
    let mut rig = rig_with(vec![rich, typed], vec![]);
    assert!(has_text(
        &rig,
        "Pastes formatting and images. Apps that need plain text use the text alternative."
    ));
    super::editor::open(&mut rig, 2);
    assert!(has_text(
        &rig,
        "Types plain text. Images use their text alternatives."
    ));
}

#[test]
fn the_app_rule_keeps_its_apps_and_resets_the_insertion_on_a_new_mode() {
    let mut rig = one();
    rig.click_key("editor-rule:allow");
    assert_eq!(
        draft(&rig).rule,
        Some(AppRule {
            mode: AppRuleMode::Allow,
            bundle_ids: vec![],
            insertion: None,
        })
    );
    assert!(has_text(&rig, "Disabled everywhere"));
    assert!(has_text(&rig, "Allow an app"));
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(behavior::RULE_KEY)));

    click_in_rule(&mut rig, "Allow an app");
    assert!(rig.rt.state.editor.rules.picker_open);
    // The list is on the overlay layer; its row is the only "Mail" while nothing is chosen.
    rig.click_text("Mail");
    assert!(
        rig.rt.state.editor.rules.picker_open,
        "the list stays open across checks"
    );
    assert_eq!(draft(&rig).rule.unwrap().bundle_ids, ["com.apple.mail"]);
    assert!(has_text(&rig, "Only in Mail"));
    rig.rt.update(|s, _| s.editor.rules.picker_open = false);
    rig.settle();

    assert!(has_text(&rig, "In these apps, insert by"));
    rig.click_key("editor-rule-insertion-choice:type");
    assert_eq!(
        draft(&rig).rule.unwrap().insertion,
        Some(InsertionMethod::Type)
    );
    rig.click_key("editor-rule:allow");
    assert_eq!(
        draft(&rig).rule.unwrap().insertion,
        Some(InsertionMethod::Type),
        "the same mode keeps it"
    );

    rig.click_key("editor-rule:deny");
    assert_eq!(
        draft(&rig).rule,
        Some(AppRule {
            mode: AppRuleMode::Deny,
            bundle_ids: vec!["com.apple.mail".into()],
            insertion: None,
        })
    );
    assert!(has_text(&rig, "Everywhere except Mail"));
    assert!(has_text(&rig, "Block an app"));
    assert!(!has_text(&rig, "In these apps, insert by"));

    rig.press(cmd('s'));
    assert_eq!(stored(&rig, 1).rule.unwrap().mode, AppRuleMode::Deny);
    rig.click_key("editor-rule:inherit");
    assert_eq!(draft(&rig).rule, None);
    assert!(!rig
        .rt
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(behavior::RULE_KEY)));
}

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
