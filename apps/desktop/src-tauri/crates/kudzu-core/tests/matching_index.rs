//! Trigger firing, ranking, enablement, app rules and insertion (ported from TriggerIndexTests.swift).

mod matching_support;

use std::time::Instant;

use kudzu_core::matching::{CaseStyle, KeyBuffer, KeyInput, MatchingConfig, TriggerIndex};
use kudzu_core::{FireMode, InsertionMethod};
use matching_support::*;
use uuid::Uuid;

fn ch(s: &str) -> KeyInput {
    KeyInput::Character(s.into())
}

// Immediate

#[test]
fn immediate_fires_on_last_trigger_character() {
    let s = snip(&[";sig"]);
    let index = make_index(&[&s]);
    let mut results = type_all(";sig", &index, None);
    let m = results.pop().flatten().expect("match");
    assert!(results.iter().all(Option::is_none));
    assert_eq!(m.snippet_id, s.id());
    assert_eq!(m.trigger, ";sig");
    assert_eq!(m.typed_trigger, ";sig");
    assert_eq!(m.delete_count, 4);
    assert_eq!(m.separator, None);
    assert_eq!(m.fire_mode, FireMode::Immediate);
}

#[test]
fn immediate_fires_mid_word_after_other_text() {
    let index = make_index(&[&snip(&[";sig"])]);
    assert_eq!(typed("hello;sig", &index).unwrap().delete_count, 4);
}

#[test]
fn trigger_at_very_start_of_buffer() {
    let index = make_index(&[&snip(&["ab"])]);
    assert_eq!(typed("ab", &index).unwrap().typed_trigger, "ab");
}

#[test]
fn immediate_does_not_fire_on_partial_or_later_keystroke() {
    let index = make_index(&[&snip(&[";sig"])]);
    assert!(typed(";si", &index).is_none());
    assert!(typed("sig", &index).is_none());
    assert!(typed(";sigx", &index).is_none());
}

#[test]
fn multiple_triggers_per_snippet_all_work() {
    let s = snip(&[";sig", ";regards", "brb"]);
    let index = make_index(&[&s]);
    for trigger in [";sig", ";regards", "brb"] {
        let m = typed(trigger, &index).unwrap();
        assert_eq!(m.snippet_id, s.id());
        assert_eq!(m.trigger, trigger);
    }
}

#[test]
fn empty_and_missing_triggers_are_skipped() {
    let index = make_index(&[&snip(&[]), &snip(&[""]), &snip(&["", ";ok"])]);
    assert_eq!(index.trigger_count(), 1);
    assert!(typed(";ok", &index).is_some());
}

#[test]
fn duplicate_triggers_in_one_snippet_are_indexed_once() {
    assert_eq!(make_index(&[&snip(&[";a", ";A"])]).trigger_count(), 1);
    assert_eq!(
        make_index(&[&snip(&[";a", ";A"]).exact()]).trigger_count(),
        2
    );
}

#[test]
fn backspace_in_middle_of_typing_trigger() {
    let index = make_index(&[&snip(&[";sig"])]);
    let mut buffer = KeyBuffer::new();
    let mut press = |input: KeyInput| {
        buffer.apply(&input);
        index.find_match(&buffer, &input, None)
    };
    assert!(press(ch(";")).is_none());
    assert!(press(ch("s")).is_none());
    assert!(press(ch("x")).is_none());
    assert!(press(KeyInput::Backspace).is_none());
    assert!(press(ch("i")).is_none());
    assert!(press(ch("g")).is_some());
}

#[test]
fn backspace_after_completion_does_not_refire() {
    let index = make_index(&[&snip(&[";sig"])]);
    let mut buffer = KeyBuffer::new();
    for c in [";", "s", "i", "g"] {
        buffer.apply(&ch(c));
    }
    buffer.apply(&KeyInput::Backspace);
    assert!(index
        .find_match(&buffer, &KeyInput::Backspace, None)
        .is_none());
    buffer.apply(&ch("g"));
    assert!(index.find_match(&buffer, &ch("g"), None).is_some());
}

#[test]
fn reset_mid_trigger_prevents_match() {
    let index = make_index(&[&snip(&[";sig"])]);
    let mut buffer = KeyBuffer::new();
    for c in [";", "s"] {
        buffer.apply(&ch(c));
    }
    buffer.apply(&KeyInput::Reset);
    for c in ["i", "g"] {
        buffer.apply(&ch(c));
    }
    assert!(index.find_match(&buffer, &ch("g"), None).is_none());
}

#[test]
fn non_character_last_input_and_empty_buffer_never_match() {
    let index = make_index(&[&snip(&[";sig"])]);
    let mut buffer = KeyBuffer::new();
    for c in [";", "s", "i", "g"] {
        buffer.apply(&ch(c));
    }
    assert!(index.find_match(&buffer, &KeyInput::Reset, None).is_none());
    assert!(index
        .find_match(&buffer, &KeyInput::Backspace, None)
        .is_none());
    assert!(index
        .find_match(&KeyBuffer::new(), &ch("x"), None)
        .is_none());
}

#[test]
fn triggers_containing_spaces() {
    let index = make_index(&[&snip(&["my address"])]);
    assert_eq!(typed("my address", &index).unwrap().delete_count, 10);
    assert!(typed("my  address", &index).is_none());
}

#[test]
fn emoji_triggers() {
    let index = make_index(&[&snip(&["👍🏽", ":)"]), &snip(&["🇺🇸"])]);
    let thumbs = typed("ok 👍🏽", &index).unwrap();
    assert_eq!(thumbs.delete_count, 1);
    assert_eq!(thumbs.trigger, "👍🏽");
    assert_eq!(typed("🇺🇸", &index).unwrap().delete_count, 1);
    assert!(typed("👍🏿", &index).is_none());
}

#[test]
fn multi_scalar_input_characters_match_canonically() {
    let index = make_index(&[&snip(&["caf\u{E9}"])]);
    let mut buffer = KeyBuffer::new();
    for piece in ["c", "a", "f", "e\u{301}"] {
        buffer.apply(&ch(piece));
    }
    let m = index
        .find_match(&buffer, &ch("e\u{301}"), None)
        .expect("canonically equivalent match");
    assert_eq!(m.delete_count, 4);
    // And the other way round, for an exact snippet.
    let decomposed = make_index(&[&snip(&["cafe\u{301}"]).exact()]);
    assert!(typed("caf\u{E9}", &decomposed).is_some());
}

// Whole word

#[test]
fn whole_word_fires_on_separator_and_keeps_it_out() {
    let s = snip(&["brb"]).whole_word();
    let index = make_index(&[&s]);
    let mut results = type_all("brb ", &index, None);
    let m = results.pop().flatten().unwrap();
    assert!(results.iter().all(Option::is_none));
    assert_eq!(m.snippet_id, s.id());
    assert_eq!(m.delete_count, 3);
    assert_eq!(m.separator.as_deref(), Some(" "));
    assert_eq!(m.delete_count_including_separator(), 4);
    assert_eq!(m.typed_trigger, "brb");
    assert_eq!(m.fire_mode, FireMode::WholeWord);
}

#[test]
fn whole_word_with_enter_separator() {
    let index = make_index(&[&snip(&["brb"]).whole_word()]);
    let m = typed("brb\n", &index).unwrap();
    assert_eq!(m.separator.as_deref(), Some("\n"));
    assert_eq!(m.delete_count, 3);
    assert_eq!(
        typed("brb\r\n", &index).unwrap().separator.as_deref(),
        Some("\r\n")
    );
}

#[test]
fn whole_word_default_separators() {
    let index = make_index(&[&snip(&["brb"]).whole_word()]);
    for separator in [
        " ", "\t", "\n", ".", ",", "!", "?", ";", ":", ")", "]", "}", "'", "\"",
    ] {
        let m =
            typed(&format!("brb{separator}"), &index).unwrap_or_else(|| panic!("{separator:?}"));
        assert_eq!(m.separator.as_deref(), Some(separator));
        assert_eq!(m.delete_count, 3);
    }
}

#[test]
fn whole_word_ignores_non_separators() {
    let index = make_index(&[&snip(&["brb"]).whole_word()]);
    for c in ["-", "_", "a", "1", "/", "("] {
        assert!(typed(&format!("brb{c}"), &index).is_none(), "{c}");
    }
    assert!(type_all("brb", &index, None).iter().all(Option::is_none));
}

#[test]
fn whole_word_requires_boundary_before_trigger() {
    let index = make_index(&[&snip(&["brb"]).whole_word()]);
    assert!(typed("abrb ", &index).is_none());
    assert!(typed("1brb ", &index).is_none());
    assert_eq!(typed("so brb ", &index).unwrap().delete_count, 3);
    assert!(typed("so,brb ", &index).is_some());
    assert!(typed("(brb ", &index).is_none());
    assert!(typed("\nbrb ", &index).is_some());
    assert!(typed("brb ", &index).is_some());
}

#[test]
fn whole_word_trigger_starting_with_separator_character() {
    let index = make_index(&[&snip(&[";sig"]).whole_word()]);
    let m = typed(";sig ", &index).unwrap();
    assert_eq!(m.delete_count, 4);
    assert_eq!(m.separator.as_deref(), Some(" "));
    assert!(typed("hello ;sig ", &index).is_some());
    assert!(typed("hello;sig ", &index).is_none());
}

#[test]
fn whole_word_trigger_ending_with_separator_needs_another_separator() {
    let index = make_index(&[&snip(&["brb."]).whole_word()]);
    assert!(typed("brb.", &index).is_none());
    let m = typed("brb. ", &index).unwrap();
    assert_eq!(m.typed_trigger, "brb.");
    assert_eq!(m.delete_count, 4);
}

#[test]
fn whole_word_trigger_with_spaces() {
    let index = make_index(&[&snip(&["my address"]).whole_word()]);
    let m = typed("hi my address!", &index).unwrap();
    assert_eq!(m.delete_count, 10);
    assert_eq!(m.separator.as_deref(), Some("!"));
}

#[test]
fn overlapping_whole_word_triggers_both_fire() {
    let (a, ab) = (snip(&[";a"]).whole_word(), snip(&[";ab"]).whole_word());
    let index = make_index(&[&a, &ab]);
    assert_eq!(typed(";a ", &index).unwrap().snippet_id, a.id());
    assert_eq!(typed(";ab ", &index).unwrap().snippet_id, ab.id());
    assert!(type_all(";ab", &index, None).iter().all(Option::is_none));
}

#[test]
fn immediate_shadows_longer_trigger() {
    let (a, ab) = (snip(&[";a"]), snip(&[";ab"]));
    assert_eq!(
        first_match(";ab", &make_index(&[&a, &ab]))
            .unwrap()
            .snippet_id,
        a.id()
    );
}

#[test]
fn immediate_and_whole_word_coexist() {
    let (imm, ww) = (snip(&[";i"]), snip(&["btw"]).whole_word());
    let index = make_index(&[&imm, &ww]);
    assert_eq!(typed(";i", &index).unwrap().snippet_id, imm.id());
    assert_eq!(typed("btw ", &index).unwrap().snippet_id, ww.id());
    assert!(typed("btw", &index).is_none());
}

// Case

#[test]
fn propagate_matches_case_insensitively_and_reports_style() {
    let index = make_index(&[&snip(&[";sig"])]);
    for t in [";sig", ";Sig", ";SIG", ";sIg"] {
        let m = typed(t, &index).unwrap_or_else(|| panic!("{t} should match"));
        assert_eq!(m.trigger, ";sig");
        assert_eq!(m.typed_trigger, t);
    }
    assert_eq!(typed(";sig", &index).unwrap().case_style, CaseStyle::AsIs);
    assert_eq!(
        typed(";Sig", &index).unwrap().case_style,
        CaseStyle::Capitalized
    );
    assert_eq!(typed(";SIG", &index).unwrap().case_style, CaseStyle::Upper);
}

#[test]
fn exact_is_case_sensitive_and_never_propagates() {
    let index = make_index(&[&snip(&[";Sig"]).exact()]);
    assert_eq!(typed(";Sig", &index).unwrap().case_style, CaseStyle::AsIs);
    assert!(typed(";sig", &index).is_none());
    assert!(typed(";SIG", &index).is_none());
}

#[test]
fn propagate_with_unicode_letters() {
    let index = make_index(&[&snip(&[";éclair"])]);
    assert_eq!(
        typed(";ÉCLAIR", &index).unwrap().case_style,
        CaseStyle::Upper
    );
    assert_eq!(
        typed(";Éclair", &index).unwrap().case_style,
        CaseStyle::Capitalized
    );
}

#[test]
fn whole_word_propagate() {
    let index = make_index(&[&snip(&["brb"]).whole_word()]);
    let m = typed("BRB ", &index).unwrap();
    assert_eq!(m.case_style, CaseStyle::Upper);
    assert_eq!(m.typed_trigger, "BRB");
}

#[test]
fn exact_and_propagate_coexist_on_same_last_character() {
    let (exact, prop) = (snip(&["Ab"]).exact(), snip(&["xb"]));
    let index = make_index(&[&exact, &prop]);
    assert_eq!(typed("Ab", &index).unwrap().snippet_id, exact.id());
    assert_eq!(typed("XB", &index).unwrap().snippet_id, prop.id());
    assert!(typed("ab", &index).is_none());
}

// Ranking

#[test]
fn longest_trigger_wins() {
    let (tail, full) = (snip(&["ail"]), snip(&["gmail"]));
    let m = typed("gmail", &make_index(&[&tail, &full])).unwrap();
    assert_eq!(m.snippet_id, full.id());
    assert_eq!(m.delete_count, 5);
    assert_eq!(
        typed("gmail", &make_index(&[&full, &tail]))
            .unwrap()
            .snippet_id,
        full.id()
    );
}

#[test]
fn tie_breaks_on_most_recently_updated() {
    let (old, new) = (snip(&[";x"]).updated(100), snip(&[";x"]).updated(200));
    assert_eq!(
        typed(";x", &make_index(&[&old, &new])).unwrap().snippet_id,
        new.id()
    );
    assert_eq!(
        typed(";x", &make_index(&[&new, &old])).unwrap().snippet_id,
        new.id()
    );
    let (a, b) = (snip(&[";x"]).updated(5), snip(&[";x"]).updated(5));
    assert_eq!(
        typed(";x", &make_index(&[&a, &b])).unwrap().snippet_id,
        typed(";x", &make_index(&[&a, &b])).unwrap().snippet_id
    );
    let (old, new) = (snip(&[";X"]).updated(1), snip(&[";x"]).updated(2));
    assert_eq!(
        typed(";x", &make_index(&[&old, &new])).unwrap().snippet_id,
        new.id()
    );
}

#[test]
fn longer_whole_word_beats_shorter_immediate_ending_same_buffer() {
    let (imm, ww) = (snip(&["o "]), snip(&["hello"]).whole_word());
    assert_eq!(
        typed("hello ", &make_index(&[&imm, &ww]))
            .unwrap()
            .snippet_id,
        ww.id()
    );
}

// Enablement

#[test]
fn disabled_snippets_and_groups_never_fire() {
    let index = make_index(&[&snip(&[";x"]).disabled()]);
    assert_eq!(index.trigger_count(), 0);
    assert!(typed(";x", &index).is_none());

    let mut g = group("g");
    g.is_enabled = false;
    let index = make_index_with(
        &[&snip(&[";x"]).group(g.id)],
        &[g],
        MatchingConfig::default(),
    );
    assert!(typed(";x", &index).is_none());

    let mut top = group("top");
    top.is_enabled = false;
    let mut mid = group("mid");
    mid.parent_id = Some(top.id);
    let mut leaf = group("leaf");
    leaf.parent_id = Some(mid.id);
    let index = make_index_with(
        &[&snip(&[";x"]).group(leaf.id)],
        &[leaf, mid, top],
        MatchingConfig::default(),
    );
    assert!(typed(";x", &index).is_none());
}

#[test]
fn enabled_chain_and_sibling_groups() {
    let top = group("top");
    let mut leaf = group("leaf");
    leaf.parent_id = Some(top.id);
    let index = make_index_with(
        &[&snip(&[";x"]).group(leaf.id)],
        &[leaf, top],
        MatchingConfig::default(),
    );
    assert!(typed(";x", &index).is_some());

    let on = group("on");
    let mut off = group("off");
    off.is_enabled = false;
    let index = make_index_with(
        &[&snip(&[";a"]).group(on.id), &snip(&[";b"]).group(off.id)],
        &[on, off],
        MatchingConfig::default(),
    );
    assert!(typed(";a", &index).is_some());
    assert!(typed(";b", &index).is_none());
}

#[test]
fn group_cycles_do_not_hang_and_still_honour_enablement() {
    let (a_id, b_id) = (Uuid::new_v4(), Uuid::new_v4());
    let mut a = group("a");
    a.id = a_id;
    a.parent_id = Some(b_id);
    let mut b = group("b");
    b.id = b_id;
    b.parent_id = Some(a_id);
    let s = snip(&[";x"]).group(a_id);
    assert!(typed(
        ";x",
        &make_index_with(&[&s], &[a.clone(), b.clone()], MatchingConfig::default())
    )
    .is_some());
    b.is_enabled = false;
    assert!(typed(
        ";x",
        &make_index_with(&[&s], &[a, b], MatchingConfig::default())
    )
    .is_none());

    let mut own = group("self");
    own.parent_id = Some(own.id);
    let s = snip(&[";x"]).group(own.id);
    assert!(typed(
        ";x",
        &make_index_with(&[&s], &[own], MatchingConfig::default())
    )
    .is_some());

    assert!(typed(";x", &make_index(&[&snip(&[";x"]).group(Uuid::new_v4())])).is_some());
}

// App rules

#[test]
fn allow_and_deny_rules() {
    let index = make_index(&[&snip(&[";x"]).rule(allow(&["com.apple.Notes"]))]);
    assert!(type_in(";x", &index, Some("com.apple.Notes")).is_some());
    assert!(type_in(";x", &index, Some("com.apple.Safari")).is_none());
    assert!(type_in(";x", &index, None).is_none());

    let index = make_index(&[&snip(&[";x"]).rule(deny(&["com.apple.Terminal"]))]);
    assert!(type_in(";x", &index, Some("com.apple.Terminal")).is_none());
    assert!(type_in(";x", &index, Some("com.apple.Notes")).is_some());
    assert!(type_in(";x", &index, None).is_some());
}

#[test]
fn snippet_rule_overrides_group_rule() {
    let mut g = group("g");
    g.rule = Some(allow(&["com.a"]));
    let own = snip(&[";own"]).group(g.id).rule(allow(&["com.b"]));
    let inherited = snip(&[";inh"]).group(g.id);
    let index = make_index_with(&[&own, &inherited], &[g], MatchingConfig::default());
    assert!(type_in(";own", &index, Some("com.b")).is_some());
    assert!(type_in(";own", &index, Some("com.a")).is_none());
    assert!(type_in(";inh", &index, Some("com.a")).is_some());
    assert!(type_in(";inh", &index, Some("com.b")).is_none());
}

#[test]
fn nearest_ancestor_group_rule_wins_and_is_inherited() {
    let mut top = group("top");
    top.rule = Some(allow(&["com.top"]));
    let mut mid = group("mid");
    mid.parent_id = Some(top.id);
    mid.rule = Some(allow(&["com.mid"]));
    let mut leaf = group("leaf");
    leaf.parent_id = Some(mid.id);
    let s = snip(&[";x"]).group(leaf.id);
    let index = make_index_with(&[&s], &[top, mid, leaf], MatchingConfig::default());
    assert!(type_in(";x", &index, Some("com.mid")).is_some());
    assert!(type_in(";x", &index, Some("com.top")).is_none());

    let mut top = group("top");
    top.rule = Some(deny(&["com.bad"]));
    let mut leaf = group("leaf");
    leaf.parent_id = Some(top.id);
    let s = snip(&[";x"]).group(leaf.id);
    let index = make_index_with(&[&s], &[top, leaf], MatchingConfig::default());
    assert!(type_in(";x", &index, Some("com.bad")).is_none());
    assert!(type_in(";x", &index, Some("com.ok")).is_some());
}

#[test]
fn global_denylist_always_applies() {
    let config = MatchingConfig {
        denied_bundle_ids: ["com.1password.1password".to_string()].into(),
        ..MatchingConfig::default()
    };
    let index = make_index_with(
        &[
            &snip(&[";x"]).rule(allow(&["com.1password.1password"])),
            &snip(&[";y"]),
        ],
        &[],
        config,
    );
    assert!(type_in(";x", &index, Some("com.1password.1password")).is_none());
    assert!(type_in(";y", &index, Some("com.1password.1password")).is_none());
    assert!(type_in(";y", &index, Some("com.apple.Notes")).is_some());

    let index = make_index(&[&snip(&[";y"])]);
    for id in [
        "com.1password.1password",
        "com.bitwarden.desktop",
        "com.apple.keychainaccess",
        "com.apple.loginwindow",
    ] {
        assert!(type_in(";y", &index, Some(id)).is_none(), "{id}");
    }
}

#[test]
fn rule_excluded_snippet_does_not_block_other_candidate() {
    let blocked = snip(&["xyz"]).rule(allow(&["com.only"]));
    let ok = snip(&["yz"]);
    let index = make_index(&[&blocked, &ok]);
    assert_eq!(
        type_in("xyz", &index, Some("com.other"))
            .unwrap()
            .snippet_id,
        ok.id()
    );
}

// Insertion

#[test]
fn insertion_precedence_snippet_then_group_then_config() {
    let config = MatchingConfig {
        default_insertion: InsertionMethod::Paste,
        ..MatchingConfig::default()
    };
    assert_eq!(
        typed(
            ";x",
            &make_index_with(&[&snip(&[";x"])], &[], config.clone())
        )
        .unwrap()
        .insertion,
        InsertionMethod::Paste
    );
    let mut top = group("top");
    top.insertion = Some(InsertionMethod::Type);
    let mut leaf = group("leaf");
    leaf.parent_id = Some(top.id);
    let index = make_index_with(
        &[
            &snip(&[";own"])
                .group(leaf.id)
                .insertion(InsertionMethod::Automatic),
            &snip(&[";grp"]).group(leaf.id),
            &snip(&[";cfg"]),
        ],
        &[top, leaf],
        config,
    );
    assert_eq!(
        typed(";own", &index).unwrap().insertion,
        InsertionMethod::Automatic
    );
    assert_eq!(
        typed(";grp", &index).unwrap().insertion,
        InsertionMethod::Type
    );
    assert_eq!(
        typed(";cfg", &index).unwrap().insertion,
        InsertionMethod::Paste
    );
}

#[test]
fn nearest_group_insertion_wins() {
    let mut top = group("top");
    top.insertion = Some(InsertionMethod::Type);
    let mut leaf = group("leaf");
    leaf.parent_id = Some(top.id);
    leaf.insertion = Some(InsertionMethod::Paste);
    let index = make_index_with(
        &[&snip(&[";x"]).group(leaf.id)],
        &[top, leaf],
        MatchingConfig::default(),
    );
    assert_eq!(
        typed(";x", &index).unwrap().insertion,
        InsertionMethod::Paste
    );
}

#[test]
fn rule_insertion_override() {
    let mut rule = allow(&["com.term", "com.notes"]);
    rule.insertion = Some(InsertionMethod::Paste);
    let index = make_index(&[&snip(&[";x"])
        .insertion(InsertionMethod::Type)
        .rule(rule.clone())]);
    assert_eq!(
        type_in(";x", &index, Some("com.term")).unwrap().insertion,
        InsertionMethod::Paste
    );
    assert_eq!(
        type_in(";x", &index, Some("com.notes")).unwrap().insertion,
        InsertionMethod::Paste
    );

    let mut g = group("g");
    g.rule = Some(rule);
    let index = make_index_with(
        &[&snip(&[";x"]).group(g.id)],
        &[g],
        MatchingConfig::default(),
    );
    assert_eq!(
        type_in(";x", &index, Some("com.term")).unwrap().insertion,
        InsertionMethod::Paste
    );

    let mut deny_rule = deny(&["com.term"]);
    deny_rule.insertion = Some(InsertionMethod::Paste);
    let index = make_index(&[&snip(&[";x"])
        .insertion(InsertionMethod::Type)
        .rule(deny_rule)]);
    assert_eq!(
        type_in(";x", &index, Some("com.notes")).unwrap().insertion,
        InsertionMethod::Type
    );

    let index = make_index(&[&snip(&[";x"])
        .insertion(InsertionMethod::Type)
        .rule(allow(&["com.term"]))]);
    assert_eq!(
        type_in(";x", &index, Some("com.term")).unwrap().insertion,
        InsertionMethod::Type
    );
}

#[test]
fn default_config_uses_paste_for_default_terminals() {
    let index = make_index(&[&snip(&[";x"])]);
    for id in MatchingConfig::default_app_insertions().keys() {
        assert_eq!(
            type_in(";x", &index, Some(id)).unwrap().insertion,
            InsertionMethod::Paste,
            "{id}"
        );
    }
    assert_eq!(
        type_in(";x", &index, Some("com.apple.Notes"))
            .unwrap()
            .insertion,
        InsertionMethod::Automatic
    );
}

#[test]
fn full_insertion_precedence_order() {
    let (app, other) = ("com.target.app", "com.other.app");
    let config = MatchingConfig {
        default_insertion: InsertionMethod::Automatic,
        app_insertions: [(app.to_string(), InsertionMethod::Paste)].into(),
        ..MatchingConfig::default()
    };
    let mut rule = allow(&[app]);
    rule.insertion = Some(InsertionMethod::Type);
    let mut g1 = group("g1");
    g1.insertion = Some(InsertionMethod::Automatic);
    let s1 = snip(&[";1"])
        .group(g1.id)
        .insertion(InsertionMethod::Automatic)
        .rule(rule);
    let mut g2 = group("g2");
    g2.insertion = Some(InsertionMethod::Automatic);
    let s2 = snip(&[";2"]).group(g2.id).insertion(InsertionMethod::Type);
    let mut g3 = group("g3");
    g3.insertion = Some(InsertionMethod::Type);
    let s3 = snip(&[";3"]).group(g3.id);
    let s4 = snip(&[";4"]);
    let index = make_index_with(&[&s1, &s2, &s3, &s4], &[g1, g2, g3], config);
    assert_eq!(
        type_in(";1", &index, Some(app)).unwrap().insertion,
        InsertionMethod::Type
    );
    assert_eq!(
        type_in(";2", &index, Some(app)).unwrap().insertion,
        InsertionMethod::Type
    );
    assert_eq!(
        type_in(";3", &index, Some(app)).unwrap().insertion,
        InsertionMethod::Type
    );
    assert_eq!(
        type_in(";4", &index, Some(app)).unwrap().insertion,
        InsertionMethod::Paste
    );
    assert_eq!(
        type_in(";4", &index, Some(other)).unwrap().insertion,
        InsertionMethod::Automatic
    );
}

// Performance

#[test]
fn matching_stays_fast_with_five_thousand_snippets() {
    let snippets: Vec<Snip> = (0..5_000)
        .map(|i| {
            let s = snip(&[&format!(";s{i}"), &format!("tag{i}x")]);
            if i % 2 == 0 {
                s
            } else {
                s.whole_word()
            }
        })
        .collect();
    let refs: Vec<&Snip> = snippets.iter().collect();
    let index: TriggerIndex = make_index(&refs);
    assert_eq!(index.trigger_count(), 10_000);

    let text = "The quick brown fox jumps over the lazy dog ;s42 and ;s4999 tag12x 0123456789 ";
    let mut buffer = KeyBuffer::new();
    let mut matches = 0;
    let rounds = 40;
    let start = Instant::now();
    for _ in 0..rounds {
        for c in text.chars() {
            let input = KeyInput::Character(c.to_string());
            buffer.apply(&input);
            if index
                .find_match(&buffer, &input, Some("com.apple.Notes"))
                .is_some()
            {
                matches += 1;
                buffer.apply(&KeyInput::Reset);
            }
        }
    }
    let per_keystroke = start.elapsed() / (rounds * text.chars().count()) as u32;
    assert!(matches > 0);
    assert!(
        per_keystroke.as_micros() < 1000,
        "avg {per_keystroke:?} per keystroke"
    );
}

#[test]
fn building_large_index_is_reasonable() {
    let snippets: Vec<Snip> = (0..5_000).map(|i| snip(&[&format!(";s{i}")])).collect();
    let refs: Vec<&Snip> = snippets.iter().collect();
    let start = Instant::now();
    let _ = make_index(&refs);
    assert!(start.elapsed().as_secs() < 5);
}
