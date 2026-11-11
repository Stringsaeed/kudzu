//! Read-only checks against the real system: nothing is activated and no selection is copied.
use super::*;
use kudzu_app::fixtures::contract::apps::{self as contract, AppsHarness};

struct Real;
impl AppsHarness for Real {
    fn catalog(&self) -> &dyn AppCatalog {
        &SystemApps
    }
    fn target(&self) -> &dyn TargetApp {
        &SystemApps
    }
    fn file_icons(&self) -> &dyn FileIcons {
        &SystemApps
    }
    fn existing_path(&self) -> String {
        "/System/Applications".into()
    }
}

#[test]
fn system_apps_pass_the_apps_contract() {
    if installed_apps().is_empty() {
        return; // Not a regular macOS install (CI sandbox).
    }
    contract::catalog(&Real);
    contract::file_icons(&Real);
    contract::target_reads(&Real);
}

#[test]
fn finds_system_apps_sorted_and_unique() {
    let apps = installed_apps();
    if apps.is_empty() {
        return; // Not a regular macOS install (CI sandbox).
    }
    let mut ids: Vec<&str> = apps.iter().map(|a| a.bundle_id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), total);
    let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
    assert!(names.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn icon_is_png() {
    let Some(app) = installed_apps().into_iter().find(|a| a.path.is_some()) else {
        return;
    };
    let png = icon_png(app.path.as_deref().unwrap(), 32).expect("icon");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert!(icon_png("/definitely/not/here.app", 32).is_none());
}

#[test]
fn icons_by_bundle_id_resolve_through_launch_services() {
    let Some(app) = installed_apps()
        .into_iter()
        .find(|a| a.bundle_id == "com.apple.finder" || a.path.is_some())
    else {
        return;
    };
    let png = SystemApps.icon(&app.bundle_id, 64).unwrap().expect("icon");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(SystemApps.icon("", 64), Ok(None));
    assert_eq!(SystemApps.icon(&app.bundle_id, 0), Ok(None));
    assert_eq!(SystemApps.icon(&app.bundle_id, MAX_ICON_PX + 1), Ok(None));
}

#[test]
fn activating_an_app_that_is_not_running_is_target_gone() {
    let gone = AppInfo {
        bundle_id: "invalid.kudzu.s35.not-running".into(),
        name: "Gone".into(),
        pid: None,
        path: None,
    };
    assert_eq!(SystemApps.activate(&gone), Err(PortError::TargetGone));
}
