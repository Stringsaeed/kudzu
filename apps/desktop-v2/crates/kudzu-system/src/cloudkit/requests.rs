//! Issue engine commands without changing application policy.
use super::{engine::SyncEngine, errors, native::Session, sending};
use block2::RcBlock;
use kudzu_app::ports::CloudBatch;
use objc2::rc::autoreleasepool;
use objc2_foundation::NSError;
use std::sync::{Arc, PoisonError};

pub(super) fn start(engine: &impl SyncEngine, fresh: bool, availability: impl FnOnce()) {
    autoreleasepool(|_| {
        // The archive has IDs, not current payloads. Only the service can submit fresh
        // record intents after its initial-fetch/format gate. Database changes remain.
        engine.remove(&engine.pending());
        if fresh {
            engine.save_zone();
        }
        availability();
    })
}

pub(super) fn fetch(engine: &impl SyncEngine, session: &Arc<Session>) {
    autoreleasepool(|_| {
        if !session
            .fetching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .request()
        {
            return;
        }
        let session = session.clone();
        let completion = RcBlock::new(move |error: *mut NSError| {
            autoreleasepool(|_| {
                let Some(lease) = session.delivery.enter() else {
                    return;
                };
                let result = session
                    .fetching
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .completed(unsafe { error.as_ref() }.map(errors::code));
                if let Some(event) = result {
                    lease.emit(event);
                }
            })
        });
        engine.fetch(Some(&completion));
    })
}

pub(super) fn send(engine: &impl SyncEngine, session: &Arc<Session>, batch: CloudBatch) {
    autoreleasepool(|_| {
        let save_zone = {
            let mut state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.pending.upsert(batch);
            std::mem::take(&mut state.zone_save_needed)
        };
        if save_zone {
            engine.save_zone();
        }
        sending::queue_pending(engine, session);
        // Per-record/database delegate events are the sole send results. A completion
        // error can summarize those same conflicts and must not become another failure.
        engine.send(None);
    })
}
