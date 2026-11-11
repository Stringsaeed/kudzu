//! Typed settings and their local persistence.

mod model;
pub use model::{Settings, Theme, UpdateChannel, Variant};

use crate::persist::{JsonFile, LoadSource, PersistError, Persisted, Revision, SaveState, Writer};
use crate::watch::{Publisher, Watch};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("Settings could not be saved or loaded.")]
    Persistence(#[from] PersistError),
    #[error("The trigger prefix must be at most four UTF-16 code units.")]
    TriggerPrefix,
    #[error("Keep history for 1 to 3650 days, or forever.")]
    Retention,
}

/// Local preferences. Side effects belong to subscribers, never to a settings transaction.
pub struct SettingsService {
    state: Mutex<Publisher<Settings>>,
    persisted: Persisted<Settings>,
}

impl SettingsService {
    /// Startup I/O only. Missing keys inherit this variant's defaults before typed decoding.
    /// Out-of-range stored fields are clamped and queued for repair without discarding preferences.
    pub fn open(path: PathBuf, variant: Variant, writer: &Writer) -> Result<Self, SettingsError> {
        let (file, mut loaded) = JsonFile::open_with(
            path,
            || Settings::for_variant(variant),
            |bytes| {
                let serde_json::Value::Object(file) =
                    serde_json::from_slice(bytes).map_err(|_| PersistError::Decode)?
                else {
                    return Err(PersistError::Decode);
                };
                let mut base = serde_json::to_value(Settings::for_variant(variant))
                    .map_err(|_| PersistError::Encode)?;
                base.as_object_mut()
                    .expect("settings are an object")
                    .extend(file);
                serde_json::from_value(base).map_err(|_| PersistError::Decode)
            },
            |value| serde_json::to_vec_pretty(value).map_err(|_| PersistError::Encode),
        )?;
        let repaired = clamp_stored(&mut loaded.value);
        let durable = if loaded.source == LoadSource::Existing && !repaired {
            Revision(1)
        } else {
            Revision(0)
        };
        Self::new(loaded.value, writer.register(file, durable)?)
    }

    /// Construct over a writer target (including an in-memory target for scenes).
    /// Register at revision 0 for unsaved input, or 1 for an already durable snapshot.
    pub fn new(initial: Settings, persisted: Persisted<Settings>) -> Result<Self, SettingsError> {
        let initial = Arc::new(initial);
        persisted.submit(Revision(1), initial.clone())?;
        Ok(Self {
            state: Mutex::new(Publisher::with_revision(initial, Revision(1))),
            persisted,
        })
    }

    pub fn watch(&self) -> Watch<Settings> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watch()
    }

    /// The same acceptance revision is used by Watch and saved().
    pub fn snapshot(&self) -> (Revision, Arc<Settings>) {
        self.watch().snapshot()
    }

    pub fn saved(&self) -> Watch<SaveState> {
        self.persisted.saved()
    }

    /// A pure edit of the latest settings. Do no I/O and call no service in the closure.
    pub fn update(&self, edit: impl FnOnce(&mut Settings)) -> Result<Revision, SettingsError> {
        let (revision, notification) = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let (revision, current) = state.watch().snapshot();
            let mut next = (*current).clone();
            edit(&mut next);
            validate(&next)?;
            if next == *current {
                return Ok(revision);
            }
            let revision = Revision(
                revision
                    .0
                    .checked_add(1)
                    .expect("settings revision exhausted"),
            );
            let next = Arc::new(next);
            self.persisted.submit(revision, next.clone())?;
            let committed = state.replace(next);
            (committed.revision, committed.notification)
        };
        notification.notify();
        Ok(revision)
    }

    /// Matches the reference input's four UTF-16 code unit limit, including emoji.
    pub fn set_trigger_prefix(&self, prefix: String) -> Result<Revision, SettingsError> {
        self.update(|s| s.default_trigger_prefix = prefix)
    }

    /// Zero means forever; positive values are days.
    pub fn set_clipboard_retention_days(&self, days: u32) -> Result<Revision, SettingsError> {
        self.update(|s| s.clipboard_retention_days = days)
    }
}

fn clamp_stored(settings: &mut Settings) -> bool {
    let mut units = 0;
    let end = settings
        .default_trigger_prefix
        .char_indices()
        .find_map(|(at, character)| {
            units += character.len_utf16();
            (units > 4).then_some(at)
        });
    if let Some(end) = end {
        settings.default_trigger_prefix.truncate(end);
    }
    let retention_invalid = settings.clipboard_retention_days > 3650;
    settings.clipboard_retention_days = settings.clipboard_retention_days.min(3650);
    end.is_some() || retention_invalid
}

fn validate(settings: &Settings) -> Result<(), SettingsError> {
    if settings.default_trigger_prefix.encode_utf16().count() > 4 {
        return Err(SettingsError::TriggerPrefix);
    }
    if settings.clipboard_retention_days > 3650 {
        return Err(SettingsError::Retention);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
