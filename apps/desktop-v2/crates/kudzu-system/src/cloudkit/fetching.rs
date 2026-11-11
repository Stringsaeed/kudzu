//! Terminal fetch results. Record batches and state events are never buffered here.
use super::{errors, native::Event};
use kudzu_app::ports::CloudBatch;
use objc2_cloud_kit::CKErrorCode;

#[derive(Default)]
pub(super) struct Fetching {
    requested: bool,
    request_failed_in_delegate: bool,
    cycle_active: bool,
    cycle_failed: bool,
}

impl Fetching {
    /// A concurrent call shares the outstanding native fetch and its terminal result.
    pub(super) fn request(&mut self) -> bool {
        if self.requested {
            return false;
        }
        self.requested = true;
        self.request_failed_in_delegate = self.cycle_active && self.cycle_failed;
        true
    }

    pub(super) fn began(&mut self) {
        self.cycle_active = true;
        self.cycle_failed = false;
    }

    pub(super) fn zone_failed(&mut self) {
        self.cycle_failed = true;
        if self.requested {
            self.request_failed_in_delegate = true;
        }
    }

    pub(super) fn delegate_finished(&mut self) -> Option<Event> {
        self.cycle_active = false;
        // Explicit fetches wait for their completion result. DidFetchChanges also
        // arrives after failure; it alone must not mark a first fetch complete.
        (!self.requested && !self.cycle_failed).then(full_fetch)
    }

    pub(super) fn completed(&mut self, error: Option<CKErrorCode>) -> Option<Event> {
        if !std::mem::take(&mut self.requested) {
            return None;
        }
        if std::mem::take(&mut self.request_failed_in_delegate) {
            return None;
        }
        match error {
            None => Some(full_fetch()),
            Some(code) if errors::transient(code) || code == CKErrorCode::ZoneNotFound => None,
            Some(code) => Some(Event::Failed(errors::port(code))),
        }
    }
}

fn full_fetch() -> Event {
    Event::Fetched(CloudBatch {
        full_fetch: true,
        ..Default::default()
    })
}
