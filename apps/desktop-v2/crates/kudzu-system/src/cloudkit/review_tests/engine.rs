use super::super::engine::{Completion, SyncEngine};
use objc2::rc::Retained;
use objc2_cloud_kit::{CKErrorCode, CKErrorDomain, CKSyncEnginePendingRecordZoneChange};
use objc2_foundation::NSError;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

type OnRequest = Box<dyn Fn(Option<&Completion>)>;

#[derive(Default)]
pub(super) struct Engine {
    pub(super) pending: RefCell<BTreeMap<String, Retained<CKSyncEnginePendingRecordZoneChange>>>,
    pub(super) sends: Cell<usize>,
    pub(super) fetches: Cell<usize>,
    pub(super) zones: Cell<usize>,
    pub(super) on_send: Option<OnRequest>,
    pub(super) on_fetch: Option<OnRequest>,
}

impl SyncEngine for Engine {
    fn pending(&self) -> Vec<Retained<CKSyncEnginePendingRecordZoneChange>> {
        self.pending.borrow().values().cloned().collect()
    }
    fn add(&self, changes: &[Retained<CKSyncEnginePendingRecordZoneChange>]) {
        for change in changes {
            let name = unsafe { change.recordID().recordName() }.to_string();
            self.pending.borrow_mut().insert(name, change.clone());
        }
    }
    fn remove(&self, changes: &[Retained<CKSyncEnginePendingRecordZoneChange>]) {
        for change in changes {
            self.pending
                .borrow_mut()
                .remove(&unsafe { change.recordID().recordName() }.to_string());
        }
    }
    fn save_zone(&self) {
        self.zones.set(self.zones.get() + 1);
    }
    fn fetch(&self, completion: Option<&Completion>) {
        self.fetches.set(self.fetches.get() + 1);
        if let Some(callback) = &self.on_fetch {
            callback(completion);
        }
    }
    fn send(&self, completion: Option<&Completion>) {
        self.sends.set(self.sends.get() + 1);
        if let Some(callback) = &self.on_send {
            callback(completion);
        }
    }
}

pub(super) fn error(code: CKErrorCode) -> Retained<NSError> {
    unsafe { NSError::errorWithDomain_code_userInfo(CKErrorDomain, code.0, None) }
}
