//! Main-thread regression tests for `GuidePanel` bundle validation (`S66` review).

use std::path::{Path, PathBuf};

use kudzu_appkit::guide_panel::GuidePanel;
use objc2_foundation::MainThreadMarker;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-guide-panel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn guide_panel_rejects_stemless_and_missing_app_bundles(mtm: MainThreadMarker) {
    let scratch = Scratch::new();

    assert!(
        GuidePanel::new(mtm, Path::new("/tmp/.app")).is_err(),
        "stemless '/tmp/.app' must be rejected before creating an NSPanel"
    );

    let missing = scratch.0.join("Missing.app");
    assert!(
        GuidePanel::new(mtm, &missing).is_err(),
        "nonexistent '.app' directory must be rejected before creating an NSPanel"
    );

    let regular_file = scratch.0.join("NotADirectory.app");
    std::fs::write(&regular_file, b"not a bundle").unwrap();
    assert!(
        GuidePanel::new(mtm, &regular_file).is_err(),
        "regular file ending in '.app' must be rejected"
    );

    let valid = scratch.0.join("Kudzu.app");
    std::fs::create_dir_all(&valid).unwrap();
    let panel = GuidePanel::new(mtm, &valid).expect("existing '.app' directory should initialize");
    assert_eq!(panel.window_number(), None);
}

fn main() {
    let mtm =
        MainThreadMarker::new().expect("harness = false runs main() on the macOS main thread");
    guide_panel_rejects_stemless_and_missing_app_bundles(mtm);
}
