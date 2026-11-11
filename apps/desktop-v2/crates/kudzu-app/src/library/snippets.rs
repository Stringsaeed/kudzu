use super::{LibraryError, LibraryService};
use crate::persist::Revision;
use kudzu_core::{
    formatting::Formatting,
    library::model::{AppRule, CaseMode, FireMode, InsertionMethod, SnippetRecord},
};
use std::collections::HashSet;
use uuid::Uuid;

/// Undo retains snippet records only. Form memory pruning belongs to the forms reactor.
#[derive(Clone)]
pub struct Deleted {
    pub snippets: Vec<SnippetRecord>,
}

/// Editable snippet fields. Identity and timestamps are assigned only by the service.
#[derive(Clone)]
pub struct SnippetDraft {
    pub name: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub formatting: Option<Formatting>,
    pub group_id: Option<Uuid>,
    pub fire_mode: FireMode,
    pub case_mode: CaseMode,
    pub insertion: Option<InsertionMethod>,
    pub rule: Option<AppRule>,
    pub is_favorite: bool,
    pub is_enabled: bool,
}
impl Default for SnippetDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            triggers: vec![],
            content: String::new(),
            formatting: None,
            group_id: None,
            fire_mode: FireMode::default(),
            case_mode: CaseMode::default(),
            insertion: None,
            rule: None,
            is_favorite: false,
            is_enabled: true,
        }
    }
}
impl LibraryService {
    pub fn delete_snippets(&self, ids: &[Uuid]) -> Result<(Deleted, Revision), LibraryError> {
        let ids: HashSet<_> = ids.iter().copied().collect();
        self.transact(|library| {
            let mut snippets = Vec::new();
            library.snippets.retain(|snippet| {
                if ids.contains(&snippet.id) {
                    snippets.push(snippet.clone());
                    false
                } else {
                    true
                }
            });
            Ok(Deleted { snippets })
        })
    }

    pub fn restore(&self, deleted: Deleted) -> Result<(Vec<Uuid>, Revision), LibraryError> {
        self.transact(|library| {
            let mut present: HashSet<_> = library.snippets.iter().map(|s| s.id).collect();
            let mut restored = Vec::new();
            for snippet in deleted.snippets {
                if present.insert(snippet.id) {
                    restored.push(snippet.id);
                    library.snippets.push(snippet);
                }
            }
            Ok(restored)
        })
    }

    pub fn move_snippets(
        &self,
        ids: &[Uuid],
        group_id: Option<Uuid>,
    ) -> Result<Revision, LibraryError> {
        let ids: HashSet<_> = ids.iter().copied().collect();
        self.transact(|library| {
            let now = self.clock.now().into();
            for snippet in library.snippets.iter_mut().filter(|s| ids.contains(&s.id)) {
                snippet.group_id = group_id;
                snippet.updated_at = now;
            }
            Ok(())
        })
        .map(|(_, revision)| revision)
    }

    pub fn duplicate_snippet(&self, id: Uuid) -> Result<(SnippetRecord, Revision), LibraryError> {
        self.transact(|library| {
            let at = library
                .snippets
                .iter()
                .position(|s| s.id == id)
                .ok_or(LibraryError::MissingSnippet)?;
            let mut copy = library.snippets[at].clone();
            let taken: HashSet<_> = library
                .snippets
                .iter()
                .flat_map(|s| s.triggers.iter().map(|t| t.to_lowercase()))
                .collect();
            copy.triggers = copy
                .triggers
                .iter()
                .map(|trigger| {
                    (2u64..)
                        .map(|n| format!("{trigger}{n}"))
                        .find(|candidate| !taken.contains(&candidate.to_lowercase()))
                        .expect("trigger suffix exhausted")
                })
                .collect();
            copy.id = self.ids.next_id();
            if !copy.name.is_empty() {
                copy.name.push_str(" copy");
            }
            copy.created_at = self.clock.now().into();
            copy.updated_at = copy.created_at;
            library.snippets.insert(at + 1, copy.clone());
            Ok(copy)
        })
    }

    pub fn update_snippet(
        &self,
        mut snippet: SnippetRecord,
    ) -> Result<(SnippetRecord, Revision), LibraryError> {
        self.transact(|library| {
            let existing = library
                .snippets
                .iter_mut()
                .find(|s| s.id == snippet.id)
                .ok_or(LibraryError::MissingSnippet)?;
            snippet.created_at = existing.created_at;
            snippet.updated_at = self.clock.now().into();
            snippet.triggers.retain(|t| !t.trim().is_empty());
            *existing = snippet.clone();
            Ok(snippet)
        })
    }

    pub fn create_snippet(
        &self,
        draft: SnippetDraft,
    ) -> Result<(SnippetRecord, Revision), LibraryError> {
        self.transact(|library| {
            let now = self.clock.now().into();
            let snippet = SnippetRecord {
                id: self.ids.next_id(),
                name: draft.name,
                triggers: draft.triggers,
                content: draft.content,
                formatting: draft.formatting,
                group_id: draft.group_id,
                fire_mode: draft.fire_mode,
                case_mode: draft.case_mode,
                insertion: draft.insertion,
                rule: draft.rule,
                is_favorite: draft.is_favorite,
                is_enabled: draft.is_enabled,
                created_at: now,
                updated_at: now,
            };
            library.snippets.insert(0, snippet.clone());
            Ok(snippet)
        })
    }
}
