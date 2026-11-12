//! First-run setup. Domain transitions live in the application Onboarding machine.
mod access;
mod actions;
mod brand;
pub mod scenes;
mod steps;
mod view;

use kudzu_app::tasks::Task;
use rune::TextEdit;
pub use view::view;

#[derive(Default)]
pub struct State {
    pub(super) brand: Option<brand::Brand>,
    seen: Option<kudzu_app::watch::Revision>,
    seen_engine: Option<kudzu_app::watch::Revision>,
    pub(crate) permission_opening: bool,
    pub(crate) permission_error: Option<String>,
    pub(crate) scene_static: bool,
    arrival_started: Option<f64>,
    arrival_y: f32,
    reduced_motion: std::cell::Cell<bool>,
    permission_ack: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) scene_finish_error: bool,
    pub(crate) opening: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub practice: TextEdit,
    pub(crate) task: Option<Task<()>>,
    task_subscription: Option<kudzu_app::watch::Subscription>,
    pub(crate) detecting: bool,
}

impl State {
    pub fn set_reduced_motion(&self, reduced: bool) {
        self.reduced_motion.set(reduced);
    }
    pub fn arrival_y(&self) -> f32 {
        if self.scene_static || self.reduced_motion.get() {
            0.0
        } else if self.arrival_started.is_none() {
            4.0
        } else {
            self.arrival_y
        }
    }

    pub fn permission_finished(&mut self, result: Result<(), String>) {
        self.permission_opening = false;
        self.permission_error = result.err();
    }
    pub fn opening(&self) -> bool {
        self.opening.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn is_composing(&self) -> bool {
        self.practice.is_composing()
    }
    pub fn is_editing_text(&self) -> bool {
        self.practice.focused
    }
    pub fn blur_inputs(&mut self) {
        self.practice.focused = false;
    }
    pub fn substitute(&mut self) {
        if !self.practice.is_composing() && self.practice.text.to_lowercase().ends_with(";kudzu") {
            let end = self.practice.text.len() - 6;
            let value = format!("{}Hello from Kudzu!", &self.practice.text[..end]);
            self.practice = TextEdit::new(value);
            self.practice.focused = true;
        }
    }
    pub fn success(&self) -> bool {
        self.practice.text.contains("Hello from Kudzu!")
    }
}

/// Watches carry domain state; only revision stamps are cached in the screen.
pub fn poll(s: &mut crate::shell::MainState) -> bool {
    let mut changed = false;
    if matches!(s.frame.view, crate::main::MainView::Onboarding) {
        brand::ensure(s);
        changed = s.onboarding.brand.as_mut().is_some_and(brand::Brand::poll);
    }
    if let Some(result) = s.onboarding.task.as_ref().and_then(Task::take) {
        s.onboarding.task = None;
        s.onboarding.task_subscription = None;
        changed = true;
        if result.is_err() && s.onboarding.opening() {
            s.onboarding
                .opening
                .store(false, std::sync::atomic::Ordering::Release);
            s.svc.onboarding.fail_finish();
        }
    }
    let engine_revision = s.svc.expansion.status().revision();
    if s.onboarding.seen_engine != Some(engine_revision) {
        s.onboarding.seen_engine = Some(engine_revision);
        changed = true;
    }
    let revision = s.svc.onboarding.watch().revision();
    if s.onboarding.seen == Some(revision) {
        return changed;
    }
    s.onboarding.seen = Some(revision);
    true
}

/// Observe the queued companion request after its window has processed the handoff.
pub fn poll_frame(s: &mut crate::shell::MainState, tx: &mut rune::Tx) -> bool {
    use std::sync::atomic::Ordering;
    let changed = poll_arrival(s, tx);
    if !s.onboarding.permission_opening {
        return changed;
    }
    let handle = &s.shell.handles().permissions;
    if s.onboarding.permission_ack.load(Ordering::Acquire) {
        if let Some(error) = handle.window_error() {
            s.onboarding.permission_finished(Err(error));
            return true;
        }
        if handle.is_visible() {
            s.onboarding.permission_finished(Ok(()));
            return true;
        }
    }
    tx.wake_at(tx.clock() + 0.025);
    changed
}

// Rune skips presence transitions on the first frame. Drive the initial rise explicitly.
fn poll_arrival(s: &mut crate::shell::MainState, tx: &mut rune::Tx) -> bool {
    if !matches!(s.frame.view, crate::main::MainView::Onboarding)
        || s.svc.onboarding.snapshot().direction != 0
    {
        return false;
    }
    let before = s.onboarding.arrival_y();
    let start = *s.onboarding.arrival_started.get_or_insert(tx.clock());
    let progress = ((tx.clock() - start) / 0.24).clamp(0.0, 1.0) as f32;
    s.onboarding.arrival_y = if progress >= 1.0 {
        0.0
    } else {
        4.0 * (1.0 - crate::theme::motion::SPRING_SOFT.eval(progress))
    };
    if progress < 1.0 && !s.onboarding.scene_static && !s.onboarding.reduced_motion.get() {
        tx.wake_at(tx.clock() + 1.0 / 120.0);
    }
    before != s.onboarding.arrival_y()
}

fn track_task(s: &mut crate::shell::MainState) {
    if let Some(task) = &s.onboarding.task {
        let watch = task.ready();
        let handle = s.shell.handles().main.clone();
        let waker = handle.clone();
        s.onboarding.task_subscription = Some(watch.subscribe(move || waker.wake()));
        if *watch.get() {
            handle.wake();
        }
    }
}
