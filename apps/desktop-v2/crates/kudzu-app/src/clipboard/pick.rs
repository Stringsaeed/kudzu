//! Copy and picker sessions; paste runs on a worker after the panel hides.
use super::*;
use std::sync::Condvar;
use std::time::Instant;

/// One copy-through-paste at a time, across every picker opening. The pasteboard is shared:
/// a second copy between the first copy and its keystroke would change what the first paste
/// inserts.
#[derive(Default)]
pub(super) struct PasteGate {
    busy: Mutex<bool>,
    freed: Condvar,
}

/// Holds the gate until it is dropped.
struct Turn(Arc<PasteGate>);

impl PasteGate {
    fn try_enter(self: &Arc<Self>) -> Option<Turn> {
        let mut busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        if *busy {
            return None;
        }
        *busy = true;
        Some(Turn(self.clone()))
    }

    /// Waits for the gate. Call on a worker.
    fn enter(self: &Arc<Self>) -> Turn {
        let mut busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        while *busy {
            busy = self
                .freed
                .wait(busy)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *busy = true;
        Turn(self.clone())
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        *self.0.busy.lock().unwrap_or_else(PoisonError::into_inner) = false;
        self.0.freed.notify_all();
    }
}

/// Proof that the pasteboard holds a clip for a paste in progress. Nothing else may write the
/// pasteboard until it is dropped; hand it to [`PickSession::paste_copied`].
pub struct PasteTicket {
    _turn: Turn,
}

/// This process is never a paste target: a keystroke posted to it would land in Kudzu.
fn is_self(app: &AppInfo) -> bool {
    app.pid == Some(std::process::id() as i32)
}

impl ClipboardService {
    /// Puts a clip on the pasteboard. Waits for a paste in progress to finish first.
    pub fn copy(&self, id: Uuid, flavor: Flavor) -> Result<(), ClipboardError> {
        let _turn = self.pasting.enter();
        self.copy_now(id, flavor)
    }

    /// Puts a clip on the pasteboard for a paste that follows. The ticket keeps every other
    /// copy and paste away until it is spent or dropped. `PasteBusy` while another is
    /// in progress.
    pub fn copy_for_paste(&self, id: Uuid, flavor: Flavor) -> Result<PasteTicket, ClipboardError> {
        let turn = self.pasting.try_enter().ok_or(ClipboardError::PasteBusy)?;
        self.copy_now(id, flavor)?;
        Ok(PasteTicket { _turn: turn })
    }

    fn copy_now(&self, id: Uuid, flavor: Flavor) -> Result<(), ClipboardError> {
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
        let target = self.target.frontmost()?.filter(|app| !is_self(app));
        Ok(PickSession {
            service: self.clone(),
            can_paste: target.is_some() && self.board.can_paste(),
            target,
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
            // A paste in progress activates its own target; this waits for it to finish.
            let _turn = self.service.pasting.enter();
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
        let ticket = self.service.copy_for_paste(id, flavor)?;
        self.paste_copied(ticket)
    }

    /// Pastes what [`ClipboardService::copy_for_paste`] just put on the pasteboard. A picker
    /// copies first, so a copy that fails (a missing item) can still be shown in its footer,
    /// then hides, then calls this. Blocks for up to about 850 ms: call it on a worker.
    ///
    /// With no external target (Kudzu itself was in front, or nothing was), or without
    /// permission, the clip stays on the pasteboard and the outcome is `Copied`. The target is
    /// looked at again just before the keystroke, so a paste never goes to another app.
    pub fn paste_copied(&self, ticket: PasteTicket) -> Result<PasteOutcome, ClipboardError> {
        let _ticket = ticket;
        let Some(target) = &self.target else {
            return Ok(PasteOutcome::Copied);
        };
        if !self.service.board.can_paste() {
            return Ok(PasteOutcome::Copied);
        }
        match self.service.target.activate(target) {
            Ok(()) => {}
            Err(PortError::TargetGone) => return Err(ClipboardError::TargetGone),
            Err(e) => return Err(e.into()),
        }
        let deadline = Instant::now() + Duration::from_millis(800);
        loop {
            if self.in_front(target)? {
                break;
            }
            if Instant::now() >= deadline {
                return Err(ClipboardError::TargetGone);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(40));
        if !self.in_front(target)? {
            return Err(ClipboardError::TargetGone);
        }
        match self.service.board.paste(Some(target)) {
            Ok(()) => Ok(PasteOutcome::Pasted),
            Err(PortError::PermissionDenied) => Ok(PasteOutcome::Copied),
            Err(PortError::TargetGone) => Err(ClipboardError::TargetGone),
            Err(e) => Err(e.into()),
        }
    }

    /// Whether `target` is the app in front, and that app is not Kudzu.
    fn in_front(&self, target: &AppInfo) -> Result<bool, ClipboardError> {
        Ok(self.service.target.frontmost()?.is_some_and(|app| {
            !is_self(&app)
                && match (target.pid, app.pid) {
                    (Some(expected), Some(actual)) => expected == actual,
                    _ => target.bundle_id == app.bundle_id,
                }
        }))
    }
}
