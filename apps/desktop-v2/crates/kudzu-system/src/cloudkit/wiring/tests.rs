use super::*;
use crate::cloudkit::native;
use kudzu_app::ports::CloudSend;
use kudzu_core::sync::{RecordKind, RemoteRecord};
use std::sync::Condvar;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Received {
    Fetched(CloudBatch),
    Sent(CloudSend),
    State(Vec<u8>),
    Account(AccountChange),
    Availability(CloudAvailability),
    Failed(PortError),
    ZoneDeleted,
}

#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<(u64, Received)>>,
    changed: Condvar,
}

impl Recorder {
    fn push(&self, generation: u64, event: Received) {
        self.events.lock().unwrap().push((generation, event));
        self.changed.notify_all();
    }
}

impl CloudEvents for Recorder {
    fn fetched(&self, generation: u64, batch: CloudBatch) {
        self.push(generation, Received::Fetched(batch));
    }
    fn sent(&self, generation: u64, report: CloudSend) {
        self.push(generation, Received::Sent(report));
    }
    fn state(&self, generation: u64, bytes: Vec<u8>) {
        self.push(generation, Received::State(bytes));
    }
    fn account_changed(&self, generation: u64, change: AccountChange) {
        self.push(generation, Received::Account(change));
    }
    fn availability(&self, generation: u64, availability: CloudAvailability) {
        self.push(generation, Received::Availability(availability));
    }
    fn failed(&self, generation: u64, error: PortError) {
        self.push(generation, Received::Failed(error));
    }
    fn zone_deleted(&self, generation: u64) {
        self.push(generation, Received::ZoneDeleted);
    }
}

#[test]
fn a_cold_stop_is_idempotent_and_requests_without_a_session_are_refused() {
    let cloud: Arc<dyn CloudStore> = Arc::new(CloudKit::new());
    cloud.stop();
    cloud.stop();
    assert_eq!(cloud.fetch(1), Err(PortError::Cancelled));
    assert_eq!(
        cloud.send(1, CloudBatch::default()),
        Err(PortError::Cancelled)
    );
}

#[test]
fn port_receives_typed_events_and_nonempty_state_in_order_with_the_start_generation() {
    let received = Arc::new(Recorder::default());
    let target = received.clone();
    let session = Session::new(23, move |generation, event| {
        forward(target.as_ref(), generation, event);
    });
    let record = RemoteRecord {
        kind: RecordKind::Meta,
        name: "meta".into(),
        payload: "{ \"syncVersion\": 1, \"future\": true }".into(),
    };
    let batch = CloudBatch {
        records: vec![record.clone()],
        deleted: vec!["snippet-deleted".into()],
        full_fetch: true,
    };
    let report = CloudSend {
        saved: vec![record.clone()],
        conflicts: vec![record],
        requeue: vec!["meta".into()],
        zone_missing: true,
        ..Default::default()
    };
    session.delivery.emit(Event::Fetched(batch.clone()));
    session.delivery.emit(Event::State(vec![0, 255, 1]));
    session.delivery.emit(Event::State(Vec::new()));
    session.delivery.emit(Event::Sent(report.clone()));
    session.delivery.emit(Event::State(vec![2, 0, 254]));
    session
        .delivery
        .emit(Event::Failed(PortError::PermissionDenied));
    session.delivery.emit(Event::ZoneDeleted);
    session.delivery.stop();
    session.delivery.emit(Event::State(vec![3]));
    session.delivery.emit(Event::Fetched(CloudBatch::default()));
    assert_eq!(
        *received.events.lock().unwrap(),
        vec![
            (23, Received::Fetched(batch)),
            (23, Received::State(vec![0, 255, 1])),
            (23, Received::Sent(report)),
            (23, Received::State(vec![2, 0, 254])),
            (23, Received::Failed(PortError::PermissionDenied)),
            (23, Received::ZoneDeleted),
        ]
    );
}

#[test]
fn every_known_account_transition_maps_without_inventing_a_transition() {
    let received = Recorder::default();
    for change in [
        CKSyncEngineAccountChangeType::SignIn,
        CKSyncEngineAccountChangeType::SignOut,
        CKSyncEngineAccountChangeType::SwitchAccounts,
        CKSyncEngineAccountChangeType(100),
    ] {
        forward(&received, 8, Event::Account(change));
    }
    assert_eq!(
        *received.events.lock().unwrap(),
        vec![
            (8, Received::Account(AccountChange::SignIn)),
            (8, Received::Account(AccountChange::SignOut)),
            (8, Received::Account(AccountChange::Switch)),
        ]
    );
}

#[test]
fn indeterminate_availability_is_forwarded_for_the_service_to_preserve_status() {
    let received = Recorder::default();
    for status in [
        CKAccountStatus::Available,
        CKAccountStatus::NoAccount,
        CKAccountStatus::Restricted,
        CKAccountStatus::TemporarilyUnavailable,
        CKAccountStatus::CouldNotDetermine,
    ] {
        forward(&received, 19, Event::Availability(status));
    }
    assert_eq!(
        *received.events.lock().unwrap(),
        vec![
            (19, Received::Availability(CloudAvailability::Available)),
            (19, Received::Availability(CloudAvailability::NoAccount)),
            (19, Received::Availability(CloudAvailability::Restricted)),
            (
                19,
                Received::Availability(CloudAvailability::TemporarilyUnavailable)
            ),
            (
                19,
                Received::Availability(CloudAvailability::CouldNotDetermine)
            ),
        ]
    );
}

#[test]
#[ignore = "S71: requires a signed production-identity test executable, provisioning profile and an iCloud test account; may create the Kudzu zone"]
fn signed_engine_emits_nonempty_secure_state_then_stops() {
    let cloud: Arc<dyn CloudStore> = Arc::new(CloudKit::new());
    let received = Arc::new(Recorder::default());
    cloud.start(41, None, received.clone()).unwrap();
    cloud.fetch(41).unwrap();
    // Flush the initial zone save. No snippet records are created by this test.
    cloud.send(41, CloudBatch::default()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut log = received.events.lock().unwrap();
    while !log
        .iter()
        .any(|(_, event)| matches!(event, Received::State(_)))
    {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        log = received.changed.wait_timeout(log, remaining).unwrap().0;
    }
    let archive = log.iter().find_map(|(generation, event)| {
        assert_eq!(*generation, 41);
        if let Received::State(bytes) = event {
            Some(bytes.clone())
        } else {
            None
        }
    });
    drop(log);
    cloud.stop();
    let archive = archive.expect("no serialized state received from the signed engine");
    assert!(!archive.is_empty());
    assert!(native::decode_state(&archive).is_ok());
    let log = received.events.lock().unwrap();
    let count = log.len();
    let (log, _) = received
        .changed
        .wait_timeout(log, Duration::from_millis(200))
        .unwrap();
    assert_eq!(log.len(), count, "event delivered after stop");
}
