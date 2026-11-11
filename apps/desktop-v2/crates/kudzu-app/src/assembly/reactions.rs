//! Settings and sync reactions. Subscribers only set a flag. Login-item changes, agent
//! apply, and the update check run on this thread, never on the UI thread.
//!
//! A sticky [`SyncStatus::NeedsUpdate`] starts one quiet check. The updates service has
//! `Trigger::User` and `Trigger::Scheduled`; Scheduled is the quiet one (the previous app
//! called that path a background check). The check does not restart the app.

use crate::{
    agents::AgentsService,
    ports::LoginItem,
    settings::{Settings, SettingsService, UpdateChannel},
    sync::SyncStatus,
    tasks::Task,
    updates::{Channel, Prefs, Trigger, Updates},
    watch::{Publisher, Subscription, Watch},
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

/// Shown once when enabling or disabling the login item fails. The reaction does not retry
/// until `launch_at_login` changes.
pub const LOGIN_ITEM_UNAVAILABLE: &str = "login item unavailable";

struct Flags {
    settings: bool,
    sync: bool,
    stop: bool,
}

pub(super) struct Reactions {
    flags: Arc<(Mutex<Flags>, Condvar)>,
    thread: Mutex<Option<JoinHandle<()>>>,
    #[cfg_attr(not(test), allow(dead_code))]
    checks: Arc<AtomicU32>,
}

impl Reactions {
    pub(super) fn start(
        settings: Arc<SettingsService>,
        agents: Arc<AgentsService>,
        login_item: Arc<dyn LoginItem>,
        updates: Updates,
        sync_status: Watch<SyncStatus>,
        prefs: Arc<Publisher<Prefs>>,
        login_notice: Arc<Publisher<Option<String>>>,
    ) -> Self {
        let flags = Arc::new((
            Mutex::new(Flags {
                settings: true,
                sync: true,
                stop: false,
            }),
            Condvar::new(),
        ));
        let checks = Arc::new(AtomicU32::new(0));
        let settings_watch = settings.watch();
        let sync_watch = sync_status.clone();
        let wake_settings = Arc::clone(&flags);
        let wake_sync = Arc::clone(&flags);
        let subscriptions = [
            settings_watch.subscribe(move || raise(&wake_settings, true)),
            sync_watch.subscribe(move || raise(&wake_sync, false)),
        ];
        let worker = Worker {
            flags: Arc::clone(&flags),
            _subscriptions: subscriptions,
            settings,
            agents,
            login_item,
            updates,
            sync_status,
            prefs,
            login_notice,
            checks: Arc::clone(&checks),
        };
        let thread = std::thread::Builder::new()
            .name("kudzu-reactions".into())
            .spawn(move || worker.run())
            .expect("reaction thread");
        flags.1.notify_all();
        Self {
            flags,
            thread: Mutex::new(Some(thread)),
            checks,
        }
    }

    pub(super) fn stop(&self) {
        {
            let mut flags = self.flags.0.lock().unwrap_or_else(PoisonError::into_inner);
            flags.stop = true;
            self.flags.1.notify_all();
        }
        if let Some(thread) = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = thread.join();
        }
    }

    #[cfg(test)]
    pub(super) fn update_checks(&self) -> u32 {
        self.checks.load(Ordering::Acquire)
    }
}

impl Drop for Reactions {
    fn drop(&mut self) {
        self.stop();
    }
}

fn raise(flags: &Arc<(Mutex<Flags>, Condvar)>, settings: bool) {
    let mut state = flags.0.lock().unwrap_or_else(PoisonError::into_inner);
    if settings {
        state.settings = true;
    } else {
        state.sync = true;
    }
    flags.1.notify_all();
}

/// Everything the reaction thread owns. Dropping it releases the subscriptions.
struct Worker {
    flags: Arc<(Mutex<Flags>, Condvar)>,
    _subscriptions: [Subscription; 2],
    settings: Arc<SettingsService>,
    agents: Arc<AgentsService>,
    login_item: Arc<dyn LoginItem>,
    updates: Updates,
    sync_status: Watch<SyncStatus>,
    prefs: Arc<Publisher<Prefs>>,
    login_notice: Arc<Publisher<Option<String>>>,
    checks: Arc<AtomicU32>,
}

impl Worker {
    fn run(self) {
        let mut announced_update = false;
        let mut check: Option<Task<_>> = None;
        // The desired value that already failed. A later settings wake does not try it again.
        let mut login_blocked: Option<bool> = None;
        while let Some((settings_dirty, sync_dirty)) = self.next() {
            if settings_dirty {
                let current = self.settings.watch().get();
                publish_prefs(&self.prefs, &current);
                if let Err(error) = self.agents.apply() {
                    log::warn!("agent access was not applied: {error}");
                }
                apply_login_item(
                    &self.login_item,
                    &self.login_notice,
                    current.launch_at_login,
                    &mut login_blocked,
                );
            }
            if sync_dirty {
                let needs = matches!(*self.sync_status.get(), SyncStatus::NeedsUpdate);
                if needs && !announced_update {
                    announced_update = true;
                    self.checks.fetch_add(1, Ordering::Release);
                    check = Some(self.updates.check(Trigger::Scheduled));
                } else if !needs {
                    announced_update = false;
                }
            }
        }
        drop(check);
    }

    /// Blocks until a flag is raised. `None` once stopped.
    fn next(&self) -> Option<(bool, bool)> {
        let mut state = self.flags.0.lock().unwrap_or_else(PoisonError::into_inner);
        while !state.stop && !state.settings && !state.sync {
            state = self
                .flags
                .1
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if state.stop {
            return None;
        }
        let dirty = (state.settings, state.sync);
        state.settings = false;
        state.sync = false;
        Some(dirty)
    }
}

fn apply_login_item(
    login_item: &Arc<dyn LoginItem>,
    notice: &Publisher<Option<String>>,
    desired: bool,
    blocked: &mut Option<bool>,
) {
    if *blocked == Some(desired) {
        return;
    }
    if let Err(error) = login_item.set_enabled(desired) {
        *blocked = Some(desired);
        // Needs-move, a missing login item, and a failed plist readback all surface once.
        let _ = error;
        let committed = notice.replace(Arc::new(Some(LOGIN_ITEM_UNAVAILABLE.to_owned())));
        committed.notification.notify();
        log::warn!("{LOGIN_ITEM_UNAVAILABLE}");
        return;
    }
    *blocked = None;
    let committed = notice.replace(Arc::new(None));
    committed.notification.notify();
}

fn publish_prefs(prefs: &Publisher<Prefs>, settings: &Settings) {
    let channel = match settings.update_channel {
        UpdateChannel::Stable => Channel::Stable,
        UpdateChannel::Beta => Channel::Beta,
    };
    let committed = prefs.replace(Arc::new(Prefs {
        auto_check: settings.auto_check_updates,
        channel,
    }));
    committed.notification.notify();
}
