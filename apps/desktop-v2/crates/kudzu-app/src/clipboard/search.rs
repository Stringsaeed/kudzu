//! Snapshot-scoped search caches and the restricted agent representation.
use super::*;
use kudzu_core::clipboard::query::{ClipFilter, ClipQuery, ClipSearch, SearchScope};

pub(super) struct SearchCache {
    revision: Revision,
    local: ClipSearch<'static>,
    agent: ClipSearch<'static>,
}
/// The restricted representation exposed to agents. It contains no raw clipboard metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentClip {
    pub id: Uuid,
    pub kind: ClipKind,
    pub excerpt: Option<String>,
}

impl ClipboardService {
    fn search<R>(&self, run: impl FnOnce(&SearchCache) -> R) -> R {
        let (revision, history) = self.watch().snapshot();
        let mut cache = self.search.lock().unwrap_or_else(PoisonError::into_inner);
        if cache
            .as_ref()
            .is_none_or(|entry| entry.revision != revision)
        {
            #[cfg(test)]
            self.cache_builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *cache = Some(SearchCache {
                revision,
                local: ClipSearch::owned(history.entries.clone(), SearchScope::Local),
                agent: ClipSearch::owned(history.entries.clone(), SearchScope::Agent),
            });
        }
        let entry = cache.as_ref().expect("search cache initialized");
        run(entry)
    }
    /// Local-only: results contain unredacted clipboard data.
    pub fn query(&self, filter: ClipFilter, query: &ClipQuery) -> Vec<ClipEntry> {
        self.search(|cache| {
            cache
                .local
                .search(filter, query)
                .into_iter()
                .cloned()
                .collect()
        })
    }
    /// Agent-scoped matching and redaction; raw entries never leave this method.
    pub fn agent_query(&self, filter: ClipFilter, query: &ClipQuery) -> Vec<AgentClip> {
        self.search(|cache| {
            cache
                .agent
                .search(filter, query)
                .into_iter()
                .map(|e| {
                    let excerpt = e.text.as_deref().and_then(|text| match e.secret_override {
                        Some(true) => None,
                        Some(false) => Some(text.to_owned()),
                        None => {
                            let complete = e.text_len <= clipboard::TEXT_EXCERPT_LIMIT;
                            let text = if complete {
                                text
                            } else {
                                &text[..text.rfind(char::is_whitespace).unwrap_or(0)]
                            };
                            let redacted = kudzu_core::secrets::redact(text);
                            if complete && redacted.whole {
                                None
                            } else {
                                Some(redacted.text)
                            }
                        }
                    });
                    AgentClip {
                        id: e.id,
                        kind: e.kind,
                        excerpt,
                    }
                })
                .collect()
        })
    }
}
