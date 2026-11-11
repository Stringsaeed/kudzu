//! The small CKSyncEngine command surface used by the adapter; replaceable offline.
use block2::DynBlock;
use objc2::rc::Retained;
use objc2_cloud_kit::{CKSyncEngine, CKSyncEnginePendingRecordZoneChange};
use objc2_foundation::{NSArray, NSError};

pub(super) type Completion = DynBlock<dyn Fn(*mut NSError)>;

pub(super) trait SyncEngine {
    fn pending(&self) -> Vec<Retained<CKSyncEnginePendingRecordZoneChange>>;
    fn add(&self, changes: &[Retained<CKSyncEnginePendingRecordZoneChange>]);
    fn remove(&self, changes: &[Retained<CKSyncEnginePendingRecordZoneChange>]);
    fn save_zone(&self);
    fn fetch(&self, completion: Option<&Completion>);
    fn send(&self, completion: Option<&Completion>);
}

impl SyncEngine for CKSyncEngine {
    fn pending(&self) -> Vec<Retained<CKSyncEnginePendingRecordZoneChange>> {
        unsafe { self.state().pendingRecordZoneChanges() }
            .iter()
            .collect()
    }
    fn add(&self, changes: &[Retained<CKSyncEnginePendingRecordZoneChange>]) {
        unsafe {
            self.state()
                .addPendingRecordZoneChanges(&NSArray::from_retained_slice(changes));
        }
    }
    fn remove(&self, changes: &[Retained<CKSyncEnginePendingRecordZoneChange>]) {
        unsafe {
            self.state()
                .removePendingRecordZoneChanges(&NSArray::from_retained_slice(changes));
        }
    }
    fn save_zone(&self) {
        super::native::queue_zone(self);
    }
    fn fetch(&self, completion: Option<&Completion>) {
        unsafe {
            self.fetchChangesWithCompletionHandler(completion);
        }
    }
    fn send(&self, completion: Option<&Completion>) {
        unsafe {
            self.sendChangesWithCompletionHandler(completion);
        }
    }
}
