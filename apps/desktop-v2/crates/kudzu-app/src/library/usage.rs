use super::{LibraryError, LibraryService};
use crate::persist::Revision;
use kudzu_core::library::model::UsageRecord;
use uuid::Uuid;

impl LibraryService {
    /// Called only after insertion. Keeps the device's row separate from synced usage rows.
    pub fn record_expansion(
        &self,
        snippet_id: Uuid,
        chars_saved: i64,
    ) -> Result<Revision, LibraryError> {
        self.transact(|library| {
            let now = self.clock.now().into();
            if let Some(row) = library
                .usage
                .iter_mut()
                .find(|u| u.snippet_id == snippet_id && u.device_id == self.device_id)
            {
                row.count = row.count.saturating_add(1);
                row.chars_saved = row.chars_saved.saturating_add(chars_saved.max(0));
                row.last_used_at = Some(now);
            } else {
                library.usage.push(UsageRecord {
                    snippet_id,
                    device_id: self.device_id.clone(),
                    count: 1,
                    chars_saved: chars_saved.max(0),
                    last_used_at: Some(now),
                });
            }
            Ok(())
        })
        .map(|(_, revision)| revision)
    }
}
