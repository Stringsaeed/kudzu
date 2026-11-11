//! Permission-setup state machine and the restart marker.

mod state;

use crate::error::{IoFailure, PortError};
use crate::ports::{ExpansionEngine, PermissionKind, Relauncher, SettingsPane, SystemSettings};
use crate::schedule::{ScheduleError, Scheduler, Timer};
use crate::watch::{Publisher, Watch};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

pub use state::{
    accessibility_name, granted, ready, PermissionsState, Phase, ACCESSIBILITY_NAME,
    DEVICE_CONTROL_NAME, OPEN_SETTINGS_ERROR,
};

pub const RESTART_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PermissionsError {
    #[error("{}", OPEN_SETTINGS_ERROR)]
    OpenSettings,
    #[error("The restart marker could not be written.")]
    Marker(#[from] IoFailure),
    #[error("The restart timer could not start.")]
    Schedule(#[from] ScheduleError),
    #[error(transparent)]
    Port(#[from] PortError),
}

pub struct PermissionsConfig {
    pub app_name: String,
    pub os_major: u32,
    pub marker: PathBuf,
}

struct Inner {
    engine: Arc<dyn ExpansionEngine>,
    system: Arc<dyn SystemSettings>,
    relauncher: Arc<dyn Relauncher>,
    scheduler: Arc<Scheduler>,
    marker: PathBuf,
    publisher: Mutex<Publisher<PermissionsState>>,
    restart_timer: Mutex<Option<Timer>>,
    refreshing: AtomicBool,
}

pub struct Permissions {
    inner: Arc<Inner>,
}

impl Permissions {
    pub fn new(
        engine: Arc<dyn ExpansionEngine>,
        system: Arc<dyn SystemSettings>,
        relauncher: Arc<dyn Relauncher>,
        scheduler: Arc<Scheduler>,
        config: PermissionsConfig,
    ) -> Self {
        let status = engine.status();
        let initial = PermissionsState {
            selected: PermissionKind::Accessibility,
            phase: Phase::Checking,
            ready: ready(&status),
            restart_offered: false,
            accessibility_name: accessibility_name(config.os_major),
            app_name: config.app_name,
            status,
            error: None,
        };
        Self {
            inner: Arc::new(Inner {
                engine,
                system,
                relauncher,
                scheduler,
                marker: config.marker,
                publisher: Mutex::new(Publisher::new(initial)),
                restart_timer: Mutex::new(None),
                refreshing: AtomicBool::new(false),
            }),
        }
    }

    pub fn watch(&self) -> Watch<PermissionsState> {
        self.inner
            .publisher
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watch()
    }

    pub fn state(&self) -> Arc<PermissionsState> {
        self.watch().get()
    }

    pub fn select(&self, kind: PermissionKind) {
        self.edit(|live| {
            live.selected = kind;
            live.error = None;
            if live.phase == Phase::Opening || live.phase == Phase::Waiting {
                live.phase = Phase::Checking;
            }
            apply_status(live);
        });
    }

    pub fn begin(&self, kind: PermissionKind) -> Result<(), PermissionsError> {
        self.edit(|live| {
            live.selected = kind;
            live.phase = Phase::Opening;
            live.error = None;
            live.restart_offered = false;
        });
        let pane = match kind {
            PermissionKind::Accessibility => SettingsPane::Accessibility,
            PermissionKind::InputMonitoring => SettingsPane::InputMonitoring,
        };
        if let Err(error) = self
            .inner
            .system
            .request(kind)
            .and_then(|_| self.inner.system.open(pane))
        {
            self.edit(|live| {
                live.phase = Phase::Error;
                live.error = Some(OPEN_SETTINGS_ERROR.into());
            });
            return Err(match error {
                PortError::Unavailable | PortError::Io(_) => PermissionsError::OpenSettings,
                other => PermissionsError::Port(other),
            });
        }
        self.edit(|live| {
            if live.phase == Phase::Opening {
                live.phase = Phase::Checking;
            }
        });
        self.refresh();
        self.edit(|live| {
            if live.phase == Phase::Checking {
                live.phase = Phase::Waiting;
            }
        });
        Ok(())
    }

    pub fn refresh(&self) {
        if self
            .inner
            .refreshing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let status = self.inner.engine.status();
        self.edit(|live| {
            live.status = status;
            apply_status(live);
        });
        self.inner.refreshing.store(false, Ordering::Release);
    }

    pub fn request_restart(&self) -> Result<(), PermissionsError> {
        if let Some(parent) = self.inner.marker.parent() {
            std::fs::create_dir_all(parent).map_err(IoFailure::from)?;
        }
        std::fs::write(&self.inner.marker, []).map_err(IoFailure::from)?;
        self.inner.relauncher.relaunch()?;
        Ok(())
    }

    /// Consume the marker once. The marker is not a grant.
    pub fn take_pending_restart(&self) -> bool {
        std::fs::remove_file(&self.inner.marker).is_ok()
    }

    fn edit(&self, update: impl FnOnce(&mut PermissionsState)) {
        let connecting = {
            let publisher = self
                .inner
                .publisher
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let mut next = (*publisher.watch().get()).clone();
            update(&mut next);
            let connecting = next.phase == Phase::Connecting;
            let committed = publisher.replace(Arc::new(next));
            drop(publisher);
            committed.notification.notify();
            connecting
        };
        self.sync_restart_timer(connecting);
    }

    fn sync_restart_timer(&self, connecting: bool) {
        let mut timer = self
            .inner
            .restart_timer
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if connecting {
            if timer.is_some() {
                return;
            }
            let inner = Arc::downgrade(&self.inner);
            if let Ok(handle) = self.inner.scheduler.after(RESTART_WAIT, move || {
                if let Some(inner) = inner.upgrade() {
                    offer_restart(&inner);
                }
            }) {
                *timer = Some(handle);
            }
        } else {
            timer.take();
        }
    }
}

fn offer_restart(inner: &Inner) {
    let publisher = inner
        .publisher
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let current = publisher.watch().get();
    if current.phase != Phase::Connecting || current.restart_offered {
        return;
    }
    let mut next = (*current).clone();
    next.restart_offered = true;
    let committed = publisher.replace(Arc::new(next));
    drop(publisher);
    committed.notification.notify();
}

fn apply_status(live: &mut PermissionsState) {
    live.ready = ready(&live.status);
    let selected_granted = granted(&live.status, live.selected);
    if live.phase == Phase::Opening {
        return;
    }
    if live.phase == Phase::Error && !selected_granted {
        return;
    }
    live.error = None;
    if selected_granted {
        live.phase = if live.selected == PermissionKind::Accessibility && !live.ready {
            Phase::Connecting
        } else {
            Phase::Allowed
        };
        if live.phase != Phase::Connecting {
            live.restart_offered = false;
        }
        return;
    }
    if live.phase == Phase::Allowed || live.phase == Phase::Connecting {
        live.phase = Phase::Revoked;
        live.restart_offered = false;
        return;
    }
    if live.phase != Phase::Waiting {
        live.phase = Phase::Checking;
        live.restart_offered = false;
    }
}

#[cfg(test)]
mod tests;
