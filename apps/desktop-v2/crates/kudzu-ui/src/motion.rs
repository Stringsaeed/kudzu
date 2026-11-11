//! Motion the widgets and screens share, from `components/app/editor/motion.tsx`,
//! `components/app/clipboard/motion.ts` and `hooks/use-changes.ts`: the rejected-input
//! [`Shake`], [`Land`], the change census ([`Changed`], [`Arrivals`], [`census`]) and the
//! presence transitions.
//!
//! RUNE-GAP(#567): Rune's keyframe sequences (`Beat`) animate opacity and uniform scale only, so
//! the sequences that translate (shake) or land from a scale on an element already on screen
//! are run as timed targets here: the state keeps the step, its poll advances it, and the
//! element springs or tweens to each step's target. Delete [`Shake`] and [`Land`] once
//! `Beat` takes a translation, and play `theme::motion::SHAKE` as a sequence instead.
//!
//! Animations play on real changes only: never on first load, and never again when a virtual
//! row scrolls back into view. Reduce Motion: the callers skip `start` when
//! `Cx::is_reduced_motion` is on, as `nudge` and `land` return early in the Tauri app.

use std::collections::HashSet;
use std::hash::Hash;

use rune::{Curve, Easing, El, Spring, SpringPair, Transition, Tx};

use crate::theme::motion::{self as tokens, EasingScope, SHAKE};

/// Seconds from the start of a keyframe run to each keyframe: the offsets mapped through the
/// easing when it applies to the whole run (a Web Animations `easing` option), else linearly.
fn keyframe_times(k: &tokens::Keyframes) -> Vec<f32> {
    k.poses
        .iter()
        .map(|(at, _)| match k.scope {
            EasingScope::PerSegment => at * k.duration,
            EasingScope::Whole => invert(k.easing, *at) * k.duration,
        })
        .collect()
}

/// The time at which a monotonic easing reaches `progress` (bisection; easings here are
/// monotonic from 0 to 1).
fn invert(e: Easing, progress: f32) -> f32 {
    if progress <= 0.0 || progress >= 1.0 {
        return progress.clamp(0.0, 1.0);
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if e.eval(mid) < progress {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// A run of timed targets: `start` stamps it, `poll` moves to the step whose time has come and
/// asks for a wake at the next one.
#[derive(Clone, Debug, Default, PartialEq)]
struct Steps {
    started: Option<f64>,
    step: usize,
}

impl Steps {
    /// Stamps the run at `clock`; the deadline of the next step.
    fn start_at(&mut self, times: &[f32], clock: f64) -> Option<f64> {
        self.started = Some(clock);
        self.step = 0;
        times.get(1).map(|next| clock + *next as f64)
    }

    /// Moves to the step `clock` has reached: whether the step changed (the view reads it),
    /// and the next deadline.
    fn advance(&mut self, times: &[f32], clock: f64) -> (bool, Option<f64>) {
        let Some(t0) = self.started else {
            return (false, None);
        };
        let elapsed = (clock - t0) as f32;
        let reached = times
            .iter()
            .rposition(|t| elapsed + 1e-4 >= *t)
            .unwrap_or(0);
        if reached + 1 >= times.len() {
            self.started = None;
            self.step = 0;
            return (true, None);
        }
        let changed = reached != self.step;
        self.step = reached;
        (changed, Some(t0 + times[reached + 1] as f64))
    }

    fn start(&mut self, times: &[f32], tx: &mut Tx) {
        if let Some(due) = self.start_at(times, tx.clock()) {
            tx.wake_at(due);
        }
    }

    fn poll(&mut self, times: &[f32], clock: f64, tx: &mut Tx) -> bool {
        let (changed, due) = self.advance(times, clock);
        if let Some(due) = due {
            tx.wake_at(due);
        }
        changed
    }

    fn running(&self) -> bool {
        self.started.is_some()
    }
}

/// The editor's "that input was rejected" shake (`nudge`): 0, −3, +3, −2, +1, 0 pt sideways
/// over `SLOW`, eased out over the whole run. Keep one in the state of the field it shakes,
/// call [`Shake::start`] from the handler that rejects the input and [`Shake::poll`] from the
/// window's poll, and pass the field through [`Shake::apply`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shake {
    steps: Steps,
}

impl Shake {
    pub fn start(&mut self, tx: &mut Tx) {
        self.steps.start(&keyframe_times(&SHAKE), tx);
    }

    /// True when the view must rebuild to move to the next target.
    pub fn poll(&mut self, clock: f64, tx: &mut Tx) -> bool {
        self.steps.poll(&keyframe_times(&SHAKE), clock, tx)
    }

    pub fn running(&self) -> bool {
        self.steps.running()
    }

    /// The target of the current step and the time to reach it: the next keyframe's pose,
    /// tweened linearly over the segment, as CSS interpolates between keyframes.
    pub fn target(&self) -> (f32, f32) {
        if !self.running() {
            return (0.0, tokens::FAST);
        }
        let times = keyframe_times(&SHAKE);
        let i = (self.steps.step + 1).min(SHAKE.poses.len() - 1);
        (SHAKE.poses[i].1.translate[0], times[i] - times[i - 1])
    }

    pub fn apply(&self, e: El) -> El {
        let (x, seconds) = self.target();
        e.offset(x, 0.0)
            .offset_motion(Curve::tween(seconds, Easing::Linear))
    }
}

/// `land`: from `scale` back to 1 on the pop spring; with `fade` it fades in too (something
/// arrived), without it reads as "here it is" (pointing at something already on screen, as a
/// duplicate trigger chip does at 1.12).
#[derive(Clone, Debug, PartialEq)]
pub struct Land {
    steps: Steps,
    pub scale: f32,
    pub fade: bool,
}

impl Default for Land {
    fn default() -> Self {
        Land {
            steps: Steps::default(),
            scale: 0.85,
            fade: false,
        }
    }
}

/// Step 0 lands the start pose at once; one frame later the element springs to rest.
const LAND_TIMES: [f32; 3] = [0.0, 1.0 / 120.0, 1.0 / 120.0 + tokens::SLOW];

impl Land {
    pub fn new(scale: f32, fade: bool) -> Land {
        Land {
            scale,
            fade,
            ..Land::default()
        }
    }
    pub fn start(&mut self, tx: &mut Tx) {
        self.steps.start(&LAND_TIMES, tx);
    }
    pub fn poll(&mut self, clock: f64, tx: &mut Tx) -> bool {
        self.steps.poll(&LAND_TIMES, clock, tx)
    }
    pub fn running(&self) -> bool {
        self.steps.running()
    }
    pub fn apply(&self, e: El) -> El {
        if self.running() && self.steps.step == 0 {
            let e = e.scale(self.scale).scale_motion(Spring::NONE);
            return if self.fade {
                e.opacity(0.0).opacity_motion(Spring::NONE)
            } else {
                e
            };
        }
        e.scale(1.0)
            .scale_motion(tokens::POP)
            .opacity(1.0)
            .opacity_motion(tokens::POP)
    }
}

/// `useChanged`: true once `value` has changed while `key` stayed the same (this entry got
/// pinned while it was on screen). Moving to another key resets it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changed<K, V> {
    seen: Option<(K, V)>,
    changed: bool,
}

impl<K: PartialEq, V: PartialEq> Changed<K, V> {
    pub fn observe(&mut self, key: K, value: V) -> bool {
        match &self.seen {
            Some((k, v)) if *k == key => {
                if *v != value {
                    self.changed = true;
                    self.seen = Some((key, value));
                }
            }
            _ => {
                self.changed = false;
                self.seen = Some((key, value));
            }
        }
        self.changed
    }
}

/// Changes closer together than this are a burst (a held arrow key), where replaying an
/// entrance would strobe (`breadcrumbs.tsx`).
pub const BURST: f64 = 0.300;

/// `useArrivals` / `useChanges`: counts arrivals at a new id, skipping the first one (mount)
/// and bursts. Key the arriving element on the count to replay its entrance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arrivals {
    last: Option<(String, f64)>,
    count: u32,
}

impl Arrivals {
    pub fn observe(&mut self, id: &str, now: f64) -> u32 {
        match &self.last {
            // Mount stamps now, as `breadcrumbs.tsx` stamps the page's start (`at: 0`): an
            // arrival right after mount is a burst too.
            None => self.last = Some((id.to_owned(), now)),
            Some((last, at)) if last != id => {
                let burst = now - at < BURST;
                self.last = Some((id.to_owned(), now));
                if !burst {
                    self.count += 1;
                }
            }
            _ => {}
        }
        self.count
    }
    pub fn count(&self) -> u32 {
        self.count
    }
}

/// One row as the census sees it: `stamp` changes when the row is new content (copied
/// again, restored), `place` when it moves for another reason (pinned).
#[derive(Clone, Debug, PartialEq)]
pub struct Seen<K> {
    pub id: K,
    pub stamp: i64,
    pub place: i64,
}

/// More changes than this at once is a load or a bulk action: no motion, just the new state.
pub const BULK: usize = 10;
/// How long rows may glide after a change before the list is still again.
pub const SETTLE: f32 = 0.400;

/// What changed between two renders of a list (`useListChanges`).
#[derive(Clone, Debug, PartialEq)]
pub struct Changes<K: Eq + Hash> {
    /// New, copied again or restored.
    pub fresh: HashSet<K>,
    /// Moved for another reason, plus removed.
    pub moved: usize,
}

/// The change census: `None` when nothing changed or the change is bulk.
pub fn census<K: Eq + Hash + Clone>(before: &[Seen<K>], after: &[Seen<K>]) -> Option<Changes<K>> {
    let mut old: std::collections::HashMap<&K, &Seen<K>> =
        before.iter().map(|s| (&s.id, s)).collect();
    let (mut fresh, mut moved) = (HashSet::new(), 0);
    for s in after {
        match old.remove(&s.id) {
            None => {
                fresh.insert(s.id.clone());
            }
            Some(b) if b.stamp != s.stamp => {
                fresh.insert(s.id.clone());
            }
            Some(b) if b.place != s.place => moved += 1,
            Some(_) => {}
        }
    }
    moved += old.len();
    let total = fresh.len() + moved;
    (total > 0 && total <= BULK).then_some(Changes { fresh, moved })
}

/// `animate-rise`: 4 pt up and in, on the soft spring.
pub fn rise() -> Transition {
    Transition::offset(0.0, 4.0)
        .combined(Transition::opacity())
        .springs(SpringPair::new(
            tokens::SOFT,
            tokens::soft_spring(tokens::MODERATE),
        ))
}

/// `animate-arrive`: a new row drops in from 6 pt above.
pub fn arrive() -> Transition {
    Transition::offset(0.0, -6.0)
        .combined(Transition::opacity())
        .springs(SpringPair::new(
            tokens::SOFT,
            tokens::soft_spring(tokens::MODERATE),
        ))
}

/// `Presence` with `animate-check` in and `animate-vanish` out: a small mark (a star, a badge)
/// turning on or off. Rune plays an element's transition only when it mounts or unmounts
/// after the first frame, so state that was already true stays still, as `Presence` does.
pub fn check_in() -> Transition {
    Transition::scale(0.4)
        .combined(Transition::opacity())
        .allow_small_scale()
        .springs(SpringPair::new(
            tokens::POP,
            tokens::soft_spring(tokens::MODERATE),
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake_keyframes_run_eased_out_over_the_whole_shake() {
        let t = keyframe_times(&SHAKE);
        assert_eq!(t.len(), 6);
        assert!(t[0] == 0.0 && (t[5] - tokens::SLOW).abs() < 1e-4);
        // Eased out as a whole: early keyframes come quickly, later ones slow down.
        let gaps: Vec<f32> = t.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.windows(2).all(|g| g[1] > g[0]), "{gaps:?}");
        assert!((Easing::EASE_OUT.eval(t[1] / tokens::SLOW) - 0.2).abs() < 1e-3);
    }

    #[test]
    fn shake_steps_through_its_targets_and_rests() {
        let mut s = Shake::default();
        assert_eq!(s.target().0, 0.0);
        let times = keyframe_times(&SHAKE);
        s.steps.start_at(&times, 1.0);
        assert_eq!(s.target().0, -3.0);
        let t = keyframe_times(&SHAKE);
        let mut seen = vec![s.target().0];
        for at in &t[1..] {
            assert!(s.steps.advance(&times, 1.0 + *at as f64).0);
            seen.push(s.target().0);
        }
        assert_eq!(seen, vec![-3.0, 3.0, -2.0, 1.0, 0.0, 0.0]);
        assert!(!s.running());
    }

    #[test]
    fn land_starts_from_its_scale_then_springs_back() {
        let mut l = Land::new(1.12, false);
        assert_eq!(
            l.steps.start_at(&LAND_TIMES, 0.0),
            Some(LAND_TIMES[1] as f64)
        );
        assert!(l.running());
        assert!(l.steps.advance(&LAND_TIMES, LAND_TIMES[1] as f64).0);
        assert!(l.running());
        assert!(l.steps.advance(&LAND_TIMES, LAND_TIMES[2] as f64).0);
        assert!(!l.running());
    }

    #[test]
    fn changed_counts_changes_under_one_key_only() {
        let mut c = Changed::default();
        assert!(!c.observe("a", false));
        assert!(!c.observe("a", false));
        assert!(c.observe("a", true));
        assert!(c.observe("a", true), "stays changed while the key stays");
        assert!(!c.observe("b", true), "another entry is not a change");
    }

    #[test]
    fn arrivals_skip_mount_and_bursts() {
        let mut a = Arrivals::default();
        assert_eq!(a.observe("work", 0.0), 0);
        assert_eq!(a.observe("work", 1.0), 0);
        assert_eq!(a.observe("home", 2.0), 1);
        assert_eq!(a.observe("mail", 2.1), 1, "a burst");
        assert_eq!(a.observe("docs", 3.0), 2);
    }

    fn seen(id: &str, stamp: i64, place: i64) -> Seen<String> {
        Seen {
            id: id.into(),
            stamp,
            place,
        }
    }

    #[test]
    fn census_finds_fresh_and_moved_rows() {
        let before = vec![seen("a", 1, 0), seen("b", 1, 0), seen("c", 1, 0)];
        let after = vec![seen("n", 5, 0), seen("a", 2, 0), seen("b", 1, 1)];
        let ch = census(&before, &after).unwrap();
        assert_eq!(ch.fresh, ["n", "a"].iter().map(|s| s.to_string()).collect());
        assert_eq!(ch.moved, 2, "b pinned, c removed");
        assert_eq!(census(&before, &before), None);
    }

    #[test]
    fn census_ignores_bulk_changes() {
        let before: Vec<_> = (0..20).map(|i| seen(&i.to_string(), 0, 0)).collect();
        let after: Vec<_> = (0..20).map(|i| seen(&i.to_string(), 1, 0)).collect();
        assert_eq!(census(&before, &after), None);
    }
}
