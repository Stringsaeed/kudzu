use super::*;
use crate::{fixtures::clock::FakeClock, persist::Writer};
use std::{fs, path::PathBuf, sync::Arc, time::Duration};

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-settings-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("settings.json")
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn writer() -> Writer {
    Writer::new(
        Arc::new(FakeClock::default()),
        Duration::from_secs(30),
        Duration::from_secs(1),
    )
    .unwrap()
}

// Ported from the reference store: acceptance must survive a durable close/reopen.
#[test]
fn dev_first_run_is_safe_and_preserves_later_choices() {
    let folder = Folder::new();
    let writes = writer();
    let settings = SettingsService::open(folder.path(), Variant::Dev, &writes).unwrap();
    assert!(settings.watch().get().paused);
    assert_ne!(
        settings.watch().get().palette_hotkey,
        Settings::default().palette_hotkey
    );
    let revision = settings
        .update(|s| {
            s.paused = false;
            s.palette_hotkey = "Control+Alt+K".into();
        })
        .unwrap();
    assert!(settings.saved().get().durable < revision);
    writes.shutdown().unwrap();
    assert_eq!(settings.saved().get().durable, revision);
    let writes = writer();
    let again = SettingsService::open(folder.path(), Variant::Dev, &writes).unwrap();
    assert!(!again.watch().get().paused);
    assert_eq!(again.watch().get().palette_hotkey, "Control+Alt+K");
    writes.shutdown().unwrap();
}

#[test]
fn missing_settings_take_the_variant_defaults() {
    let folder = Folder::new();
    fs::write(
        folder.path(),
        r#"{"paused":false,"paletteHotkey":"Control+Alt+K"}"#,
    )
    .unwrap();
    let writes = writer();
    let dev = SettingsService::open(folder.path(), Variant::Dev, &writes).unwrap();
    assert_eq!(dev.watch().get().clipboard_hotkey, "Control+Alt+Shift+V");
    assert_eq!(dev.watch().get().palette_hotkey, "Control+Alt+K");
    assert!(!dev.watch().get().paused);
    writes.shutdown().unwrap();

    fs::write(folder.path(), r#"{"paused":true}"#).unwrap();
    let writes = writer();
    let prod = SettingsService::open(folder.path(), Variant::Production, &writes).unwrap();
    assert_eq!(prod.watch().get().clipboard_hotkey, "Control+Alt+V");
    assert_eq!(prod.watch().get().clipboard_retention_days, 7);
    assert!(prod.watch().get().paused);
    writes.shutdown().unwrap();
}

#[test]
fn validated_setters_and_update_reject_atomically_and_no_ops_do_not_publish() {
    let folder = Folder::new();
    let writes = writer();
    let settings = SettingsService::open(folder.path(), Variant::Production, &writes).unwrap();
    let (before, initial) = settings.snapshot();
    assert_eq!(settings.update(|_| {}).unwrap(), before);
    assert_eq!(settings.watch().revision(), Revision(1));
    assert_eq!(
        settings.set_trigger_prefix("😀😀x".into()),
        Err(SettingsError::TriggerPrefix)
    );
    assert_eq!(
        settings.set_clipboard_retention_days(3651),
        Err(SettingsError::Retention)
    );
    assert_eq!(
        settings.update(|s| {
            s.paused = true;
            s.default_trigger_prefix = "12345".into();
        }),
        Err(SettingsError::TriggerPrefix)
    );
    assert_eq!(settings.snapshot(), (before, initial));
    settings.set_trigger_prefix("😀😀".into()).unwrap();
    settings.set_trigger_prefix(String::new()).unwrap();
    for days in [0, 1, 3650] {
        settings.set_clipboard_retention_days(days).unwrap();
        assert_eq!(settings.watch().get().clipboard_retention_days, days);
    }
    assert_eq!(settings.watch().revision(), Revision(6));
    writes.shutdown().unwrap();
}

#[test]
fn every_default_and_persisted_key_matches_the_reference_settings() {
    let value = serde_json::to_value(Settings::default()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "typingSpeedWPM": 40, "defaultTriggerPrefix": ";", "paletteHotkey": "Control+Alt+Space",
            "paused": false, "defaultInsertion": "automatic",
            "deniedBundleIDs": ["com.1password.1password", "com.agilebits.onepassword-osx", "com.agilebits.onepassword7", "com.apple.Passwords", "com.apple.keychainaccess", "com.apple.loginwindow", "com.bitwarden.desktop"],
            "appInsertions": {"com.apple.Terminal":"paste", "com.googlecode.iterm2":"paste", "dev.warp.Warp-Stable":"paste", "com.mitchellh.ghostty":"paste", "org.alacritty":"paste", "net.kovidgoyal.kitty":"paste", "com.github.wez.wezterm":"paste"},
            "launchAtLogin": false, "onboardingCompleted": false, "theme": "dark", "showInDock": true,
            "autoCheckUpdates": true, "updateChannel": "stable", "icloudSync": true, "lastRunVersion": null,
            "clipboardEnabled": true, "clipboardRetentionDays": 7, "clipboardHotkey": "Control+Alt+V",
            "clipboardIgnoredApps": ["com.1password.1password", "com.agilebits.onepassword7", "com.agilebits.onepassword-osx", "com.bitwarden.desktop", "com.apple.keychainaccess", "com.apple.Passwords", "org.keepassxc.keepassxc", "com.dashlane.dashlanephonefinal", "com.lastpass.LastPass", "in.sinew.Enpass-Desktop", "com.nordsec.nordpass", "me.proton.pass.electron"],
            "clipboardPastePlain": false, "clipboardShowSecrets": false,
            "agentAccess": false, "agentWrite": true, "agentClipboard": true
        })
    );
    let mut dev = value;
    dev["paused"] = true.into();
    dev["paletteHotkey"] = "Control+Alt+Shift+Space".into();
    dev["clipboardHotkey"] = "Control+Alt+Shift+V".into();
    assert_eq!(
        serde_json::to_value(Settings::for_variant(Variant::Dev)).unwrap(),
        dev
    );
    assert_eq!(
        Settings::default().matching_config(),
        kudzu_core::matching::MatchingConfig::default()
    );
}

#[test]
fn corrupt_settings_are_quarantined_before_variant_defaults_are_saved() {
    for content in [r#"{"theme":"PRIVATE-BODY"}"#, "[]", "invalid JSON"] {
        let folder = Folder::new();
        fs::write(folder.path(), content).unwrap();
        let writes = writer();
        let settings = SettingsService::open(folder.path(), Variant::Dev, &writes).unwrap();
        assert!(settings.watch().get().paused);
        assert_eq!(settings.watch().get().theme, Theme::Dark);
        writes.shutdown().unwrap();
        let recovered = fs::read_dir(&folder.0)
            .unwrap()
            .map(Result::unwrap)
            .find(|p| {
                p.file_name()
                    .to_string_lossy()
                    .starts_with("settings-corrupt-")
            })
            .unwrap();
        assert_eq!(fs::read_to_string(recovered.path()).unwrap(), content);
    }
}

#[test]
fn panic_rolls_back_settings_and_notifications_run_after_unlock() {
    let folder = Folder::new();
    let writes = writer();
    let settings = Arc::new(SettingsService::open(folder.path(), Variant::Dev, &writes).unwrap());
    let before = settings.snapshot();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| settings.update(|s| {
            s.agent_access = true;
            panic!("edit failed");
        })))
        .is_err()
    );
    assert_eq!(settings.snapshot(), before);
    let reader = settings.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let _sub = settings.watch().subscribe(move || {
        tx.send(reader.update(|_| {}).unwrap()).unwrap();
    });
    let worker = settings.clone();
    let join = std::thread::spawn(move || worker.update(|s| s.agent_write = false).unwrap());
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        Revision(2)
    );
    join.join().unwrap();
    assert!(!settings.watch().get().agent_access);
    assert!(!settings.watch().get().agent_write);
    assert!(settings.watch().get().agent_clipboard);
    writes.shutdown().unwrap();
    let before = settings.snapshot();
    assert_eq!(
        settings.update(|s| s.paused = false),
        Err(SettingsError::Persistence(PersistError::Stopped))
    );
    assert_eq!(settings.snapshot(), before);
}

#[test]
fn opening_out_of_range_settings_repairs_only_invalid_fields_and_allows_later_edits() {
    for (prefix, days, expected_prefix, expected_days, dirty) in [
        ("ab😀x", 4000, "ab😀", 3650, true),
        ("abc😀", 7, "abc", 7, true),
        ("😀😀", 0, "😀😀", 0, false),
    ] {
        let folder = Folder::new();
        fs::write(
            folder.path(),
            serde_json::to_vec(&serde_json::json!({
                "defaultTriggerPrefix": prefix,
                "clipboardRetentionDays": days,
                "paused": true,
                "theme": "light"
            }))
            .unwrap(),
        )
        .unwrap();
        let writes = writer();
        let settings = SettingsService::open(folder.path(), Variant::Production, &writes).unwrap();
        let current = settings.watch().get();
        assert_eq!(current.default_trigger_prefix, expected_prefix);
        assert_eq!(current.clipboard_retention_days, expected_days);
        assert!(current.paused);
        assert_eq!(current.theme, Theme::Light);
        assert_eq!(
            settings.saved().get().durable,
            if dirty { Revision(0) } else { Revision(1) }
        );
        let revision = settings
            .update(|s| s.show_in_dock = false)
            .expect("a bad stored field blocked an unrelated edit");
        writes.shutdown().unwrap();
        assert_eq!(settings.saved().get().durable, revision);
        let writes = writer();
        let reopened = SettingsService::open(folder.path(), Variant::Production, &writes).unwrap();
        assert_eq!(
            reopened.watch().get().default_trigger_prefix,
            expected_prefix
        );
        assert_eq!(
            reopened.watch().get().clipboard_retention_days,
            expected_days
        );
        assert!(!reopened.watch().get().show_in_dock);
        assert_eq!(reopened.saved().get().durable, Revision(1));
        writes.shutdown().unwrap();
        assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 1);
    }
}
