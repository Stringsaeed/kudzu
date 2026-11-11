//! Reversed-trigger suffix tries: a keystroke costs O(longest trigger), independent of library size.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use super::config::resolve_insertion;
use super::key_buffer::{folded_key, folded_string, nfc, KeyBuffer};
use super::{
    case_propagation, CaseStyle, KeyInput, MatchingConfig, SkipReason, SkippedMatch, TriggerMatch,
};
use crate::activity::RuleLevel;
use crate::library::groups::{nearest_disabled_group, EffectiveSnippet, GroupResolver};
use crate::library::model::{
    AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, SnippetRecord,
};

/// The immutable data behind a `TriggerIndex`.
#[derive(Debug, Default)]
pub(crate) struct IndexData {
    pub config: MatchingConfig,
    snippets: Vec<IndexedSnippet>,
    entries: Vec<Entry>,
    /// Entries whose snippet is enabled (the rest exist only to explain a skip).
    live_entries: usize,
    /// Case-insensitive triggers, keyed by folded clusters.
    folded: SuffixTrie,
    /// Case-sensitive triggers, keyed by clusters as written.
    exact: SuffixTrie,
}

#[derive(Debug)]
struct Entry {
    snippet: u32,
    trigger: String,
    /// Grapheme clusters.
    length: usize,
    fire_mode: FireMode,
}

#[derive(Debug)]
struct IndexedRule {
    mode: AppRuleMode,
    bundle_ids: HashSet<String>,
    insertion: Option<InsertionMethod>,
}

impl IndexedRule {
    fn new(rule: &AppRule) -> Self {
        IndexedRule {
            mode: rule.mode,
            bundle_ids: rule.bundle_ids.iter().cloned().collect(),
            insertion: rule.insertion,
        }
    }

    /// An unknown app (`None`) cannot satisfy an allow-list and is not caught by a deny-list.
    fn permits(&self, bundle_id: Option<&str>) -> bool {
        match (self.mode, bundle_id) {
            (AppRuleMode::Allow, Some(id)) => self.bundle_ids.contains(id),
            (AppRuleMode::Allow, None) => false,
            (AppRuleMode::Deny, Some(id)) => !self.bundle_ids.contains(id),
            (AppRuleMode::Deny, None) => true,
        }
    }
}

#[derive(Debug)]
struct IndexedSnippet {
    id: Uuid,
    updated_at: DateTime<Utc>,
    case_mode: CaseMode,
    rule: Option<IndexedRule>,
    insertion: Option<InsertionMethod>,
    /// Why the snippet can never fire; `None` when it is effectively enabled.
    off: Option<SkipReason>,
    /// The group `rule` was inherited from; `None` for the snippet's own rule.
    rule_group: Option<Uuid>,
}

/// Which candidates a trie walk accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    /// Enabled snippets whose rule permits the app: what may fire.
    Fire,
    /// Every full match, to explain why none fired.
    Explain,
}

impl IndexedSnippet {
    fn insertion_for(&self, bundle_id: Option<&str>, config: &MatchingConfig) -> InsertionMethod {
        let rule_override = self.rule.as_ref().and_then(|rule| {
            let id = bundle_id?;
            rule.bundle_ids
                .contains(id)
                .then_some(rule.insertion)
                .flatten()
        });
        resolve_insertion(rule_override, self.insertion, bundle_id, config)
    }
}

/// Trie over reversed keys: the path from the root spells a trigger from its last cluster backwards.
/// Clusters are interned to `u32` symbols so one edge map serves the whole trie.
#[derive(Debug)]
struct SuffixTrie {
    symbols: HashMap<String, u32>,
    edges: HashMap<(u32, u32), u32>,
    /// Entry indices per node; node 0 is the root.
    entries: Vec<Vec<u32>>,
}

impl Default for SuffixTrie {
    fn default() -> Self {
        SuffixTrie {
            symbols: HashMap::new(),
            edges: HashMap::new(),
            entries: vec![Vec::new()],
        }
    }
}

impl SuffixTrie {
    fn insert(&mut self, key: &[String], entry: u32) {
        let mut current = 0u32;
        for cluster in key.iter().rev() {
            let next_symbol = self.symbols.len() as u32;
            let symbol = *self.symbols.entry(cluster.clone()).or_insert(next_symbol);
            let next_node = self.entries.len() as u32;
            current = match self.edges.get(&(current, symbol)) {
                Some(&node) => node,
                None => {
                    self.edges.insert((current, symbol), next_node);
                    self.entries.push(Vec::new());
                    next_node
                }
            };
        }
        self.entries[current as usize].push(entry);
    }

    fn child(&self, node: u32, cluster: &str) -> Option<u32> {
        let symbol = self.symbols.get(cluster)?;
        self.edges.get(&(node, *symbol)).copied()
    }

    fn entries(&self, node: u32) -> &[u32] {
        &self.entries[node as usize]
    }
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    entry: u32,
    start: usize,
    end: usize,
}

/// Whether `cluster` ends a word: whitespace, a newline, or a configured separator.
pub(crate) fn is_separator(config: &MatchingConfig, cluster: &str) -> bool {
    let mut chars = cluster.chars();
    match (chars.next(), chars.next()) {
        (Some(first), None) => config.is_separator(first),
        // A multi-scalar cluster (e.g. CRLF) is judged on its first scalar, like Swift's `Character.isWhitespace`.
        (Some(first), Some(_)) => first.is_whitespace(),
        (None, _) => false,
    }
}

impl IndexData {
    pub fn new(snippets: &[SnippetRecord], groups: &[GroupRecord], config: MatchingConfig) -> Self {
        let resolver = GroupResolver::new(groups);
        let mut data = IndexData {
            config,
            ..IndexData::default()
        };
        for snippet in snippets {
            let effective = EffectiveSnippet::new(snippet, &resolver);
            let off = if effective.is_enabled {
                None
            } else if !snippet.is_enabled {
                Some(SkipReason::SnippetOff)
            } else {
                // Disabled only through its ancestry; an unresolvable group reads as the snippet.
                Some(
                    nearest_disabled_group(snippet.group_id, groups)
                        .map_or(SkipReason::SnippetOff, |group_id| SkipReason::GroupOff {
                            group_id,
                        }),
                )
            };
            let propagate = snippet.case_mode == CaseMode::Propagate;
            let mut seen = HashSet::new();
            let triggers: Vec<&String> = snippet
                .triggers
                .iter()
                .filter(|t| !t.is_empty())
                .filter(|t| {
                    seen.insert(if propagate {
                        folded_string(t)
                    } else {
                        nfc(t).into_owned()
                    })
                })
                .collect();
            if triggers.is_empty() {
                continue;
            }
            let snippet_index = data.snippets.len() as u32;
            data.snippets.push(IndexedSnippet {
                id: snippet.id,
                updated_at: snippet.updated_at,
                case_mode: snippet.case_mode,
                rule: effective.rule.as_ref().map(IndexedRule::new),
                insertion: effective.insertion,
                off,
                rule_group: effective.rule_group.as_ref().map(|g| g.id),
            });
            if effective.is_enabled {
                data.live_entries += triggers.len();
            }
            for trigger in triggers {
                let entry_index = data.entries.len() as u32;
                let clusters: Vec<&str> = trigger.graphemes(true).collect();
                data.entries.push(Entry {
                    snippet: snippet_index,
                    trigger: trigger.clone(),
                    length: clusters.len(),
                    fire_mode: snippet.fire_mode,
                });
                if propagate {
                    let key: Vec<String> = clusters.iter().map(|c| folded_key(c)).collect();
                    data.folded.insert(&key, entry_index);
                } else {
                    let key: Vec<String> = clusters.iter().map(|c| nfc(c).into_owned()).collect();
                    data.exact.insert(&key, entry_index);
                }
            }
        }
        data
    }

    pub fn trigger_count(&self) -> usize {
        self.live_entries
    }

    /// The trigger completed by the most recent keystroke (already applied to `buffer`), if any.
    ///
    /// Only character inputs fire. Immediate snippets fire when the buffer ends with the trigger;
    /// whole-word snippets when the last cluster is a separator, the clusters before it end with
    /// the trigger, and the cluster before the trigger is a separator or the buffer start.
    /// Longest trigger wins, then the most recently updated snippet, then the earliest entry.
    pub fn find_match(
        &self,
        buffer: &KeyBuffer,
        last_input: &KeyInput,
        bundle_id: Option<&str>,
    ) -> Option<TriggerMatch> {
        if !matches!(last_input, KeyInput::Character(_)) || self.live_entries == 0 {
            return None;
        }
        let count = buffer.len();
        if count == 0 {
            return None;
        }
        if bundle_id.is_some_and(|id| self.config.denied_bundle_ids.contains(id)) {
            return None;
        }
        let (best, last) = self.best(buffer, bundle_id, Pass::Fire);
        let best = best?;
        let entry = &self.entries[best.entry as usize];
        let snippet = &self.snippets[entry.snippet as usize];
        let typed = buffer.text(best.start, best.end);
        let case_style = match snippet.case_mode {
            CaseMode::Propagate => case_propagation::detect(&typed),
            CaseMode::Exact => CaseStyle::AsIs,
        };
        Some(TriggerMatch {
            snippet_id: snippet.id,
            trigger: entry.trigger.clone(),
            typed_trigger: typed,
            delete_count: entry.length,
            separator: (entry.fire_mode == FireMode::WholeWord).then(|| last.clone()),
            insertion: snippet.insertion_for(bundle_id, &self.config),
            case_style,
            fire_mode: entry.fire_mode,
        })
    }

    /// See [`TriggerIndex::find_skipped`](super::TriggerIndex::find_skipped).
    pub fn find_skipped(
        &self,
        buffer: &KeyBuffer,
        last_input: &KeyInput,
        bundle_id: Option<&str>,
    ) -> Option<SkippedMatch> {
        if !matches!(last_input, KeyInput::Character(_))
            || self.entries.is_empty()
            || buffer.is_empty()
        {
            return None;
        }
        let (best, _) = self.best(buffer, bundle_id, Pass::Explain);
        let entry = &self.entries[best?.entry as usize];
        let snippet = &self.snippets[entry.snippet as usize];
        // Checked in the order the engine applies them: excluded apps first, then enablement,
        // then the snippet's (own or inherited) rule.
        let cause =
            if let Some(id) = bundle_id.filter(|id| self.config.denied_bundle_ids.contains(*id)) {
                let level = if MatchingConfig::default_denied_bundle_ids().contains(id) {
                    RuleLevel::Default
                } else {
                    RuleLevel::AppSetting
                };
                SkipReason::Rule {
                    level,
                    group_id: None,
                }
            } else if let Some(off) = &snippet.off {
                off.clone()
            } else if snippet
                .rule
                .as_ref()
                .is_some_and(|rule| !rule.permits(bundle_id))
            {
                match snippet.rule_group {
                    Some(group_id) => SkipReason::Rule {
                        level: RuleLevel::Group,
                        group_id: Some(group_id),
                    },
                    None => SkipReason::Rule {
                        level: RuleLevel::Snippet,
                        group_id: None,
                    },
                }
            } else {
                // It may fire: the caller asked without `find_match` having returned `None`.
                return None;
            };
        Some(SkippedMatch {
            snippet_id: snippet.id,
            trigger: entry.trigger.clone(),
            cause,
        })
    }

    /// The best-ranked trigger ending at the last keystroke, plus that keystroke's cluster.
    fn best<'b>(
        &self,
        buffer: &'b KeyBuffer,
        bundle_id: Option<&str>,
        pass: Pass,
    ) -> (Option<Candidate>, &'b String) {
        let count = buffer.len();
        let mut best = None;
        self.search(buffer, count, false, bundle_id, pass, &mut best);
        let last = &buffer.cluster(count - 1).text;
        if count >= 2 && is_separator(&self.config, last) {
            self.search(buffer, count - 1, true, bundle_id, pass, &mut best);
        }
        (best, last)
    }

    /// Walks both tries backwards from cluster `end - 1`, considering every trigger ending at `end`.
    fn search(
        &self,
        buffer: &KeyBuffer,
        end: usize,
        whole_word: bool,
        bundle_id: Option<&str>,
        pass: Pass,
        best: &mut Option<Candidate>,
    ) {
        let mut folded = Some(0u32);
        let mut exact = Some(0u32);
        let mut depth = 0;
        while depth < end && (folded.is_some() || exact.is_some()) {
            let cluster = buffer.cluster(end - 1 - depth);
            folded = folded.and_then(|node| self.folded.child(node, &cluster.folded));
            exact = exact.and_then(|node| self.exact.child(node, &cluster.exact));
            depth += 1;
            let start = end - depth;
            if let Some(node) = folded {
                self.consider(
                    self.folded.entries(node),
                    start,
                    end,
                    buffer,
                    whole_word,
                    bundle_id,
                    pass,
                    best,
                );
            }
            if let Some(node) = exact {
                self.consider(
                    self.exact.entries(node),
                    start,
                    end,
                    buffer,
                    whole_word,
                    bundle_id,
                    pass,
                    best,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn consider(
        &self,
        candidates: &[u32],
        start: usize,
        end: usize,
        buffer: &KeyBuffer,
        whole_word: bool,
        bundle_id: Option<&str>,
        pass: Pass,
        best: &mut Option<Candidate>,
    ) {
        for &entry_index in candidates {
            let entry = &self.entries[entry_index as usize];
            if (entry.fire_mode == FireMode::WholeWord) != whole_word {
                continue;
            }
            if whole_word
                && start > 0
                && !is_separator(&self.config, &buffer.cluster(start - 1).text)
            {
                continue;
            }
            let snippet = &self.snippets[entry.snippet as usize];
            if pass == Pass::Fire
                && (snippet.off.is_some()
                    || snippet
                        .rule
                        .as_ref()
                        .is_some_and(|rule| !rule.permits(bundle_id)))
            {
                continue;
            }
            if best.is_some_and(|current| !self.outranks(entry_index, current.entry)) {
                continue;
            }
            *best = Some(Candidate {
                entry: entry_index,
                start,
                end,
            });
        }
    }

    /// Longest trigger, then most recently updated, then earliest in the input.
    fn outranks(&self, lhs: u32, rhs: u32) -> bool {
        let (l, r) = (&self.entries[lhs as usize], &self.entries[rhs as usize]);
        if l.length != r.length {
            return l.length > r.length;
        }
        let (l_date, r_date) = (
            self.snippets[l.snippet as usize].updated_at,
            self.snippets[r.snippet as usize].updated_at,
        );
        if l_date != r_date {
            return l_date > r_date;
        }
        lhs < rhs
    }
}
