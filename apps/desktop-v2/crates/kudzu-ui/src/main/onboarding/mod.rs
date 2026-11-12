//! First-run setup. Domain transitions live in the application Onboarding machine.
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
    pub(crate) scene_finish_error: bool,
    pub(crate) opening: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub practice: TextEdit,
    pub(crate) task: Option<Task<()>>,
    pub(crate) detecting: bool,
}

impl State {
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
    let revision = s.svc.onboarding.watch().revision();
    if s.onboarding.seen == Some(revision) {
        return changed;
    }
    s.onboarding.seen = Some(revision);
    true
}
