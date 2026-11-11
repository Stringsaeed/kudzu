//! Why a fully typed trigger did not fire (`TriggerIndex::find_skipped`), for the activity log.

mod matching_support;

use kudzu_core::activity::RuleLevel;
use kudzu_core::matching::{
    ExpansionMatcher, KeyBuffer, KeyInput, MatchingConfig, SkipReason, SkippedMatch, TriggerIndex,
};
use matching_support::*;
use unicode_segmentation::UnicodeSegmentation;

/// Types `text` and reports what `find_skipped` says after the last keystroke.
fn skipped(text: &str, index: &TriggerIndex, bundle: Option<&str>) -> Option<SkippedMatch> {
    let mut buffer = KeyBuffer::new();
    let mut last = None;
    for cluster in text.graphemes(true) {
        let input = KeyInput::Character(cluster.to_string());
        buffer.apply(&input);
        assert!(
            index.find_match(&buffer, &input, bundle).is_none(),
            "nothing should fire"
        );
        last = index.find_skipped(&buffer, &input, bundle);
    }
    last
}

#[test]
fn a_snippet_that_is_off_is_reported_with_its_stored_trigger() {
    let s = snip(&[";Sig"]).disabled();
    let index = make_index(&[&s]);
    let found = skipped("hello ;sig", &index, None).expect("skipped");
    assert_eq!(found.snippet_id, s.id());
    // The stored trigger, never the text as typed.
    assert_eq!(found.trigger, ";Sig");
    assert_eq!(found.cause, SkipReason::SnippetOff);
}

#[test]
fn a_group_that_is_off_names_the_nearest_disabled_group() {
    let mut parent = group("parent");
    parent.is_enabled = false;
    let mut child = group("child");
    child.parent_id = Some(parent.id);
    let s = snip(&[";x"]).group(child.id);
    let index = make_index_with(&[&s], &[parent.clone(), child], MatchingConfig::default());
    assert_eq!(index.trigger_count(), 0);
    let found = skipped(";x", &index, None).expect("skipped");
    assert_eq!(
        found.cause,
        SkipReason::GroupOff {
            group_id: parent.id
        }
    );
}

#[test]
fn a_snippet_turned_off_wins_over_its_group() {
    let mut g = group("g");
    g.is_enabled = false;
    let s = snip(&[";x"]).group(g.id).disabled();
    let index = make_index_with(&[&s], &[g], MatchingConfig::default());
    assert_eq!(
        skipped(";x", &index, None).unwrap().cause,
        SkipReason::SnippetOff
    );
}

#[test]
fn a_snippet_rule_is_reported_at_snippet_level() {
    let s = snip(&[";x"]).rule(deny(&["com.tinyspeck.slackmacgap"]));
    let index = make_index(&[&s]);
    let found = skipped(";x", &index, Some("com.tinyspeck.slackmacgap")).expect("skipped");
    assert_eq!(
        found.cause,
        SkipReason::Rule {
            level: RuleLevel::Snippet,
            group_id: None
        }
    );
    // Elsewhere it fires, so there is nothing to explain.
    let mut buffer = KeyBuffer::new();
    for c in [";", "x"] {
        buffer.apply(&KeyInput::Character(c.into()));
    }
    assert!(index
        .find_match(
            &buffer,
            &KeyInput::Character("x".into()),
            Some("com.apple.mail")
        )
        .is_some());
}

#[test]
fn an_allow_list_blocks_unknown_apps() {
    let s = snip(&[";x"]).rule(allow(&["com.apple.mail"]));
    let index = make_index(&[&s]);
    assert!(matches!(
        skipped(";x", &index, None).unwrap().cause,
        SkipReason::Rule {
            level: RuleLevel::Snippet,
            ..
        }
    ));
}

#[test]
fn an_inherited_rule_names_the_group_that_set_it() {
    let mut parent = group("parent");
    parent.rule = Some(deny(&["com.apple.mail"]));
    let mut child = group("child");
    child.parent_id = Some(parent.id);
    let s = snip(&[";x"]).group(child.id);
    let index = make_index_with(&[&s], &[parent.clone(), child], MatchingConfig::default());
    let found = skipped(";x", &index, Some("com.apple.mail")).expect("skipped");
    assert_eq!(
        found.cause,
        SkipReason::Rule {
            level: RuleLevel::Group,
            group_id: Some(parent.id)
        }
    );
}

#[test]
fn built_in_exclusions_are_the_default_level() {
    let s = snip(&[";x"]);
    let index = make_index(&[&s]);
    let found = skipped(";x", &index, Some("com.1password.1password")).expect("skipped");
    assert_eq!(found.snippet_id, s.id());
    assert_eq!(
        found.cause,
        SkipReason::Rule {
            level: RuleLevel::Default,
            group_id: None
        }
    );
}

#[test]
fn apps_the_user_excluded_are_the_app_setting_level() {
    let mut config = MatchingConfig::default();
    config.denied_bundle_ids.insert("com.example.game".into());
    let index = make_index_with(&[&snip(&[";x"])], &[], config);
    let found = skipped(";x", &index, Some("com.example.game")).expect("skipped");
    assert_eq!(
        found.cause,
        SkipReason::Rule {
            level: RuleLevel::AppSetting,
            group_id: None
        }
    );
}

#[test]
fn near_misses_are_never_reported() {
    let index = make_index(&[
        &snip(&[";sig"]).disabled(),
        &snip(&["brb"]).whole_word().disabled(),
    ]);
    // A prefix, a longer word and a whole-word trigger without its separator.
    assert!(skipped(";si", &index, None).is_none());
    assert!(skipped(";sigx", &index, None).is_none());
    assert!(skipped("brb", &index, None).is_none());
    assert!(skipped("xbrb ", &index, None).is_none());
    // The whole word, completed by a separator, is a full match.
    assert_eq!(skipped("brb ", &index, None).unwrap().trigger, "brb");
}

#[test]
fn only_typed_characters_can_complete_a_trigger() {
    let index = make_index(&[&snip(&[";x"]).disabled()]);
    let mut buffer = KeyBuffer::new();
    for c in [";", "x"] {
        buffer.apply(&KeyInput::Character(c.into()));
    }
    assert!(index
        .find_skipped(&buffer, &KeyInput::Backspace, None)
        .is_none());
    assert!(index
        .find_skipped(&buffer, &KeyInput::Reset, None)
        .is_none());
}

#[test]
fn the_matcher_explains_a_miss_after_handling_the_key() {
    let s = snip(&[";x"]).disabled();
    let mut matcher = ExpansionMatcher::with_index(make_index(&[&s]));
    assert!(matcher
        .handle(KeyInput::Character(";".into()), None)
        .is_none());
    assert!(matcher.skipped(None).is_none());
    assert!(matcher
        .handle(KeyInput::Character("x".into()), None)
        .is_none());
    assert_eq!(matcher.skipped(None).unwrap().snippet_id, s.id());
}
