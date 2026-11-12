//! Which rows move: only the ones that changed (a copy, a re-copy, a restore), and only when a
//! few changed. More than ten at once is a load or a bulk action and stays still; so does
//! anything that happens while the window is hidden (`motion.ts`, FEATURES §3).

use std::collections::{HashMap, HashSet};

use kudzu_core::clipboard::History;
use uuid::Uuid;

/// More changes than this at once is a load or a bulk action: no motion, just the new state.
pub const BULK: usize = 10;
/// How long rows may glide to their new places after a change.
pub const SETTLE_SECS: f64 = 0.4;

#[derive(Debug, Default)]
pub struct Census {
    fresh: HashSet<Uuid>,
    settle_until: Option<f64>,
}

impl Census {
    /// Compares two snapshots at `now` (seconds on the UI clock). `visible` is whether the
    /// window shows the list; a hidden list never animates. True when anything it reports
    /// changed.
    pub fn observe(&mut self, before: &History, after: &History, visible: bool, now: f64) -> bool {
        let had = !self.fresh.is_empty() || self.settle_until.is_some();
        self.fresh.clear();
        self.settle_until = None;
        if visible {
            let old: HashMap<Uuid, (_, bool)> = before
                .entries
                .iter()
                .map(|e| (e.id, (e.copied_at, e.pinned)))
                .collect();
            let mut fresh = HashSet::new();
            let mut moved = 0;
            for e in &after.entries {
                match old.get(&e.id) {
                    None => {
                        fresh.insert(e.id);
                    }
                    Some((at, _)) if *at != e.copied_at => {
                        fresh.insert(e.id);
                    }
                    Some((_, pinned)) if *pinned != e.pinned => moved += 1,
                    Some(_) => {}
                }
            }
            moved += old.keys().filter(|id| after.get(**id).is_none()).count();
            let total = fresh.len() + moved;
            if (1..=BULK).contains(&total) {
                self.fresh = fresh;
                self.settle_until = Some(now + SETTLE_SECS);
            }
        }
        had || self.settle_until.is_some()
    }

    /// Row `id` just arrived: it drops in once.
    pub fn is_fresh(&self, id: Uuid) -> bool {
        self.fresh.contains(&id)
    }

    /// Rows may glide to their new places.
    pub fn settling(&self, now: f64) -> bool {
        self.settle_until.is_some_and(|until| now < until)
    }

    /// Ends the settle window once it is over. True when it ended.
    pub fn expire(&mut self, now: f64) -> bool {
        if self.settle_until.is_some_and(|until| now >= until) {
            self.settle_until = None;
            self.fresh.clear();
            return true;
        }
        false
    }

    /// When to wake to end the settle window.
    pub fn next_due(&self) -> Option<f64> {
        self.settle_until
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::{clip, history};

    #[test]
    fn a_new_copy_arrives_once_and_the_list_settles() {
        let a = clip("a", 5, false);
        let before = history(vec![a.clone()]);
        let b = clip("b", 1, false);
        let after = history(vec![b.clone(), a]);
        let mut census = Census::default();
        assert!(census.observe(&before, &after, true, 10.0));
        assert!(census.is_fresh(b.id));
        assert!(census.settling(10.2));
        assert!(!census.settling(10.5));
        assert!(census.expire(10.5));
        assert!(!census.is_fresh(b.id));
    }

    #[test]
    fn a_recopy_arrives_but_a_pin_only_moves() {
        let mut a = clip("a", 5, false);
        let before = history(vec![a.clone()]);
        a.pinned = true;
        let mut census = Census::default();
        assert!(census.observe(&before, &history(vec![a.clone()]), true, 0.0));
        assert!(!census.is_fresh(a.id));
        assert!(census.settling(0.1), "a pin glides rows into place");
        let mut again = a.clone();
        again.copied_at += chrono::Duration::minutes(5);
        census.observe(&history(vec![a]), &history(vec![again.clone()]), true, 1.0);
        assert!(census.is_fresh(again.id));
    }

    #[test]
    fn more_than_ten_changes_stay_still() {
        let before = history(vec![]);
        let after = history((0..11).map(|i| clip(&format!("c{i}"), i, false)).collect());
        let mut census = Census::default();
        census.observe(&before, &after, true, 0.0);
        assert!(!census.settling(0.1));
        let ten = history((0..10).map(|i| clip(&format!("c{i}"), i, false)).collect());
        census.observe(&before, &ten, true, 0.0);
        assert!(census.settling(0.1), "ten is the most that still animates");
    }

    #[test]
    fn nothing_arrives_while_the_window_is_hidden() {
        let after = history(vec![clip("a", 1, false)]);
        let mut census = Census::default();
        census.observe(&history(vec![]), &after, false, 0.0);
        assert!(!census.is_fresh(after.entries[0].id));
        assert!(!census.settling(0.1));
    }

    #[test]
    fn an_unchanged_history_reports_no_change() {
        let h = history(vec![clip("a", 1, false)]);
        assert!(!Census::default().observe(&h, &h, true, 0.0));
    }
}
