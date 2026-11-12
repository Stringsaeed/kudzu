//! The derived list, cached by its inputs: the library snapshot (by pointer: every revision is
//! a new `Arc`), the view, the filter's text and the sort. The folded [`Indexed`] snapshot is
//! kept across queries, so a keystroke only ranks. The cache holds the snapshot's `Arc`, so a
//! pointer can't be reused while it is compared.
//!
//! The change census (`snippet-list.tsx` `ListMotion`): when a new snapshot adds or removes at
//! most [`MAX_MOVING`] snippets, the added rows rise in once; a bigger change (an import, a
//! sync) lands still. Rune never replays a row's entrance when the virtual list remounts it.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use kudzu_core::KudzuLibrary;
use uuid::Uuid;

use super::model::{list, Hit, Indexed, Sort};
use crate::main::state::LibraryFilter;

/// Bulk changes (imports, syncs) just appear; motion is for the few rows a person touched.
pub const MAX_MOVING: usize = 12;

/// One built list.
pub struct Listing {
    pub indexed: Rc<Indexed>,
    pub hits: Vec<Hit>,
    pub ids: Vec<Uuid>,
    /// Changes whenever the rows are rebuilt: `virtual_rows_version`.
    pub version: u64,
    scope: LibraryFilter,
    query: String,
    sort: Sort,
}

/// The last library change, as rows see it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Motion {
    /// Rows that arrived: they rise in.
    pub fresh: HashSet<Uuid>,
    /// The change was bulk (or the first load): everything lands at once.
    pub still: bool,
}

pub(super) struct Cache {
    listing: Rc<Listing>,
    pub(super) motion: Motion,
    next_version: u64,
}

fn census(before: &KudzuLibrary, after: &KudzuLibrary) -> Motion {
    let old: HashSet<Uuid> = before.snippets.iter().map(|s| s.id).collect();
    let new: HashSet<Uuid> = after.snippets.iter().map(|s| s.id).collect();
    let fresh: HashSet<Uuid> = new.difference(&old).copied().collect();
    let gone = old.difference(&new).count();
    let moving = fresh.len() + gone;
    if moving > MAX_MOVING {
        return Motion {
            fresh: HashSet::new(),
            still: true,
        };
    }
    Motion {
        fresh,
        still: false,
    }
}

/// The listing for these inputs, from `cache` when nothing changed.
pub(super) fn refresh(
    cache: &mut Option<Cache>,
    library: &Arc<KudzuLibrary>,
    scope: &LibraryFilter,
    query: &str,
    sort: Sort,
    now: DateTime<Utc>,
) -> Rc<Listing> {
    if let Some(c) = cache.as_ref() {
        let l = &c.listing;
        let same_library = Arc::ptr_eq(&l.indexed.library, library);
        if same_library && l.scope == *scope && l.query == query && l.sort == sort {
            return Rc::clone(&c.listing);
        }
    }
    let (indexed, motion, next_version) = match cache.take() {
        Some(c) if Arc::ptr_eq(&c.listing.indexed.library, library) => {
            (Rc::clone(&c.listing.indexed), c.motion, c.next_version)
        }
        Some(c) => (
            Rc::new(Indexed::new(Arc::clone(library))),
            census(&c.listing.indexed.library, library),
            c.next_version,
        ),
        None => (
            Rc::new(Indexed::new(Arc::clone(library))),
            Motion {
                fresh: HashSet::new(),
                still: true,
            },
            0,
        ),
    };
    let hits = list(&indexed, scope, query, sort, now);
    let ids = hits
        .iter()
        .map(|h| indexed.snippet(h.position).id)
        .collect();
    let listing = Rc::new(Listing {
        indexed,
        hits,
        ids,
        version: next_version,
        scope: scope.clone(),
        query: query.to_owned(),
        sort,
    });
    *cache = Some(Cache {
        listing: Rc::clone(&listing),
        motion,
        next_version: next_version + 1,
    });
    listing
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_app::fixtures::SampleState;

    fn now() -> DateTime<Utc> {
        DateTime::UNIX_EPOCH
    }

    #[test]
    fn the_cache_keeps_the_snapshot_and_bumps_the_version_on_change() {
        let lib = Arc::new(SampleState::load().unwrap().library);
        let mut cache = None;
        let a = refresh(&mut cache, &lib, &LibraryFilter::All, "", Sort::Name, now());
        let b = refresh(&mut cache, &lib, &LibraryFilter::All, "", Sort::Name, now());
        assert!(Rc::ptr_eq(&a, &b), "same inputs, same listing");
        let c = refresh(
            &mut cache,
            &lib,
            &LibraryFilter::All,
            "sig",
            Sort::Name,
            now(),
        );
        assert!(Rc::ptr_eq(&a.indexed, &c.indexed), "a query only ranks");
        assert_ne!(a.version, c.version);
        assert!(
            cache.as_ref().unwrap().motion.still,
            "the first load lands still"
        );

        let mut edited = (*lib).clone();
        let mut copy = edited.snippets[0].clone();
        copy.id = Uuid::from_u128(7);
        edited.snippets.insert(0, copy);
        let edited = Arc::new(edited);
        let d = refresh(
            &mut cache,
            &edited,
            &LibraryFilter::All,
            "sig",
            Sort::Name,
            now(),
        );
        assert!(!Rc::ptr_eq(&c.indexed, &d.indexed));
        let motion = &cache.as_ref().unwrap().motion;
        assert!(!motion.still && motion.fresh.contains(&Uuid::from_u128(7)));

        let mut bulk = (*edited).clone();
        bulk.snippets.truncate(2);
        let bulk = Arc::new(bulk);
        refresh(
            &mut cache,
            &bulk,
            &LibraryFilter::All,
            "",
            Sort::Name,
            now(),
        );
        let motion = &cache.as_ref().unwrap().motion;
        assert!(
            motion.still && motion.fresh.is_empty(),
            "17 rows left: bulk"
        );
    }
}
