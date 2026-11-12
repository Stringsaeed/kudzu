//! Import and export use the same file-dialog calls as onboarding, with the menu's
//! generic titles from Tauri's `runImport` and `runExport("json")`. Paths stay out of logs.

use std::time::{Duration, Instant};

use kudzu_app::settings::Theme;
use rune::{FileDialogOptions, Tx};

use crate::shell::MainState;

pub fn import_snippets(_s: &mut MainState, tx: &mut Tx) {
    let options = FileDialogOptions {
        title: "Import snippets".into(),
        ..FileDialogOptions::default()
    }
    .filter(
        "Snippet files",
        &["json", "yml", "yaml", "textexpander", "csv"],
    );
    tx.open_file(options, |s: &mut MainState, result, _| {
        let Ok(Some(path)) = result else {
            return;
        };
        let Ok(task) = s.svc.data.import(path) else {
            return;
        };
        let handle = s.shell.handles().main.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while task.take().is_none() {
                if Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            handle.update(|state, _| state.refresh_library_snapshot());
            handle.wake();
        });
    });
}

pub fn export_library(s: &mut MainState, tx: &mut Tx) {
    let options = FileDialogOptions {
        title: "Export library".into(),
        file_name: Some(s.svc.data.export_json_name()),
        ..FileDialogOptions::default()
    }
    .filter("Kudzu library", &["json"]);
    tx.save_file(options, |s: &mut MainState, result, _| {
        let Ok(Some(path)) = result else {
            return;
        };
        // Dropping the task does not cancel the write.
        let _ = s.svc.data.export_json(path);
    });
}

pub fn toggle_appearance(s: &mut MainState) {
    let _ = s.svc.settings.update(|settings| {
        settings.theme = if settings.theme == Theme::Dark {
            Theme::Light
        } else {
            Theme::Dark
        };
    });
}

pub fn toggle_clipboard(s: &mut MainState) {
    let _ = s.svc.settings.update(|settings| {
        settings.clipboard_enabled = !settings.clipboard_enabled;
    });
}
