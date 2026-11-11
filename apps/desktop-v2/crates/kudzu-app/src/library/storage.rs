use super::{LibraryError, LibraryService};
use crate::schedule::Scheduler;
use crate::{
    persist::{JsonFile, LoadSource, PersistError, Revision, Writer},
    ports::{Clock, IdSource},
};
use kudzu_core::{
    interchange::InterchangeError,
    library::{json, model::KudzuLibrary, starter},
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

impl LibraryService {
    /// Startup I/O only. Missing and corrupt are distinct: only Missing gets starter snippets.
    /// LoadSource also lets composition distinguish first run and report a recovered file.
    pub fn open(
        path: PathBuf,
        device_id: String,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdSource>,
        writer: &Writer,
        scheduler: Arc<Scheduler>,
    ) -> Result<(Self, LoadSource), LibraryError> {
        let (file, mut loaded) = JsonFile::open_with(path, KudzuLibrary::default, decode, encode)?;
        if loaded.source == LoadSource::Missing {
            starter::ensure_getting_started_snippets(&mut loaded.value, clock.now().into());
            // Core supplies the canonical starter content; service-owned IDs keep scenes repeatable.
            let mut groups = HashMap::new();
            for group in &mut loaded.value.groups {
                let id = ids.next_id();
                groups.insert(group.id, id);
                group.id = id;
            }
            for snippet in &mut loaded.value.snippets {
                snippet.id = ids.next_id();
                snippet.group_id = snippet.group_id.and_then(|id| groups.get(&id).copied());
            }
        }
        let durable = if loaded.source == LoadSource::Existing {
            Revision(1)
        } else {
            Revision(0)
        };
        let service = Self::new(
            loaded.value,
            device_id,
            clock,
            ids,
            writer.register(file, durable)?,
            scheduler,
        )?;
        Ok((service, loaded.source))
    }
}

fn decode(bytes: &[u8]) -> Result<KudzuLibrary, PersistError> {
    let text = std::str::from_utf8(bytes).map_err(|_| PersistError::Decode)?;
    json::decode(text).map_err(|error| match error {
        InterchangeError::UnsupportedFormatVersion { .. } => PersistError::UnsupportedVersion,
        _ => PersistError::Decode,
    })
}
fn encode(library: &KudzuLibrary) -> Result<Vec<u8>, PersistError> {
    json::encode(library)
        .map(String::into_bytes)
        .map_err(|_| PersistError::Encode)
}
