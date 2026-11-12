//! Copy and picker sessions; paste runs on a worker after the panel hides.
use super::*;
use std::time::Instant;
impl ClipboardService {
    pub fn copy(&self, id: Uuid, flavor: Flavor) -> Result<(), ClipboardError> {
        let h = self.watch().get();
        let e = h.get(id).ok_or(ClipboardError::Missing)?;
        if e.size > MAX_BLOB {
            return Err(ClipboardError::TooLarge);
        }
        let stored = match fs::read(self.store.blob(id)) {
            Ok(bytes) => Some(clipboard::decode_items(&bytes).ok_or(ClipboardError::CorruptData)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ClipboardError::MissingData)
            }
            Err(error) => return Err(IoFailure::from(error).into()),
        };
        let full = stored.as_deref().and_then(clipboard::text_of).or_else(|| {
            (e.text_len <= clipboard::TEXT_EXCERPT_LIMIT)
                .then(|| e.text.clone())
                .flatten()
        });
        let plain = || {
            full.clone()
                .map(|s| vec![vec![(clipboard::uti::PLAIN_TEXT.into(), s.into_bytes())]])
        };
        let items = if flavor == Flavor::Plain {
            plain().or(stored)
        } else {
            stored.or_else(plain)
        }
        .ok_or(ClipboardError::TooLarge)?;
        self.board.write(&items)?;
        let now = self.clock.now().into();
        match self.change(|h| {
            h.mark_used(id, now).ok_or(ClipboardError::Missing)?;
            Ok(())
        }) {
            Err(ClipboardError::Missing) => Ok(()),
            result => result,
        }
    }
    pub fn begin_pick(self: &Arc<Self>) -> Result<PickSession, ClipboardError> {
        Ok(PickSession {
            service: self.clone(),
            target: self.target.frontmost()?,
            can_paste: self.board.can_paste(),
        })
    }
}
pub struct PickSession {
    service: Arc<ClipboardService>,
    target: Option<AppInfo>,
    pub can_paste: bool,
}
impl PickSession {
    /// The app the user came from, which a paste goes to and a close gives focus back to.
    pub fn target(&self) -> Option<&AppInfo> {
        self.target.as_ref()
    }

    /// Gives focus back to the app the user came from, on a worker (the activation can
    /// block). An app that has quit is not an error: there is nothing to give focus to.
    pub fn restore_target(
        self,
    ) -> Result<crate::tasks::Task<Result<(), ClipboardError>>, crate::tasks::TaskError> {
        crate::tasks::Task::spawn("kudzu-picker-restore", move || {
            let Some(target) = self.target else {
                return Ok(());
            };
            match self.service.target.activate(&target) {
                Ok(()) | Err(PortError::TargetGone) => Ok(()),
                Err(error) => Err(error.into()),
            }
        })
    }

    /// Blocks for up to about 850 ms. Hide the panel first and call on a worker, never the UI.
    pub fn paste(&self, id: Uuid, flavor: Flavor) -> Result<PasteOutcome, ClipboardError> {
        self.service.copy(id, flavor)?;
        self.paste_copied()
    }

    /// Pastes what [`ClipboardService::copy`] just put on the pasteboard. A picker copies
    /// first, so a copy that fails (a missing item) can still be shown in its footer, then
    /// hides, then calls this. Blocks for up to about 850 ms: call it on a worker.
    pub fn paste_copied(&self) -> Result<PasteOutcome, ClipboardError> {
        if !self.service.board.can_paste() {
            return Ok(PasteOutcome::Copied);
        }
        if let Some(target) = &self.target {
            match self.service.target.activate(target) {
                Ok(()) => {}
                Err(PortError::TargetGone) => return Err(ClipboardError::TargetGone),
                Err(e) => return Err(e.into()),
            }
            let deadline = Instant::now() + Duration::from_millis(800);
            loop {
                let active = self.service.target.frontmost()?;
                if active
                    .as_ref()
                    .is_some_and(|app| match (target.pid, app.pid) {
                        (Some(expected), Some(actual)) => expected == actual,
                        _ => target.bundle_id == app.bundle_id,
                    })
                {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(ClipboardError::TargetGone);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        match self.service.board.paste(self.target.as_ref()) {
            Ok(()) => Ok(PasteOutcome::Pasted),
            Err(PortError::PermissionDenied) => Ok(PasteOutcome::Copied),
            Err(PortError::TargetGone) => Err(ClipboardError::TargetGone),
            Err(e) => Err(e.into()),
        }
    }
}
