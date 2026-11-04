//! On-disk state: the library (Kudzu JSON), settings, form memory and this device's id.
//! Every write goes to a temp file first and is renamed into place, so a crash never leaves a
//! half-written library.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use kudzu_core::interchange::backup::{BackupReason, BackupStore};
use kudzu_core::interchange::{coordinator, kudzu_json};
use kudzu_core::template::FormMemory;
use kudzu_core::{KudzuLibrary, UsageRecord};
use uuid::Uuid;

use crate::settings::Settings;

pub struct Store {
    pub dir: PathBuf,
    pub library: KudzuLibrary,
    pub settings: Settings,
    pub form_memory: FormMemory,
    pub device_id: String,
    /// True the very first time Kudzu runs on this machine.
    pub first_run: bool,
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)
}

/// Earlier builds used the identifier `sh.saeed.kudzu.desktop`. On the first launch under the
/// release identifier, copy that data across (copy, not move, so a downgrade still works).
pub fn migrate_legacy_dir(dir: &Path) {
    const LEGACY: &str = "sh.saeed.kudzu.desktop";
    if dir.join("library.json").exists() {
        return;
    }
    let Some(legacy) = dir.parent().map(|p| p.join(LEGACY)) else { return };
    if legacy == dir || !legacy.join("library.json").exists() {
        return;
    }
    if let Err(e) = copy_dir(&legacy, dir) {
        log::error!("couldn't migrate data from {}: {e}", legacy.display());
    } else {
        log::info!("migrated data from {}", legacy.display());
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if !target.exists() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

impl Store {
    /// Loads (or creates) state in `dir`. A corrupt library is moved aside, never overwritten.
    pub fn load(dir: PathBuf) -> Self {
        Self::load_with_defaults(dir, Settings::default())
    }

    /// Loads the isolated development variant with safe defaults for new settings.
    pub fn load_dev(dir: PathBuf) -> Self {
        let settings = Settings {
            paused: true,
            palette_hotkey: "Control+Alt+Shift+Space".into(),
            clipboard_hotkey: "Control+Alt+Shift+V".into(),
            ..Settings::default()
        };
        Self::load_with_defaults(dir, settings)
    }

    fn load_with_defaults(dir: PathBuf, defaults: Settings) -> Self {
        let _ = fs::create_dir_all(&dir);
        let lib_path = dir.join("library.json");
        let first_run = !lib_path.exists();
        let mut library = match fs::read_to_string(&lib_path) {
            Ok(json) => kudzu_json::decode(&json).unwrap_or_else(|e| {
                log::error!("library.json unreadable ({e}); keeping a copy and starting fresh");
                let _ = fs::rename(&lib_path, dir.join(format!("library-corrupt-{}.json", Utc::now().timestamp())));
                KudzuLibrary::default()
            }),
            Err(_) => KudzuLibrary::default(),
        };
        if first_run {
            coordinator::ensure_getting_started_snippets(&mut library, Utc::now());
        }
        // Keys missing from an older file take this variant's defaults (not the global ones), so a
        // dev build that predates a setting still gets its dev-safe value.
        let settings = fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|file| {
                let mut base = serde_json::to_value(&defaults).ok()?;
                if let (serde_json::Value::Object(b), serde_json::Value::Object(f)) = (&mut base, file) {
                    b.extend(f);
                }
                serde_json::from_value(base).ok()
            })
            .unwrap_or(defaults);
        let form_memory = fs::read_to_string(dir.join("form-memory.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let device_path = dir.join("device-id");
        let device_id = fs::read_to_string(&device_path).map(|s| s.trim().to_string()).unwrap_or_default();
        let device_id = if device_id.is_empty() {
            let id = Uuid::new_v4().to_string();
            let _ = write_atomic(&device_path, id.as_bytes());
            id
        } else {
            device_id
        };
        let store = Self { dir, library, settings, form_memory, device_id, first_run };
        if first_run {
            store.save_library();
            store.save_settings();
        }
        store
    }

    pub fn backups(&self) -> BackupStore {
        BackupStore::new(self.dir.join("backups"))
    }

    pub fn save_library(&self) {
        match kudzu_json::encode(&self.library) {
            Ok(json) => {
                if let Err(e) = write_atomic(&self.dir.join("library.json"), json.as_bytes()) {
                    log::error!("saving library failed: {e}");
                }
            }
            Err(e) => log::error!("encoding library failed: {e}"),
        }
    }

    pub fn save_settings(&self) {
        if let Ok(json) = serde_json::to_vec_pretty(&self.settings) {
            if let Err(e) = write_atomic(&self.dir.join("settings.json"), &json) {
                log::error!("saving settings failed: {e}");
            }
        }
    }

    pub fn save_form_memory(&self) {
        if let Ok(json) = serde_json::to_vec(&self.form_memory) {
            let _ = write_atomic(&self.dir.join("form-memory.json"), &json);
        }
    }

    /// Adds one expansion to this device's usage row for `snippet_id`.
    pub fn record_usage(&mut self, snippet_id: Uuid, chars_saved: i64) {
        let now = Utc::now();
        if let Some(row) = self.library.usage.iter_mut().find(|u| u.snippet_id == snippet_id && u.device_id == self.device_id) {
            row.count += 1;
            row.chars_saved += chars_saved.max(0);
            row.last_used_at = Some(now);
        } else {
            self.library.usage.push(UsageRecord {
                snippet_id,
                device_id: self.device_id.clone(),
                count: 1,
                chars_saved: chars_saved.max(0),
                last_used_at: Some(now),
            });
        }
    }

    /// Writes a daily backup if none exists for today, keeping the last 14.
    /// On the first launch of a new version, keeps a copy of the library from before it ran
    /// (so a migration or a bad release can always be undone), then records the version.
    pub fn note_app_version(&mut self, current: &str) {
        let previous = self.settings.last_run_version.clone();
        if previous.as_deref() == Some(current) {
            return;
        }
        if let Some(previous) = previous {
            log::info!("updated from {previous} to {current}");
            if let Err(e) = self.backups().write(&self.library, BackupReason::PreUpdate, Utc::now()) {
                log::error!("pre-update backup failed: {e}");
            }
        }
        self.settings.last_run_version = Some(current.to_string());
        self.save_settings();
    }

    pub fn daily_backup(&self) {
        let backups = self.backups();
        let now = Utc::now();
        if backups.needs_daily_backup(now) {
            if let Err(e) = backups.write(&self.library, BackupReason::Daily, now) {
                log::error!("daily backup failed: {e}");
            }
            let _ = backups.prune(14);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kudzu-store-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dev_first_run_is_safe_and_preserves_later_choices() {
        let dir = temp_root("dev-defaults");
        let mut store = Store::load_dev(dir.clone());
        assert!(store.settings.paused);
        assert_ne!(store.settings.palette_hotkey, Settings::default().palette_hotkey);
        store.settings.paused = false;
        store.settings.palette_hotkey = "Control+Alt+K".into();
        store.save_settings();

        let again = Store::load_dev(dir.clone());
        assert!(!again.settings.paused);
        assert_eq!(again.settings.palette_hotkey, "Control+Alt+K");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_settings_take_the_variant_defaults() {
        let dir = temp_root("variant-defaults");
        fs::write(dir.join("settings.json"), r#"{"paused":false,"paletteHotkey":"Control+Alt+K"}"#).unwrap();
        let dev = Store::load_dev(dir.clone());
        assert_eq!(dev.settings.clipboard_hotkey, "Control+Alt+Shift+V");
        assert_eq!(dev.settings.palette_hotkey, "Control+Alt+K");
        assert!(!dev.settings.paused);
        let _ = fs::remove_dir_all(dir);

        let dir = temp_root("variant-defaults-prod");
        fs::write(dir.join("settings.json"), r#"{"paused":true}"#).unwrap();
        let prod = Store::load(dir.clone());
        assert_eq!(prod.settings.clipboard_hotkey, "Control+Alt+V");
        assert_eq!(prod.settings.clipboard_retention_days, 7);
        assert!(prod.settings.paused);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_pre_release_data_once_and_never_overwrites() {
        let root = temp_root("migrate");
        let legacy = root.join("sh.saeed.kudzu.desktop");
        let current = root.join("sh.saeed.kudzu");
        fs::create_dir_all(legacy.join("backups")).unwrap();
        fs::write(legacy.join("library.json"), "legacy").unwrap();
        fs::write(legacy.join("backups/a.json"), "b").unwrap();

        migrate_legacy_dir(&current);
        assert_eq!(fs::read_to_string(current.join("library.json")).unwrap(), "legacy");
        assert_eq!(fs::read_to_string(current.join("backups/a.json")).unwrap(), "b");
        assert!(legacy.join("library.json").exists(), "copy, not move");

        // Once the new folder has a library, the legacy one is ignored.
        fs::write(current.join("library.json"), "mine").unwrap();
        fs::write(legacy.join("library.json"), "stale").unwrap();
        migrate_legacy_dir(&current);
        assert_eq!(fs::read_to_string(current.join("library.json")).unwrap(), "mine");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backs_up_once_per_new_version() {
        let dir = temp_root("version");
        let mut store = Store::load(dir.clone());
        let count = |s: &Store| s.backups().list().unwrap().iter().filter(|b| b.reason == BackupReason::PreUpdate).count();

        store.note_app_version("0.2.0"); // first launch with version tracking: nothing to protect yet
        assert_eq!(count(&store), 0);
        store.note_app_version("0.2.0");
        assert_eq!(count(&store), 0);
        store.note_app_version("0.3.0");
        assert_eq!(count(&store), 1);
        assert_eq!(store.settings.last_run_version.as_deref(), Some("0.3.0"));

        // Persisted: a fresh load knows the version already ran.
        let mut again = Store::load(dir.clone());
        again.note_app_version("0.3.0");
        assert_eq!(count(&again), 1);
        let _ = fs::remove_dir_all(dir);
    }
}
