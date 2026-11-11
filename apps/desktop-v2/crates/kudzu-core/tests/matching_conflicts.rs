//! Trigger conflicts, effective settings and trigger validation
//! (ported from TriggerConflictsTests, EffectiveSettingsTests, TriggerValidatorTests).

mod matching_support;

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use kudzu_core::library::conflicts::{find_conflicts, ConflictKind, TriggerConflict};
use kudzu_core::library::groups::{rule_summary, EffectiveSnippet, GroupResolver};
use kudzu_core::library::triggers::{
    trigger_tokens, validate_trigger, TriggerIssue, MAX_TRIGGER_LENGTH,
};
use kudzu_core::matching::MatchingConfig;
use kudzu_core::{AppRule, AppRuleMode, GroupRecord, InsertionMethod, SnippetRecord};
use matching_support::*;
use uuid::Uuid;

fn conflicts(snippets: &[&Snip], groups: &[GroupRecord]) -> Vec<TriggerConflict> {
    find_conflicts(&records(snippets), groups)
}

// Conflicts

#[test]
fn no_conflicts_for_distinct_triggers() {
    assert!(conflicts(&[&snip(&[";a"]), &snip(&[";b"])], &[]).is_empty());
}

#[test]
fn duplicate_trigger_on_two_snippets() {
    let (a, b) = (snip(&[";sig"]), snip(&["x", ";sig"]));
    let found = conflicts(&[&a, &b], &[]);
    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert_eq!(c.kind, ConflictKind::Duplicate);
    assert_eq!(
        HashSet::from([c.snippet_id, c.other_snippet_id]),
        HashSet::from([a.id(), b.id()])
    );
    assert_eq!(
        (c.trigger.as_str(), c.other_trigger.as_str()),
        (";sig", ";sig")
    );
}

#[test]
fn duplicate_case_rules() {
    let prop = snip(&[";Sig"]);
    assert_eq!(conflicts(&[&prop, &snip(&[";sig"]).exact()], &[]).len(), 1);
    assert_eq!(conflicts(&[&prop, &snip(&[";SIG"])], &[]).len(), 1);
    assert!(conflicts(&[&snip(&[";Sig"]).exact(), &snip(&[";sig"]).exact()], &[]).is_empty());
    assert_eq!(
        conflicts(&[&snip(&[";sig"]).exact(), &snip(&[";sig"]).exact()], &[]).len(),
        1
    );
    assert!(conflicts(&[&snip(&[";a", ";A"])], &[]).is_empty());
}

#[test]
fn disabled_snippets_are_ignored() {
    let a = snip(&[";sig"]);
    assert!(conflicts(&[&a, &snip(&[";sig"]).disabled()], &[]).is_empty());
    let mut g = group("g");
    g.is_enabled = false;
    let c = snip(&[";sig"]).group(g.id);
    assert!(conflicts(&[&a, &c], &[g]).is_empty());
}

#[test]
fn immediate_prefix_shadows_longer_trigger() {
    let (short, long) = (snip(&[";s"]), snip(&[";sig"]));
    let found = conflicts(&[&long, &short], &[]);
    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert_eq!(c.kind, ConflictKind::Shadowing);
    assert_eq!(c.snippet_id, short.id());
    assert_eq!(c.trigger, ";s");
    assert_eq!(c.other_snippet_id, long.id());
    assert_eq!(c.other_trigger, ";sig");
}

#[test]
fn shadowing_fire_mode_rules() {
    let found = conflicts(&[&snip(&[";s"]), &snip(&[";sig"]).whole_word()], &[]);
    assert_eq!(
        found.iter().map(|c| c.kind).collect::<Vec<_>>(),
        [ConflictKind::Shadowing]
    );
    let short_ww = snip(&[";s"]).whole_word();
    assert!(conflicts(&[&short_ww, &snip(&[";sig"])], &[]).is_empty());
    assert!(conflicts(&[&short_ww, &snip(&[";sig"]).whole_word()], &[]).is_empty());
}

#[test]
fn shadowing_case_rules() {
    assert_eq!(
        conflicts(&[&snip(&[";S"]), &snip(&[";sig"]).exact()], &[]).len(),
        1
    );
    assert!(conflicts(&[&snip(&[";s"]).exact(), &snip(&[";sig"])], &[]).is_empty());
    assert_eq!(
        conflicts(&[&snip(&[";1"]).exact(), &snip(&[";1x"])], &[]).len(),
        1
    );
    let short = snip(&[";s"]).exact();
    assert_eq!(conflicts(&[&short, &snip(&[";sig"]).exact()], &[]).len(), 1);
    assert!(conflicts(&[&short, &snip(&[";Sig"]).exact()], &[]).is_empty());
}

#[test]
fn prefix_within_same_snippet_and_equal_triggers() {
    assert!(conflicts(&[&snip(&[";s", ";sig"])], &[]).is_empty());
    let found = conflicts(&[&snip(&[";s"]), &snip(&[";s"])], &[]);
    assert_eq!(
        found.iter().map(|c| c.kind).collect::<Vec<_>>(),
        [ConflictKind::Duplicate]
    );
}

#[test]
fn rule_overlap() {
    let a = snip(&[";x"]).rule(allow(&["com.a"]));
    assert!(conflicts(&[&a, &snip(&[";x"]).rule(allow(&["com.b"]))], &[]).is_empty());
    assert_eq!(
        conflicts(&[&a, &snip(&[";x"]).rule(allow(&["com.a", "com.b"]))], &[]).len(),
        1
    );
    assert!(conflicts(&[&a, &snip(&[";x"]).rule(deny(&["com.a"]))], &[]).is_empty());
    assert_eq!(
        conflicts(&[&a, &snip(&[";x"]).rule(deny(&["com.z"]))], &[]).len(),
        1
    );

    let mut g = group("g");
    g.rule = Some(allow(&["com.a"]));
    let grouped = snip(&[";x"]).group(g.id);
    assert!(conflicts(&[&grouped, &snip(&[";x"]).rule(allow(&["com.b"]))], &[g]).is_empty());
}

#[test]
fn conflicts_scale_to_many_snippets() {
    let snippets: Vec<Snip> = (0..3_000).map(|i| snip(&[&format!(";s{i}")])).collect();
    let refs: Vec<&Snip> = snippets.iter().collect();
    let start = Instant::now();
    assert!(!conflicts(&refs, &[]).is_empty());
    assert!(start.elapsed().as_secs() < 5);
}

// Effective settings

fn make_group(
    name: &str,
    parent: Option<Uuid>,
    rule: Option<AppRule>,
    insertion: Option<InsertionMethod>,
) -> GroupRecord {
    GroupRecord {
        parent_id: parent,
        rule,
        insertion,
        ..group(name)
    }
}

#[test]
fn ungrouped_snippet_has_no_restrictions() {
    let resolver = GroupResolver::new(&[]);
    let settings = resolver.settings(None);
    assert!(settings.is_enabled);
    assert!(settings.rule.is_none() && settings.rule_group.is_none());
    assert!(settings.insertion.is_none() && settings.insertion_group.is_none());
    assert_eq!(
        settings.rule_description(&|s| s.to_string()),
        "No restrictions"
    );
    assert_eq!(
        settings.insertion_description(InsertionMethod::Automatic),
        "Default (Automatic, or the app's setting in Settings → Apps)"
    );
    assert_eq!(
        settings.insertion_description(InsertionMethod::Paste),
        "Default (Paste, or the app's setting in Settings → Apps)"
    );
}

#[test]
fn inherits_rule_and_insertion_with_source_group_names() {
    let rule = deny(&["com.tinyspeck.slackmacgap", "com.apple.mail"]);
    let work = make_group(
        "Work",
        None,
        Some(rule.clone()),
        Some(InsertionMethod::Paste),
    );
    let email = make_group("Email", Some(work.id), None, None);
    let resolver = GroupResolver::new(&[work.clone(), email.clone()]);
    let names: HashMap<&str, &str> = [
        ("com.tinyspeck.slackmacgap", "Slack"),
        ("com.apple.mail", "Mail"),
    ]
    .into();
    let name = |id: &str| names.get(id).map_or(id.to_string(), |n| n.to_string());
    let inherited = resolver.settings(Some(email.id));
    assert_eq!(inherited.rule.as_ref(), Some(&rule));
    assert_eq!(inherited.rule_group.as_ref().map(|g| g.id), Some(work.id));
    assert_eq!(inherited.insertion, Some(InsertionMethod::Paste));
    assert_eq!(
        inherited.rule_description(&name),
        "Inherited from group 'Work': Block in Slack, Mail"
    );
    assert_eq!(
        inherited.insertion_description(InsertionMethod::Automatic),
        "Inherited from group 'Work': Paste"
    );
    assert_eq!(
        resolver.inherited_rule(Some(email.id)).map(|(_, g)| g.id),
        Some(work.id)
    );
    assert_eq!(
        resolver.inherited_insertion(Some(email.id)).map(|(m, _)| m),
        Some(InsertionMethod::Paste)
    );
}

#[test]
fn rule_summaries() {
    let coding = make_group(
        "Coding",
        None,
        Some(allow(&["com.apple.Terminal", "com.mitchellh.ghostty"])),
        None,
    );
    let resolver = GroupResolver::new(std::slice::from_ref(&coding));
    let name = |id: &str| match id {
        "com.apple.Terminal" => "Terminal".to_string(),
        "com.mitchellh.ghostty" => "Ghostty".to_string(),
        other => other.to_string(),
    };
    assert_eq!(
        resolver.settings(Some(coding.id)).rule_description(&name),
        "Inherited from group 'Coding': Allow only in Terminal, Ghostty"
    );
    let empty = |mode| AppRule {
        mode,
        bundle_ids: vec![],
        insertion: None,
    };
    assert_eq!(
        rule_summary(&empty(AppRuleMode::Allow), &|s| s.to_string()),
        "Disabled everywhere"
    );
    assert_eq!(
        rule_summary(&empty(AppRuleMode::Deny), &|s| s.to_string()),
        "No restrictions"
    );
}

#[test]
fn nearest_ancestor_wins_independently_for_rule_and_insertion() {
    let inner = allow(&["com.inner"]);
    let top = make_group(
        "Top",
        None,
        Some(deny(&["com.outer"])),
        Some(InsertionMethod::Paste),
    );
    let mid = make_group("Mid", Some(top.id), Some(inner.clone()), None);
    let leaf = make_group("Leaf", Some(mid.id), None, None);
    let resolver = GroupResolver::new(&[top, mid, leaf.clone()]);
    let settings = resolver.settings(Some(leaf.id));
    assert_eq!(settings.rule.as_ref(), Some(&inner));
    assert_eq!(settings.rule_group.as_ref().unwrap().name, "Mid");
    assert_eq!(settings.insertion, Some(InsertionMethod::Paste));
    assert_eq!(settings.insertion_group.as_ref().unwrap().name, "Top");
}

#[test]
fn group_itself_inherits_from_parent_chain_not_itself() {
    let parent_rule = deny(&["com.parent"]);
    let own_rule = allow(&["com.own"]);
    let parent = make_group(
        "Parent",
        None,
        Some(parent_rule.clone()),
        Some(InsertionMethod::Type),
    );
    let child = make_group(
        "Child",
        Some(parent.id),
        Some(own_rule.clone()),
        Some(InsertionMethod::Paste),
    );
    let resolver = GroupResolver::new(&[parent.clone(), child.clone()]);
    let for_snippet = resolver.settings(Some(child.id));
    assert_eq!(for_snippet.rule.as_ref(), Some(&own_rule));
    assert_eq!(for_snippet.insertion, Some(InsertionMethod::Paste));
    let for_child = resolver.inherited_settings_for_group_itself(child.id);
    assert_eq!(for_child.rule.as_ref(), Some(&parent_rule));
    assert_eq!(for_child.rule_group.as_ref().map(|g| g.id), Some(parent.id));
    assert_eq!(for_child.insertion, Some(InsertionMethod::Type));
    let for_top = resolver.inherited_settings_for_group_itself(parent.id);
    assert!(for_top.rule.is_none() && for_top.insertion.is_none());
}

#[test]
fn group_itself_inheritance_handles_cycles() {
    let (a_id, b_id) = (Uuid::new_v4(), Uuid::new_v4());
    let a = GroupRecord {
        id: a_id,
        ..make_group("A", Some(b_id), Some(allow(&["com.a"])), None)
    };
    let b = GroupRecord {
        id: b_id,
        ..make_group("B", Some(a_id), None, Some(InsertionMethod::Paste))
    };
    let resolver = GroupResolver::new(&[a, b]);
    let for_a = resolver.inherited_settings_for_group_itself(a_id);
    assert!(for_a.rule.is_none());
    assert_eq!(for_a.insertion, Some(InsertionMethod::Paste));
    assert_eq!(for_a.insertion_group.as_ref().map(|g| g.id), Some(b_id));
}

#[test]
fn effective_snippet_precedence() {
    let mut rule = allow(&["com.term"]);
    rule.insertion = Some(InsertionMethod::Paste);
    let g = make_group("G", None, Some(rule), Some(InsertionMethod::Type));
    let resolver = GroupResolver::new(std::slice::from_ref(&g));
    let config = MatchingConfig {
        default_insertion: InsertionMethod::Automatic,
        app_insertions: [
            ("com.term".to_string(), InsertionMethod::Automatic),
            ("com.other".to_string(), InsertionMethod::Paste),
        ]
        .into(),
        ..MatchingConfig::default()
    };
    let grouped = |group: &GroupRecord| SnippetRecord {
        group_id: Some(group.id),
        ..SnippetRecord::new("S", vec![";1".into()], "x")
    };
    let eff1 = EffectiveSnippet::new(&grouped(&g), &resolver);
    assert_eq!(eff1.rule_group.as_ref().map(|x| x.id), Some(g.id));
    assert_eq!(eff1.insertion_group.as_ref().map(|x| x.id), Some(g.id));
    assert_eq!(
        eff1.insertion_for(Some("com.term"), &config),
        InsertionMethod::Paste
    );

    let g2 = make_group("G2", None, None, Some(InsertionMethod::Type));
    let resolver2 = GroupResolver::new(std::slice::from_ref(&g2));
    let eff2 = EffectiveSnippet::new(&grouped(&g2), &resolver2);
    assert_eq!(
        eff2.insertion_for(Some("com.other"), &config),
        InsertionMethod::Type
    );

    let eff3 = EffectiveSnippet::new(
        &SnippetRecord::new("S3", vec![";3".into()], "x"),
        &resolver2,
    );
    assert_eq!(
        eff3.insertion_for(Some("com.other"), &config),
        InsertionMethod::Paste
    );
    assert_eq!(
        eff3.insertion_for(Some("com.unlisted"), &config),
        InsertionMethod::Automatic
    );
}

// Trigger validation

#[test]
fn validates_triggers() {
    assert_eq!(validate_trigger("  ;sig ", &[], false), Ok(";sig".into()));
    assert_eq!(validate_trigger("", &[], false), Err(TriggerIssue::Empty));
    assert_eq!(
        validate_trigger("   ", &[], false),
        Err(TriggerIssue::WhitespaceOnly)
    );
    assert_eq!(
        validate_trigger("\t\n", &[], false),
        Err(TriggerIssue::WhitespaceOnly)
    );
    let existing = [";sig".to_string()];
    assert_eq!(
        validate_trigger(";SIG", &existing, false),
        Err(TriggerIssue::Duplicate)
    );
    assert_eq!(validate_trigger(";SIG", &existing, true), Ok(";SIG".into()));
    assert_eq!(
        validate_trigger(";sig", &existing, true),
        Err(TriggerIssue::Duplicate)
    );
}

#[test]
fn length_limit_is_the_key_buffer_capacity_in_clusters() {
    assert_eq!(MAX_TRIGGER_LENGTH, 64);
    let ok = "a".repeat(MAX_TRIGGER_LENGTH);
    assert_eq!(validate_trigger(&ok, &[], false), Ok(ok.clone()));
    assert_eq!(
        validate_trigger(&format!("{ok}a"), &[], false),
        Err(TriggerIssue::TooLong { length: 65 })
    );
    let family = "👨‍👩‍👧".repeat(MAX_TRIGGER_LENGTH);
    assert_eq!(validate_trigger(&family, &[], false), Ok(family.clone()));
}

#[test]
fn issue_messages_and_tokens() {
    for issue in [
        TriggerIssue::Empty,
        TriggerIssue::WhitespaceOnly,
        TriggerIssue::Duplicate,
        TriggerIssue::TooLong { length: 70 },
    ] {
        assert!(!issue.message().is_empty());
    }
    assert!(TriggerIssue::TooLong { length: 70 }
        .message()
        .contains("70"));
    assert_eq!(trigger_tokens(" a  b\tc\nd "), ["a", "b", "c", "d"]);
    assert!(trigger_tokens("   ").is_empty());
}
