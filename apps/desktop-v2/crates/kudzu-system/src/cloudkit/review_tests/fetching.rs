use super::{engine, requests, Event, Session};
use crate::cloudkit::{delegate, engine::Completion};
use objc2::rc::Retained;
use objc2_cloud_kit::CKErrorCode;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[test]
fn coalesced_fetch_calls_share_one_successful_terminal_batch() {
    let completions = Rc::new(RefCell::new(Vec::new()));
    let capture = completions.clone();
    let engine = engine::Engine {
        on_fetch: Some(Box::new(move |completion: Option<&Completion>| {
            capture.borrow_mut().push(completion.unwrap().copy())
        })),
        ..Default::default()
    };
    let received = Arc::new(Mutex::new(Vec::new()));
    let output = received.clone();
    let session = Arc::new(Session::new(1, move |_, event| {
        output.lock().unwrap().push(event)
    }));
    requests::fetch(&engine, &session);
    requests::fetch(&engine, &session);
    assert_eq!(engine.fetches.get(), 1);
    assert!(
        delegate::did_fetch(&session).is_none(),
        "completion confirms the delegate's terminal marker"
    );
    completions.borrow()[0].call((std::ptr::null_mut(),));
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert!(
        matches!(&received[0], Event::Fetched(batch) if batch.full_fetch && batch.records.is_empty())
    );
}

#[test]
fn permanent_zone_fetch_failure_is_reported_once_despite_completion_overlap() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let output = received.clone();
    let session = Arc::new(Session::new(2, move |_, event| {
        output.lock().unwrap().push(event)
    }));
    let delegate_session = session.clone();
    let engine = engine::Engine {
        on_fetch: Some(Box::new(move |completion| {
            let event = delegate::zone_fetch_result(
                &delegate_session,
                Some(CKErrorCode::InvalidArguments),
                || panic!("not a missing zone"),
            );
            delegate_session.delivery.emit(event.unwrap());
            assert!(delegate::did_fetch(&delegate_session).is_none());
            let error = engine::error(CKErrorCode::PartialFailure);
            completion
                .unwrap()
                .call((Retained::as_ptr(&error).cast_mut(),));
        })),
        ..Default::default()
    };
    requests::fetch(&engine, &session);
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert!(matches!(
        &received[0],
        Event::Failed(kudzu_app::error::PortError::InvalidData)
    ));
}

#[test]
fn a_transient_failure_before_any_delegate_event_is_silent_and_allows_retry() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let output = received.clone();
    let session = Arc::new(Session::new(3, move |_, event| {
        output.lock().unwrap().push(event)
    }));
    let engine = engine::Engine {
        on_fetch: Some(Box::new(move |completion| {
            let error = engine::error(CKErrorCode::NetworkUnavailable);
            completion
                .unwrap()
                .call((Retained::as_ptr(&error).cast_mut(),));
        })),
        ..Default::default()
    };
    requests::fetch(&engine, &session);
    requests::fetch(&engine, &session);
    assert_eq!(engine.fetches.get(), 2);
    assert!(received.lock().unwrap().is_empty());
}

#[test]
fn automatic_fetch_only_finishes_without_a_zone_failure_and_zone_deletion_is_separate() {
    let session = Session::new(4, |_, _| {});
    session.fetching.lock().unwrap().began();
    assert!(delegate::zone_fetch_result(&session, Some(CKErrorCode::ZoneBusy), || {}).is_none());
    assert!(delegate::did_fetch(&session).is_none());
    session.fetching.lock().unwrap().began();
    assert!(delegate::zone_fetch_result(&session, None, || {}).is_none());
    assert!(
        matches!(delegate::did_fetch(&session), Some(Event::Fetched(batch)) if batch.full_fetch)
    );
    assert!(matches!(session.zone_deleted(|| {}), Event::ZoneDeleted));
}
