//! What the app catalogue, target-app and file-icon ports promise, for the cases that change
//! nothing on the host: no activation, no selection copy (that presses ⌘C).
use crate::ports::{AppCatalog, FileIcons, TargetApp};

pub const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

pub trait AppsHarness {
    fn catalog(&self) -> &dyn AppCatalog;
    fn target(&self) -> &dyn TargetApp;
    fn file_icons(&self) -> &dyn FileIcons;
    /// A filesystem path that exists and has an icon.
    fn existing_path(&self) -> String;
}

pub fn catalog(h: &dyn AppsHarness) {
    let apps = h.catalog().installed().unwrap();
    let mut ids: Vec<&str> = apps.iter().map(|a| a.bundle_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), apps.len(), "one entry per bundle id");
    assert!(apps
        .iter()
        .all(|a| !a.bundle_id.is_empty() && !a.name.is_empty()));
    assert!(
        apps.iter().all(|a| a.pid.is_none()),
        "installed, not running"
    );
    let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
    assert!(names.windows(2).all(|w| w[0] <= w[1]), "sorted by name");
    if let Some(app) = apps.first() {
        let png = h.catalog().icon(&app.bundle_id, 32).unwrap().expect("icon");
        assert!(png.starts_with(PNG_MAGIC));
    }
    assert_eq!(
        h.catalog().icon("invalid.kudzu.contract.none", 32),
        Ok(None)
    );
}

pub fn file_icons(h: &dyn AppsHarness) {
    let png = h
        .file_icons()
        .file_icon(&h.existing_path(), 64)
        .unwrap()
        .expect("icon for an existing path");
    assert!(png.starts_with(PNG_MAGIC));
    assert_eq!(
        h.file_icons()
            .file_icon("/kudzu/contract/definitely/missing", 64),
        Ok(None)
    );
}

/// Read-only queries: they answer without failing; absence is a valid answer.
pub fn target_reads(h: &dyn AppsHarness) {
    if let Some(app) = h.target().frontmost().unwrap() {
        assert!(!app.bundle_id.is_empty() || !app.name.is_empty());
    }
    h.target().caret().unwrap();
}
