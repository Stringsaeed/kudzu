//! Poll and record changed pasteboard snapshots without holding the history mutex.
use super::*;
impl ClipboardService {
    /// Checks the current count once; callers schedule this every 300 ms.
    pub fn poll(&self) -> Result<(), ClipboardError> {
        let result = self.poll_inner();
        self.publish_capture_error(result.as_ref().err().copied());
        result
    }
    fn poll_inner(&self) -> Result<(), ClipboardError> {
        let settings = self.settings.get();
        let count = self.board.change_count();
        let (changed, retention_changed) = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            (
                count != state.last_change,
                state.retention != settings.clipboard_retention_days,
            )
        };
        if retention_changed {
            self.prune()?;
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .retention = settings.clipboard_retention_days;
        }
        if !changed {
            return Ok(());
        }
        if !settings.clipboard_enabled || self.board.is_own(count) {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_change = count;
            return Ok(());
        }
        let front = self.target.frontmost()?;
        if front
            .as_ref()
            .is_some_and(|a| settings.clipboard_ignored_apps.contains(&a.bundle_id))
        {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_change = count;
            return Ok(());
        }
        if self.board.has_private_marker(count)? {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_change = count;
            return Ok(());
        }
        let snapshot = self.board.read()?;
        if snapshot.own || clipboard::is_private(&snapshot.items) {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_change = snapshot.change_count;
            return Ok(());
        }
        let front = self.target.frontmost()?;
        let settings = self.settings.get();
        if !settings.clipboard_enabled
            || front
                .as_ref()
                .is_some_and(|app| settings.clipboard_ignored_apps.contains(&app.bundle_id))
        {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_change = snapshot.change_count;
            return Ok(());
        }
        if snapshot.change_count != count || self.board.change_count() != snapshot.change_count {
            return Ok(());
        }
        let Some(c) = classify(
            &snapshot.items,
            front.as_ref().map(|a| a.bundle_id.as_str()),
        ) else {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_change = snapshot.change_count;
            return Ok(());
        };
        let source = front.map(|a| ClipSource {
            bundle_id: a.bundle_id,
            name: a.name,
            path: a.path,
        });
        let hash = format!("{:016x}", c.hash);
        let known = self
            .history
            .get()
            .entries
            .iter()
            .any(|entry| entry.hash == hash);
        let mut thumb = if known {
            None
        } else {
            self.thumbnail_for(&c, &snapshot.items)?
        };
        let operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.board.change_count() != snapshot.change_count {
            return Ok(());
        }
        {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.last_change == snapshot.change_count {
                return Ok(());
            }
        }
        let (revision, current) = self.history.snapshot();
        let repeated = current.entries.iter().any(|entry| entry.hash == hash);
        if known && !repeated {
            thumb = self.thumbnail_for(&c, &snapshot.items)?;
        }
        let image = thumb.as_ref().map(|t| ImageSize {
            width: t.width,
            height: t.height,
        });
        let mut next = (*current).clone();
        let ingest = next.ingest(&c, source, image, self.clock.now().into());
        let entry = ingest.entry();
        let id = entry.id;
        let was_present = current.get(id).is_some();
        let write_result = (|| {
            let blob = self.store.blob(id);
            if c.size <= MAX_BLOB && !(repeated && blob.exists()) {
                self.store
                    .write(&blob, &clipboard::encode_items(&snapshot.items))?;
            }
            if let Some(t) = &thumb {
                self.store.write(&self.store.thumb(id), &t.png)?;
            }
            Ok::<_, ClipboardError>(())
        })();
        if let Err(error) = write_result {
            if !was_present {
                self.store.purge(std::slice::from_ref(entry))?;
            }
            return Err(error);
        }
        let next = Arc::new(next);
        if let Err(error) = self
            .persisted
            .submit(Revision(revision.0 + 1), next.clone())
        {
            if !was_present {
                self.store.purge(std::slice::from_ref(entry))?;
            }
            return Err(error.into());
        }
        let notification = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.last_change = snapshot.change_count;
            state.publisher.replace(next).notification
        };
        *self
            .usage_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        drop(operation);
        notification.notify();
        self.prune()
    }
    fn thumbnail_for(
        &self,
        classified: &clipboard::Classified,
        items: &[PasteItem],
    ) -> Result<Option<crate::ports::Thumbnail>, ClipboardError> {
        match classified.kind {
            ClipKind::Image => match clipboard::image_of(items) {
                Some((bytes, _)) => Ok(self.thumbnails.thumbnail(bytes, 480)?),
                None => Ok(None),
            },
            ClipKind::File
                if classified.files.len() == 1
                    && clipboard::image_mime_for_path(&classified.files[0]).is_some() =>
            {
                let data = fs::metadata(&classified.files[0])
                    .ok()
                    .filter(|m| m.is_file() && m.len() <= MAX_FILE_THUMB)
                    .and_then(|_| fs::read(&classified.files[0]).ok());
                match data {
                    Some(data) => Ok(self.thumbnails.thumbnail(&data, 480)?),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }
}
