//! Reversed-trigger suffix tries: a keystroke costs O(longest trigger), independent of library size.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use super::key_buffer::{folded_key, folded_string, nfc, KeyBuffer};
use super::settings::{resolve_insertion, EffectiveSnippet, GroupResolver};
use super::{case_propagation, CaseStyle, KeyInput, MatchingConfig, TriggerMatch};
use crate::types::{
    AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, SnippetRecord,
};

/// The immutable data behind a `TriggerIndex`.
#[derive(Debug, Default)]
pub(crate) struct IndexData {
    pub config: MatchingConfig,
    snippets: Vec<IndexedSnippet>,
    entries: Vec<Entry>,
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
            if !effective.is_enabled {
                continue;
            }
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
            });
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
        self.entries.len()
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
        if !matches!(last_input, KeyInput::Character(_)) || self.entries.is_empty() {
            return None;
        }
        let count = buffer.len();
        if count == 0 {
            return None;
        }
        if bundle_id.is_some_and(|id| self.config.denied_bundle_ids.contains(id)) {
            return None;
        }
        let mut best = None;
        self.search(buffer, count, false, bundle_id, &mut best);
        let last = &buffer.cluster(count - 1).text;
        if count >= 2 && is_separator(&self.config, last) {
            self.search(buffer, count - 1, true, bundle_id, &mut best);
        }
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

    /// Walks both tries backwards from cluster `end - 1`, considering every trigger ending at `end`.
    fn search(
        &self,
        buffer: &KeyBuffer,
        end: usize,
        whole_word: bool,
        bundle_id: Option<&str>,
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
            if snippet
                .rule
                .as_ref()
                .is_some_and(|rule| !rule.permits(bundle_id))
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
