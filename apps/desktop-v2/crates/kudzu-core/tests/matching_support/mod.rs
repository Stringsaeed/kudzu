//! Shared helpers for the matching tests (ported from MatchingTestSupport.swift).
#![allow(dead_code)]

use chrono::{DateTime, TimeZone, Utc};
use kudzu_core::matching::{KeyBuffer, KeyInput, MatchingConfig, TriggerIndex, TriggerMatch};
use kudzu_core::{
    AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, SnippetRecord,
};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

/// Builder for a snippet with only the fields matching cares about.
#[derive(Clone)]
pub struct Snip(pub SnippetRecord);

pub fn snip(triggers: &[&str]) -> Snip {
    let epoch: DateTime<Utc> = Utc.timestamp_opt(0, 0).unwrap();
    Snip(SnippetRecord {
        formatting: None,
        id: Uuid::new_v4(),
        name: triggers.first().copied().unwrap_or("").to_string(),
        triggers: triggers.iter().map(|t| t.to_string()).collect(),
        content: "content".into(),
        group_id: None,
        fire_mode: FireMode::Immediate,
        case_mode: CaseMode::Propagate,
        insertion: None,
        rule: None,
        is_favorite: false,
        is_enabled: true,
        created_at: epoch,
        updated_at: epoch,
    })
}

impl Snip {
    pub fn whole_word(mut self) -> Self {
        self.0.fire_mode = FireMode::WholeWord;
        self
    }
    pub fn exact(mut self) -> Self {
        self.0.case_mode = CaseMode::Exact;
        self
    }
    pub fn group(mut self, id: Uuid) -> Self {
        self.0.group_id = Some(id);
        self
    }
    pub fn insertion(mut self, method: InsertionMethod) -> Self {
        self.0.insertion = Some(method);
        self
    }
    pub fn rule(mut self, rule: AppRule) -> Self {
        self.0.rule = Some(rule);
        self
    }
    pub fn disabled(mut self) -> Self {
        self.0.is_enabled = false;
        self
    }
    pub fn updated(mut self, seconds: i64) -> Self {
        self.0.updated_at = Utc.timestamp_opt(seconds, 0).unwrap();
        self
    }
    pub fn id(&self) -> Uuid {
        self.0.id
    }
    pub fn record(&self) -> SnippetRecord {
        self.0.clone()
    }
}

pub fn group(name: &str) -> GroupRecord {
    GroupRecord {
        id: Uuid::new_v4(),
        name: name.into(),
        parent_id: None,
        sort_index: 0,
        is_enabled: true,
        insertion: None,
        rule: None,
    }
}

pub fn allow(ids: &[&str]) -> AppRule {
    AppRule {
        mode: AppRuleMode::Allow,
        bundle_ids: ids.iter().map(|s| s.to_string()).collect(),
        insertion: None,
    }
}

pub fn deny(ids: &[&str]) -> AppRule {
    AppRule {
        mode: AppRuleMode::Deny,
        bundle_ids: ids.iter().map(|s| s.to_string()).collect(),
        insertion: None,
    }
}

pub fn records(snippets: &[&Snip]) -> Vec<SnippetRecord> {
    snippets.iter().map(|s| s.record()).collect()
}

pub fn make_index(snippets: &[&Snip]) -> TriggerIndex {
    TriggerIndex::new(&records(snippets), &[], MatchingConfig::default())
}

pub fn make_index_with(
    snippets: &[&Snip],
    groups: &[GroupRecord],
    config: MatchingConfig,
) -> TriggerIndex {
    TriggerIndex::new(&records(snippets), groups, config)
}

/// Types `text` one grapheme cluster at a time and returns the match produced by each keystroke.
pub fn type_all(
    text: &str,
    index: &TriggerIndex,
    bundle_id: Option<&str>,
) -> Vec<Option<TriggerMatch>> {
    let mut buffer = KeyBuffer::new();
    text.graphemes(true)
        .map(|cluster| {
            let input = KeyInput::Character(cluster.to_string());
            buffer.apply(&input);
            index.find_match(&buffer, &input, bundle_id)
        })
        .collect()
}

/// The match produced by the final keystroke.
pub fn type_in(text: &str, index: &TriggerIndex, bundle_id: Option<&str>) -> Option<TriggerMatch> {
    type_all(text, index, bundle_id).pop().flatten()
}

/// Shorthand for typing with no bundle ID.
pub fn typed(text: &str, index: &TriggerIndex) -> Option<TriggerMatch> {
    type_in(text, index, None)
}

/// The first match produced by any keystroke.
pub fn first_match(text: &str, index: &TriggerIndex) -> Option<TriggerMatch> {
    type_all(text, index, None).into_iter().flatten().next()
}
