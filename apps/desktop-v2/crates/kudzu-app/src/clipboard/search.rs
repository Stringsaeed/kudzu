//! Snapshot-scoped search and the agent-safe clipboard boundary.
use super::*;
use kudzu_core::{
    clipboard::query::{ClipFilter, ClipQuery, ClipSearch, SearchScope},
    secrets,
};

pub const MAX_ITEM_CHARS: usize = 50_000;
const MAX_AGENT_MATCHES: usize = 100;
pub(super) struct SearchCache {
    revision: Revision,
    local: Option<Arc<ClipSearch<'static>>>,
    agent: Option<Arc<ClipSearch<'static>>>,
}
/// Indices are valid only for this result's `history`, not for any newer snapshot.
pub struct ClipResults {
    pub revision: Revision,
    pub history: Arc<History>,
    pub indices: Vec<usize>,
}
impl ClipResults {
    /// Resolve a result position through its own immutable history snapshot.
    pub fn get(&self, position: usize) -> Option<&ClipEntry> {
        self.indices
            .get(position)
            .and_then(|&index| self.history.entries.get(index))
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct AgentItem {
    pub id: Uuid,
    pub kind: ClipKind,
    pub copied_at: chrono::DateTime<chrono::Utc>,
    pub copy_count: u32,
    pub pinned: bool,
    pub length: Option<usize>,
    pub lines: Option<usize>,
    pub app: Option<String>,
    pub files: Vec<String>,
    pub image: Option<ImageSize>,
    pub color: Option<String>,
    pub text: Option<String>,
    pub withheld: Option<String>,
    pub redacted: Vec<String>,
    pub truncated: bool,
}
impl ClipboardService {
    fn indexed(
        &self,
        scope: SearchScope,
        history: Arc<History>,
        revision: Revision,
    ) -> Arc<ClipSearch<'static>> {
        {
            let cache = self.search.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = cache.as_ref().filter(|entry| entry.revision == revision) {
                let existing = match scope {
                    SearchScope::Local => &entry.local,
                    SearchScope::Agent => &entry.agent,
                };
                if let Some(index) = existing {
                    return index.clone();
                }
            }
        }
        // Building haystacks can scan 5,000 excerpts; do it without holding the cache mutex.
        let built = Arc::new(ClipSearch::shared(history.clone(), scope));
        let mut cache = self.search.lock().unwrap_or_else(PoisonError::into_inner);
        if cache
            .as_ref()
            .is_some_and(|entry| entry.revision > revision)
        {
            return built;
        }
        if cache
            .as_ref()
            .is_none_or(|entry| entry.revision != revision)
        {
            *cache = Some(SearchCache {
                revision,
                local: None,
                agent: None,
            });
        }
        let entry = cache.as_mut().expect("search cache initialized");
        let slot = match scope {
            SearchScope::Local => &mut entry.local,
            SearchScope::Agent => &mut entry.agent,
        };
        slot.get_or_insert_with(|| {
            #[cfg(test)]
            self.cache_builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            built
        })
        .clone()
    }
    /// Local-only: the shared history contains unredacted clipboard data.
    pub fn query(&self, filter: ClipFilter, query: &ClipQuery) -> ClipResults {
        let (revision, history) = self.watch().snapshot();
        let indices = if filter == ClipFilter::All
            && query.terms.is_empty()
            && query.app.is_none()
            && query.kind.is_none()
        {
            (0..history.entries.len()).collect()
        } else {
            self.indexed(SearchScope::Local, history.clone(), revision)
                .indices(filter, query)
        };
        ClipResults {
            revision,
            history,
            indices,
        }
    }
    /// Search redacted content. Full text and metadata are separately redacted before return.
    pub fn agent_query(&self, filter: ClipFilter, query: &ClipQuery) -> Vec<AgentItem> {
        let (revision, history) = self.watch().snapshot();
        let indices = self
            .indexed(SearchScope::Agent, history.clone(), revision)
            .indices(filter, query);
        indices
            .into_iter()
            .take(MAX_AGENT_MATCHES)
            .map(|i| self.agent_summary(&history.entries[i], false))
            .collect()
    }
    /// Full, bounded agent-safe content and metadata for one current item.
    pub fn agent_item(&self, id: Uuid) -> Option<AgentItem> {
        let history = self.watch().get();
        history.get(id).map(|entry| self.agent_summary(entry, true))
    }
    fn agent_summary(&self, entry: &ClipEntry, full: bool) -> AgentItem {
        let raw = if full {
            self.store
                .items(entry.id)
                .as_deref()
                .and_then(clipboard::text_of)
                .or_else(|| entry.text.clone())
        } else {
            entry.text.clone()
        };
        let complete = full
            && raw
                .as_ref()
                .is_some_and(|s| s.chars().count() == entry.text_len)
            || entry.text_len <= clipboard::TEXT_EXCERPT_LIMIT;
        let forced = entry.secret_override == Some(true);
        let detected = entry.is_secret()
            || (entry.secret_override != Some(false)
                && raw.as_deref().is_some_and(secrets::contains_secret));
        let withheld = if forced {
            Some("The user marked this as a secret, so Kudzu doesn't share it.".to_owned())
        } else if detected {
            Some(format!(
                "This looks like a {}, so Kudzu doesn't share it.",
                entry.secret.map(|s| s.label()).unwrap_or("secret")
            ))
        } else {
            None
        };
        let mut truncated = false;
        let text = if withheld.is_some() {
            None
        } else {
            raw.map(|text| {
                let text = if !complete && entry.secret_override != Some(false) {
                    &text[..text.rfind(char::is_whitespace).unwrap_or(0)]
                } else {
                    &text
                };
                let safe = if entry.secret_override == Some(false) {
                    text.to_owned()
                } else {
                    secrets::redact(text).text
                };
                truncated = !complete || safe.chars().count() > MAX_ITEM_CHARS;
                clipboard::excerpt(&safe, MAX_ITEM_CHARS)
            })
        };
        let mut redacted = Vec::new();
        if withheld.is_some() {
            redacted.push(
                entry
                    .secret
                    .map(|kind| kind.label())
                    .unwrap_or("secret")
                    .to_owned(),
            );
        }
        let mut safe = |s: &str| {
            let result = secrets::redact(s);
            for kind in &result.kinds {
                let label = kind.label().to_owned();
                if !redacted.contains(&label) {
                    redacted.push(label);
                }
            }
            result.text
        };
        let app = entry.source.as_ref().map(|source| safe(&source.name));
        let files = entry.files.iter().map(|file| safe(file)).collect();
        let color = entry.color.as_deref().map(safe);
        AgentItem {
            id: entry.id,
            kind: entry.kind,
            copied_at: entry.copied_at,
            copy_count: entry.copy_count,
            pinned: entry.pinned,
            length: withheld.is_none().then_some(entry.text_len),
            lines: withheld.is_none().then_some(entry.line_count),
            app,
            files,
            image: entry.image,
            color,
            text,
            withheld,
            redacted,
            truncated,
        }
    }
    /// Resolve the same numbered copy as local expansion but never return a secret.
    pub fn agent_history_text(&self, n: usize, current: Option<&str>) -> Option<String> {
        if !self.settings.get().clipboard_enabled {
            return None;
        }
        let history = self.watch().get();
        let entry = self.history_entry(&history, n, current)?;
        if entry.is_secret() {
            return None;
        }
        let text = self
            .store
            .items(entry.id)
            .as_deref()
            .and_then(clipboard::text_of)
            .or_else(|| entry.text.clone())?;
        if entry.secret_override != Some(false) && secrets::contains_secret(&text) {
            return None;
        }
        Some(clipboard::excerpt(&text, MAX_ITEM_CHARS))
    }
}
