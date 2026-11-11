//! Transport intents supplied by the service, deduplicated by record name.
use kudzu_app::ports::CloudBatch;
use kudzu_core::sync::RemoteRecord;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Pending(BTreeMap<String, Change>);

enum Change {
    Save(RemoteRecord),
    Delete,
}

impl Pending {
    pub(super) fn upsert(&mut self, batch: CloudBatch) {
        for record in batch.records {
            self.0.insert(record.name.clone(), Change::Save(record));
        }
        for name in batch.deleted {
            self.0.insert(name, Change::Delete);
        }
    }

    pub(super) fn saves(&self) -> Vec<RemoteRecord> {
        self.0
            .values()
            .filter_map(|change| match change {
                Change::Save(record) => Some(record.clone()),
                Change::Delete => None,
            })
            .collect()
    }

    pub(super) fn deletes(&self) -> Vec<String> {
        self.0
            .iter()
            .filter(|(_, change)| matches!(change, Change::Delete))
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub(super) fn saved(&mut self, record: &RemoteRecord) {
        if matches!(self.0.get(&record.name), Some(Change::Save(pending)) if pending == record) {
            self.0.remove(&record.name);
        }
    }

    pub(super) fn deleted(&mut self, name: &str) {
        if matches!(self.0.get(name), Some(Change::Delete)) {
            self.0.remove(name);
        }
    }

    pub(super) fn remove(&mut self, name: &str) {
        self.0.remove(name);
    }
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
