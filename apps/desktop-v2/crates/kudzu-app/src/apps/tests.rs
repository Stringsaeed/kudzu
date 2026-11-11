use super::*;
use crate::error::PortError;
use crate::fixtures::apps::FakeApps;
use crate::ports::AppCatalog;
use kudzu_core::system::AppInfo;
use std::sync::{mpsc, Arc};
use std::time::Duration;

fn mail() -> AppInfo {
    AppInfo {
        bundle_id: "com.apple.mail".into(),
        name: "Mail".into(),
        pid: None,
        path: Some("/Applications/Mail.app".into()),
    }
}

fn wait_icon(task: Task<Option<Vec<u8>>>) -> Option<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    let ready = task.ready();
    let _sub = ready.subscribe(move || {
        let _ = tx.send(());
    });
    if !*task.ready().get() {
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    task.take().unwrap().unwrap()
}

#[test]
fn enumerates_once_and_keeps_the_cached_list() {
    let fake = Arc::new(FakeApps::default());
    *fake.installed.lock().unwrap() = vec![mail()];
    let apps = Apps::new(fake.clone());
    assert_eq!(*fake.installed_calls.lock().unwrap(), 1);
    assert_eq!(apps.installed().get().as_slice(), [mail()]);
    fake.installed.lock().unwrap().clear();
    fake.installed.lock().unwrap().push(AppInfo {
        bundle_id: "com.apple.Safari".into(),
        name: "Safari".into(),
        pid: None,
        path: None,
    });
    assert_eq!(apps.installed().get().as_slice(), [mail()]);
    assert_eq!(*fake.installed_calls.lock().unwrap(), 1);
    assert_eq!(fake.installed().unwrap().len(), 1);
    assert_eq!(*fake.installed_calls.lock().unwrap(), 2);
}

#[test]
fn name_of_a_missing_app_is_the_bundle_id() {
    let fake = Arc::new(FakeApps::default());
    *fake.installed.lock().unwrap() = vec![mail()];
    let apps = Apps::new(fake);
    assert_eq!(apps.name_of("com.apple.mail"), "Mail");
    assert_eq!(apps.name_of("com.missing.app"), "com.missing.app");
}

#[test]
fn failed_enumeration_is_an_empty_catalogue() {
    let fake = Arc::new(FakeApps::default());
    *fake.failure.lock().unwrap() = Some(PortError::Unavailable);
    *fake.installed.lock().unwrap() = vec![mail()];
    let apps = Apps::new(fake);
    assert!(apps.installed().get().is_empty());
    assert_eq!(apps.name_of("com.apple.mail"), "com.apple.mail");
}

#[test]
fn icons_are_fetched_once_per_bundle_and_size() {
    let fake = Arc::new(FakeApps::default());
    *fake.installed.lock().unwrap() = vec![mail()];
    fake.icons
        .lock()
        .unwrap()
        .insert("com.apple.mail".into(), b"png".to_vec());
    let apps = Apps::new(fake.clone());
    let first = wait_icon(apps.icon("com.apple.mail", 32).unwrap());
    let second = wait_icon(apps.icon("com.apple.mail", 32).unwrap());
    assert_eq!(first.as_deref(), Some(b"png".as_slice()));
    assert_eq!(second, first);
    assert_eq!(
        *fake.icon_calls.lock().unwrap(),
        [("com.apple.mail".into(), 32)]
    );
    let _ = wait_icon(apps.icon("com.apple.mail", 64).unwrap());
    assert_eq!(fake.icon_calls.lock().unwrap().len(), 2);
    assert!(wait_icon(apps.icon("com.missing.app", 32).unwrap()).is_none());
}

#[test]
fn catalogue_is_usable_without_target_app() {
    let catalog: Arc<dyn AppCatalog> = Arc::new(FakeApps::default());
    let apps = Apps::new(catalog);
    assert!(apps.installed().get().is_empty());
}
