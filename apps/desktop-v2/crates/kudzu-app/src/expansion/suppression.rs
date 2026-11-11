use crate::ports::ExpansionEngine;
use std::sync::{Arc, Mutex, PoisonError};

pub(super) struct Suppression {
    count: Mutex<usize>,
    engine: Arc<dyn ExpansionEngine>,
}

impl Suppression {
    pub(super) fn new(engine: Arc<dyn ExpansionEngine>) -> Arc<Self> {
        Arc::new(Self {
            count: Mutex::new(0),
            engine,
        })
    }

    pub(super) fn resync(&self) {
        let count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        self.engine.set_suppressed(*count != 0);
    }
    pub(super) fn acquire(self: &Arc<Self>) -> SuppressionGuard {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        if *count == 0 {
            self.engine.set_suppressed(true);
        }
        *count += 1;
        SuppressionGuard(self.clone())
    }
}

/// Matching is suppressed until the last owner releases its guard. A hidden window must
/// explicitly drop its session; hiding a Rune window does not drop its state.
#[must_use]
pub struct SuppressionGuard(Arc<Suppression>);

impl Drop for SuppressionGuard {
    fn drop(&mut self) {
        let mut count = self.0.count.lock().unwrap_or_else(PoisonError::into_inner);
        *count -= 1;
        if *count == 0 {
            self.0.engine.set_suppressed(false);
        }
    }
}
