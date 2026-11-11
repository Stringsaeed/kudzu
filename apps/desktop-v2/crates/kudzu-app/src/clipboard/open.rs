//! Opening and reconciling the local clipboard store.
use super::*;

impl ClipboardService {
    pub fn open(
        dir: PathBuf,
        settings: Watch<Settings>,
        ports: ClipboardPorts,
        writer: &Writer,
    ) -> Result<Self, ClipboardError> {
        let ClipboardPorts {
            board,
            thumbnails,
            target,
            icons,
            clock,
        } = ports;
        let store = Store::open(dir)?;
        let (file, mut loaded) = JsonFile::open(store.dir.join("history.json"), History::new)?;
        let durable = if loaded.source == LoadSource::Existing {
            Revision(1)
        } else {
            Revision(0)
        };
        store.clean(&loaded.value)?;
        let mut dirty = loaded.value.rescan_secrets();
        // Older builds only scanned a prefix. Recheck long blobs on every open until a
        // later model revision can identify which histories were scanned completely.
        for entry in &mut loaded.value.entries {
            if entry.text_len <= clipboard::TEXT_EXCERPT_LIMIT
                || !matches!(
                    entry.kind,
                    ClipKind::Text | ClipKind::Code | ClipKind::Link | ClipKind::Other
                )
            {
                continue;
            }
            let full = store
                .items(entry.id)
                .as_deref()
                .and_then(clipboard::text_of);
            let secret = match full {
                Some(full) if full.chars().count() == entry.text_len => {
                    kudzu_core::secrets::scan(&full)
                        .first()
                        .map(|finding| finding.kind)
                }
                _ => entry
                    .secret
                    .or(Some(kudzu_core::secrets::SecretKind::Credential)),
            };
            dirty |= secret != entry.secret;
            entry.secret = secret;
        }
        let retention = settings.get().clipboard_retention_days;
        let expired = loaded.value.prune(clock.now().into(), retention);
        let initial_cleanup = if expired.is_empty() {
            vec![]
        } else {
            vec![Cleanup {
                revision: Revision(1),
                entries: expired,
                trash: false,
            }]
        };
        let persisted = writer.register_with_min_interval(
            file,
            if dirty || !initial_cleanup.is_empty() {
                Revision(0)
            } else {
                durable
            },
            Duration::from_millis(900),
        )?;
        let value = Arc::new(loaded.value);
        persisted.submit(Revision(1), value.clone())?;
        let last_change = board.change_count();
        let publisher = Publisher::with_revision(value, Revision(1));
        let history = publisher.watch();
        Ok(Self {
            state: Mutex::new(State {
                publisher,
                last_change,
                retention,
                cleanup: initial_cleanup,
            }),
            history,
            operations: Mutex::new(()),
            store,
            persisted,
            settings,
            board,
            thumbnails,
            target,
            icons,
            clock,
            runtime: Mutex::new(None),
            search: Mutex::new(None),
            #[cfg(test)]
            cache_builds: std::sync::atomic::AtomicUsize::new(0),
            storage_error: Publisher::new(None),
            capture_error: Publisher::new(None),
            usage_cache: Mutex::new(None),
            retry_collect: Mutex::new(None),
        })
    }
}
