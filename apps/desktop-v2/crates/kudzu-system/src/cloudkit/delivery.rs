//! Callback admission and a stop barrier; no lock is held while calling the sink.
use std::sync::{Condvar, Mutex, PoisonError};

pub(super) struct Delivery<T> {
    generation: u64,
    sink: Box<dyn Fn(u64, T) + Send + Sync>,
    admission: Mutex<Admission>,
    drained: Condvar,
}

struct Admission {
    active: bool,
    count: usize,
}

pub(super) struct Lease<'a, T>(&'a Delivery<T>);

impl<T> Delivery<T> {
    pub(super) fn new(generation: u64, sink: impl Fn(u64, T) + Send + Sync + 'static) -> Self {
        Self {
            generation,
            sink: Box::new(sink),
            admission: Mutex::new(Admission {
                active: true,
                count: 0,
            }),
            drained: Condvar::new(),
        }
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn is_active(&self) -> bool {
        self.admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
    }

    pub(super) fn enter(&self) -> Option<Lease<'_, T>> {
        let mut admission = self
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !admission.active {
            return None;
        }
        admission.count += 1;
        Some(Lease(self))
    }

    #[cfg(test)]
    pub(super) fn emit(&self, event: T) {
        if let Some(lease) = self.enter() {
            lease.emit(event);
        }
    }

    pub(super) fn stop(&self) {
        let mut admission = self
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        admission.active = false;
        drop(
            self.drained
                .wait_while(admission, |a| a.count != 0)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

impl<T> Lease<'_, T> {
    pub(super) fn emit(&self, event: T) {
        (self.0.sink)(self.0.generation, event);
    }
}

impl<T> Drop for Lease<'_, T> {
    fn drop(&mut self) {
        let mut admission = self
            .0
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        admission.count -= 1;
        if admission.count == 0 {
            self.0.drained.notify_all();
        }
    }
}
