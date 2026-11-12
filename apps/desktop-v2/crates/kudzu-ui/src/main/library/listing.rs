//! The derived list, cached by its inputs: the library snapshot (by pointer: every revision is
//! a new `Arc`), the view, the filter's text and the sort. The folded [`Indexed`] snapshot is
//! kept across queries, so a keystroke only ranks. The cache holds the snapshot's `Arc`, so a
//! pointer can't be reused while it is compared.
//!
//! The change census (`snippet-list.tsx` `ListMotion`): when a new snapshot adds or removes
//! between one and [`MAX_MOVING`] snippets, the added rows rise in and the rest glide, for
//! [`SETTLE_SECS`]; then the list is still again, so a later sort, filter or remount lands at
//! once. A change that adds and removes nothing (a star, an edit) and a bigger one (an import,
//! a sync) land still. Rune never replays a row's entrance when the virtual list remounts it.

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
/// How long rows may rise and glide after a change: a little longer than `--motion-slow`, so
/// the glide is never cut short (`snippet-list.tsx`, 320 ms).
pub const SETTLE_SECS: f64 = 0.32;

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
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// Rows that arrived: they rise in.
    pub fresh: HashSet<Uuid>,
    /// Nothing is moving: everything lands at once.
    pub still: bool,
    /// When the motion window ends (UI clock seconds).
    until: Option<f64>,
}

impl Default for Motion {
    fn default() -> Self {
        Self::still()
    }
}

impl Motion {
    fn still() -> Self {
        Motion {
            fresh: HashSet::new(),
            still: true,
            until: None,
        }
    }

    /// Ends the motion window once `now` reaches it. True when it ended.
    pub(super) fn expire(&mut self, now: f64) -> bool {
        if self.until.is_some_and(|until| now >= until) {
            *self = Motion::still();
            return true;
        }
        false
    }

    /// When to wake to end the motion window.
    pub(super) fn due(&self) -> Option<f64> {
        self.until
    }
}

pub(super) struct Cache {
    listing: Rc<Listing>,
    pub(super) motion: Motion,
    next_version: u64,
}

/// What a new snapshot at `now` moves (`fresh.size + gone.size` in `1..=MAX_MOVING`).
fn census(before: &KudzuLibrary, after: &KudzuLibrary, now: f64) -> Motion {
    let old: HashSet<Uuid> = before.snippets.iter().map(|s| s.id).collect();
    let new: HashSet<Uuid> = after.snippets.iter().map(|s| s.id).collect();
    let fresh: HashSet<Uuid> = new.difference(&old).copied().collect();
    let gone = old.difference(&new).count();
    if !(1..=MAX_MOVING).contains(&(fresh.len() + gone)) {
        return Motion::still();
    }
    Motion {
        fresh,
        still: false,
        until: Some(now + SETTLE_SECS),
    }
}

/// The listing for these inputs, from `cache` when nothing changed. `clock` is the UI clock,
/// against which the motion window ends; `now` is the wall clock ranking uses.
pub(super) fn refresh(
    cache: &mut Option<Cache>,
    library: &Arc<KudzuLibrary>,
    scope: &LibraryFilter,
    query: &str,
    sort: Sort,
    now: DateTime<Utc>,
    clock: f64,
) -> Rc<Listing> {
    if let Some(c) = cache.as_mut() {
        c.motion.expire(clock);
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
            census(&c.listing.indexed.library, library, clock),
            c.next_version,
        ),
        None => (
            Rc::new(Indexed::new(Arc::clone(library))),
            Motion::still(),
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

    fn at(cache: &mut Option<Cache>, lib: &Arc<KudzuLibrary>, q: &str, clock: f64) -> Rc<Listing> {
        refresh(cache, lib, &LibraryFilter::All, q, Sort::Name, now(), clock)
    }

    fn motion(cache: &Option<Cache>) -> &Motion {
        &cache.as_ref().unwrap().motion
    }

    #[test]
    fn the_cache_keeps_the_snapshot_and_bumps_the_version_on_change() {
        let lib = Arc::new(SampleState::load().unwrap().library);
        let mut cache = None;
        let a = at(&mut cache, &lib, "", 0.0);
        let b = at(&mut cache, &lib, "", 0.0);
        assert!(Rc::ptr_eq(&a, &b), "same inputs, same listing");
        let c = at(&mut cache, &lib, "sig", 0.0);
        assert!(Rc::ptr_eq(&a.indexed, &c.indexed), "a query only ranks");
        assert_ne!(a.version, c.version);
        assert!(motion(&cache).still, "the first load lands still");

        let mut edited = (*lib).clone();
        let mut copy = edited.snippets[0].clone();
        copy.id = Uuid::from_u128(7);
        edited.snippets.insert(0, copy);
        let edited = Arc::new(edited);
        let d = at(&mut cache, &edited, "sig", 0.0);
        assert!(!Rc::ptr_eq(&c.indexed, &d.indexed));
        assert!(!motion(&cache).still && motion(&cache).fresh.contains(&Uuid::from_u128(7)));

        let mut bulk = (*edited).clone();
        bulk.snippets.truncate(2);
        at(&mut cache, &Arc::new(bulk), "", 0.0);
        assert!(
            motion(&cache).still && motion(&cache).fresh.is_empty(),
            "17 rows left: bulk"
        );
    }

    /// S50 review M1: an edit that adds and removes nothing lands still, and a small change
    /// moves only until its window ends, also across a later query on the same snapshot.
    #[test]
    fn motion_needs_an_add_or_remove_and_ends_with_its_window() {
        let lib = Arc::new(SampleState::load().unwrap().library);
        let mut cache = None;
        at(&mut cache, &lib, "", 10.0);
        let mut starred = (*lib).clone();
        starred.snippets[0].is_favorite = !starred.snippets[0].is_favorite;
        at(&mut cache, &Arc::new(starred.clone()), "", 10.0);
        assert!(motion(&cache).still, "a star adds and removes nothing");

        let mut added = starred;
        let mut copy = added.snippets[0].clone();
        copy.id = Uuid::from_u128(9);
        added.snippets.insert(0, copy);
        let added = Arc::new(added);
        at(&mut cache, &added, "", 11.0);
        assert!(!motion(&cache).still && motion(&cache).fresh.contains(&Uuid::from_u128(9)));
        assert_eq!(motion(&cache).due(), Some(11.0 + SETTLE_SECS));
        at(&mut cache, &added, "sig", 11.1);
        assert!(!motion(&cache).still, "still inside the window");
        at(&mut cache, &added, "", 11.0 + SETTLE_SECS);
        assert_eq!(motion(&cache), &Motion::still(), "the window ended");
    }
}
