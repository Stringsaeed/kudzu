//! The loaded library: lookups, inherited settings and rendering helpers.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, FixedOffset};
use kudzu_core::matching::MatchingConfig;
use kudzu_core::template::{self, FormFieldSpec, Node, RenderContext, RenderResult, TemplateTag};
use kudzu_core::types::{AppRule, GroupRecord, InsertionMethod, SnippetRecord};
use uuid::Uuid;

use crate::EngineLibrary;

/// Nested snippet depth limit, matching the renderer.
const MAX_DEPTH: usize = 8;

/// An immutable snapshot of what the engine matches and renders against.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Library {
    pub snippets: HashMap<Uuid, SnippetRecord>,
    pub groups: Vec<GroupRecord>,
    pub config: MatchingConfig,
    pub locale: String,
}

impl Library {
    /// Snapshot of `library`; duplicate snippet ids keep the first record. An empty locale uses `fallback_locale`.
    pub fn new(library: &EngineLibrary, fallback_locale: impl FnOnce() -> String) -> Self {
        let mut snippets = HashMap::with_capacity(library.snippets.len());
        for snippet in &library.snippets {
            snippets.entry(snippet.id).or_insert_with(|| snippet.clone());
        }
        let locale = if library.locale.trim().is_empty() { fallback_locale() } else { library.locale.clone() };
        Self { snippets, groups: library.groups.clone(), config: library.config.clone(), locale }
    }

    /// Template source of snippet `id`.
    pub fn source(&self, id: Uuid) -> Option<String> {
        self.snippets.get(&id).map(|s| s.content.clone())
    }

    /// Form fields of `snippet`, including nested snippets.
    pub fn form_fields(&self, snippet: &SnippetRecord) -> Vec<FormFieldSpec> {
        template::form_fields(&snippet.content, &|id| self.source(id))
    }

    /// Whether `snippet` (or a nested snippet) reads the clipboard; avoids touching it otherwise.
    pub fn uses_clipboard(&self, snippet: &SnippetRecord) -> bool {
        let mut visited = HashSet::from([snippet.id]);
        self.source_uses_clipboard(&snippet.content, &mut visited, 0)
    }

    fn source_uses_clipboard(&self, source: &str, visited: &mut HashSet<Uuid>, depth: usize) -> bool {
        for node in template::parse(source).nodes {
            let Node::Tag(tag) = node else { continue };
            match template::typed(&tag) {
                TemplateTag::Clipboard { .. } => return true,
                TemplateTag::Snippet { id } if depth < MAX_DEPTH && visited.insert(id) => {
                    if let Some(nested) = self.snippets.get(&id) {
                        if self.source_uses_clipboard(&nested.content, visited, depth + 1) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Renders `snippet` at `now` with the given clipboard snapshot and form values.
    pub fn render(
        &self,
        snippet: &SnippetRecord,
        now: DateTime<FixedOffset>,
        clipboard: Option<String>,
        form_values: HashMap<String, String>,
    ) -> RenderResult {
        let resolve = |id: Uuid| self.source(id);
        let resolve_formatting = |id| self.snippets.get(&id).and_then(|s| s.formatting.clone());
        let context = RenderContext {
            resolve_formatting: Some(&resolve_formatting),
            now,
            locale: self.locale.clone(),
            clipboard,
            form_values,
            resolve_snippet: &resolve,
            root_snippet_id: Some(snippet.id),
        };
        template::render_formatted(&snippet.content, snippet.formatting.as_ref(), &context)
    }

    /// The snippet's own rule and insertion, else the nearest ancestor group's (cycle-safe).
    fn inherited(&self, snippet: &SnippetRecord) -> (Option<AppRule>, Option<InsertionMethod>) {
        let mut rule = snippet.rule.clone();
        let mut insertion = snippet.insertion;
        let mut visited = HashSet::new();
        let mut next = snippet.group_id;
        while let Some(id) = next {
            if (rule.is_some() && insertion.is_some()) || !visited.insert(id) {
                break;
            }
            let Some(group) = self.groups.iter().find(|g| g.id == id) else { break };
            if rule.is_none() {
                rule = group.rule.clone();
            }
            if insertion.is_none() {
                insertion = group.insertion;
            }
            next = group.parent_id;
        }
        (rule, insertion)
    }

    /// Insertion method for `snippet` in `bundle_id`: rule's per-app override ▸ snippet/group ▸
    /// global per-app ▸ global default.
    pub fn insertion_for(&self, snippet: &SnippetRecord, bundle_id: Option<&str>) -> InsertionMethod {
        let (rule, insertion) = self.inherited(snippet);
        if let (Some(rule), Some(bundle)) = (&rule, bundle_id) {
            if let Some(method) = rule.insertion {
                if rule.bundle_ids.iter().any(|b| b == bundle) {
                    return method;
                }
            }
        }
        if let Some(method) = insertion {
            return method;
        }
        if let Some(method) = bundle_id.and_then(|b| self.config.app_insertions.get(b)) {
            return *method;
        }
        self.config.default_insertion
    }

    /// Whether nothing may ever expand in `bundle_id` (password managers, login window…).
    pub fn is_denied(&self, bundle_id: Option<&str>) -> bool {
        bundle_id.is_some_and(|b| self.config.denied_bundle_ids.contains(b))
    }
}

/// `values` completed with each field's default, else its first choice, else empty text.
pub(crate) fn with_defaults(fields: &[FormFieldSpec], mut values: HashMap<String, String>) -> HashMap<String, String> {
    for field in fields {
        values.entry(field.name.clone()).or_insert_with(|| {
            field.default_value.clone().or_else(|| field.options.first().cloned()).unwrap_or_default()
        });
    }
    values
}

/// Local time with its UTC offset.
pub(crate) fn now() -> DateTime<FixedOffset> {
    chrono::Local::now().fixed_offset()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::template::FormFieldKind;
    use kudzu_core::types::AppRuleMode;

    fn library(snippets: Vec<SnippetRecord>, groups: Vec<GroupRecord>) -> Library {
        let lib = EngineLibrary { snippets, groups, config: MatchingConfig::default(), locale: String::new() };
        Library::new(&lib, || "en_GB".into())
    }

    #[test]
    fn empty_locale_falls_back_and_duplicates_keep_first() {
        let a = SnippetRecord::new("a", vec!["a".into()], "first");
        let mut b = a.clone();
        b.content = "second".into();
        let lib = library(vec![a.clone(), b], vec![]);
        assert_eq!(lib.locale, "en_GB");
        assert_eq!(lib.snippets.len(), 1);
        assert_eq!(lib.source(a.id).as_deref(), Some("first"));
    }

    #[test]
    fn insertion_precedence() {
        let mut parent = GroupRecord::new("parent");
        parent.insertion = Some(InsertionMethod::Type);
        let mut child = GroupRecord::new("child");
        child.parent_id = Some(parent.id);
        let mut snippet = SnippetRecord::new("s", vec!["s".into()], "x");
        snippet.group_id = Some(child.id);
        let lib = library(vec![snippet.clone()], vec![parent.clone(), child.clone()]);

        // Inherited from the grandparent group, even in a terminal.
        assert_eq!(lib.insertion_for(&snippet, Some("com.apple.Terminal")), InsertionMethod::Type);

        // Snippet override beats the group.
        let mut own = snippet.clone();
        own.insertion = Some(InsertionMethod::Paste);
        assert_eq!(lib.insertion_for(&own, None), InsertionMethod::Paste);

        // A rule's per-app override wins for its listed apps only.
        own.rule = Some(AppRule {
            mode: AppRuleMode::Allow,
            bundle_ids: vec!["com.example.app".into()],
            insertion: Some(InsertionMethod::Type),
        });
        assert_eq!(lib.insertion_for(&own, Some("com.example.app")), InsertionMethod::Type);
        assert_eq!(lib.insertion_for(&own, Some("com.other")), InsertionMethod::Paste);
    }

    #[test]
    fn insertion_falls_back_to_global_settings() {
        let snippet = SnippetRecord::new("s", vec!["s".into()], "x");
        let lib = library(vec![snippet.clone()], vec![]);
        assert_eq!(lib.insertion_for(&snippet, Some("com.apple.Terminal")), InsertionMethod::Paste);
        assert_eq!(lib.insertion_for(&snippet, Some("com.apple.TextEdit")), InsertionMethod::Automatic);
        assert_eq!(lib.insertion_for(&snippet, None), InsertionMethod::Automatic);
    }

    #[test]
    fn group_cycles_terminate() {
        let mut a = GroupRecord::new("a");
        let mut b = GroupRecord::new("b");
        a.parent_id = Some(b.id);
        b.parent_id = Some(a.id);
        let mut snippet = SnippetRecord::new("s", vec!["s".into()], "x");
        snippet.group_id = Some(a.id);
        let lib = library(vec![snippet.clone()], vec![a, b]);
        assert_eq!(lib.insertion_for(&snippet, None), InsertionMethod::Automatic);
    }

    #[test]
    fn denied_apps() {
        let lib = library(vec![], vec![]);
        assert!(lib.is_denied(Some("com.apple.loginwindow")));
        assert!(!lib.is_denied(Some("com.apple.TextEdit")));
        assert!(!lib.is_denied(None));
    }

    #[test]
    fn defaults_fill_missing_values_only() {
        let fields = vec![
            FormFieldSpec { name: "a".into(), kind: FormFieldKind::Text, options: vec![], default_value: Some("A".into()) },
            FormFieldSpec { name: "b".into(), kind: FormFieldKind::Choice, options: vec!["x".into(), "y".into()], default_value: None },
            FormFieldSpec { name: "c".into(), kind: FormFieldKind::Text, options: vec![], default_value: None },
        ];
        let values = with_defaults(&fields, HashMap::from([("a".to_string(), "given".to_string())]));
        assert_eq!(values["a"], "given");
        assert_eq!(values["b"], "x");
        assert_eq!(values["c"], "");
    }
}
