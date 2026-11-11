//! One allocator per composition/router lifetime. Retire/stop the old Engine before allocating
//! a namespace for a new Engine. Re-starting the same Engine keeps its namespace.
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct EngineInstances(AtomicU64);
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineInstance(u64);
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormToken {
    instance: EngineInstance,
    native: u64,
}
impl EngineInstances {
    pub fn next_engine(&self) -> EngineInstance {
        let previous = self
            .0
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .expect("engine instance namespace exhausted");
        EngineInstance(previous + 1)
    }
    /// S40 fences all callbacks from a retired forwarding host, not only form requests.
    pub fn is_current(&self, instance: EngineInstance) -> bool {
        instance.0 != 0 && self.0.load(Ordering::SeqCst) == instance.0
    }
}
impl EngineInstance {
    /// S35 qualifies every native form request before it crosses EngineEvents.
    pub fn form_token(self, native: u64) -> FormToken {
        FormToken {
            instance: self,
            native,
        }
    }
    /// Check before forwarding submit/cancel/focus to this Engine. A token for a retired
    /// Engine must never become a cancel of the new Engine's identically numbered form.
    pub fn native_token(self, token: FormToken) -> Option<u64> {
        (token.instance == self && token.is_valid()).then_some(token.native)
    }
}
impl FormToken {
    pub fn instance(self) -> EngineInstance {
        self.instance
    }
    pub(crate) fn is_valid(self) -> bool {
        self.instance.0 != 0 && self.native != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retiring_an_engine_fences_its_callbacks_and_native_token_mapping() {
        let instances = EngineInstances::default();
        let first = instances.next_engine();
        assert!(instances.is_current(first));
        let first_token = first.form_token(1);
        let second = instances.next_engine();
        assert!(!instances.is_current(first));
        assert!(instances.is_current(second));
        assert_eq!(second.native_token(first_token), None);
        assert_eq!(second.native_token(second.form_token(1)), Some(1));
    }
}
