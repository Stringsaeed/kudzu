//! Installed-app catalogue and icon cache. Enumeration runs once; names fall back to bundle ids.

use crate::{
    ports::AppCatalog,
    tasks::{Task, TaskError},
    watch::{Publisher, Watch},
};
use kudzu_core::system::AppInfo;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

type IconCache = HashMap<(String, u32), Vec<u8>>;

/// Cached installed apps and icons. Uses [`AppCatalog`] only; never [`crate::ports::TargetApp`].
pub struct Apps {
    installed: Watch<Vec<AppInfo>>,
    /// Held so dropping `Apps` is the only way the listing slot is abandoned.
    #[allow(dead_code)]
    listing: Task<Vec<AppInfo>>,
    icons: Arc<Mutex<IconCache>>,
    catalog: Arc<dyn AppCatalog>,
}

impl Apps {
    /// One enumeration, kept for the life of the service. A catalogue error yields an empty list.
    pub fn new(catalog: Arc<dyn AppCatalog>) -> Self {
        let publisher = Publisher::new(Vec::new());
        let installed = publisher.watch();
        let catalog_for_list = catalog.clone();
        let listing = Task::spawn("kudzu-apps-list", move || {
            let listed = catalog_for_list.installed().unwrap_or_default();
            let committed = publisher.replace(Arc::new(listed.clone()));
            committed.notification.notify();
            listed
        })
        .unwrap_or_else(|_| Task::done(Vec::new()));
        Self {
            installed,
            listing,
            icons: Arc::new(Mutex::new(HashMap::new())),
            catalog,
        }
    }

    pub fn installed(&self) -> Watch<Vec<AppInfo>> {
        self.installed.clone()
    }

    pub fn name_of(&self, bundle_id: &str) -> String {
        self.installed
            .get()
            .iter()
            .find(|app| app.bundle_id == bundle_id)
            .map(|app| app.name.clone())
            .unwrap_or_else(|| bundle_id.to_owned())
    }

    /// PNG bytes when the catalogue has an icon. Hits the in-memory cache after the first fetch.
    pub fn icon(&self, bundle_id: &str, size: u32) -> Result<Task<Option<Vec<u8>>>, TaskError> {
        let key = (bundle_id.to_owned(), size);
        if let Some(cached) = self
            .icons
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned()
        {
            return Task::spawn("kudzu-app-icon", move || Some(cached));
        }
        let catalog = self.catalog.clone();
        let cache = self.icons.clone();
        Task::spawn("kudzu-app-icon", move || {
            match catalog.icon(&key.0, key.1) {
                Ok(Some(bytes)) => {
                    cache
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(key, bytes.clone());
                    Some(bytes)
                }
                Ok(None) | Err(_) => None,
            }
        })
    }
}

#[cfg(test)]
mod tests;
