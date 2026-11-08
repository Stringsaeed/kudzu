//! Everything below the UI: the Tauri shell's Rust (store, engine host, clipboard history,
//! agents, iCloud, updates, permissions) without Tauri. Windows hear about changes on the
//! core's [`core::Bus`] and ask for window work through [`core::Shell`].

pub mod agent;
pub mod clipboard;
pub mod cloud;
pub mod commands;
pub mod core;
pub mod host;
pub mod hotkey;
pub mod identity;
pub mod logging;
pub mod permissions;
pub mod system;
pub mod updater;
pub mod windows;

use std::sync::Arc;
use std::time::Duration;

use kudzu_engine::Engine;

use self::core::{AppCore, Slot};
use crate::store;

/// Loads the store and the clipboard history and installs the core. Runs before any window.
/// Returns whether this is the first run on this Mac.
pub fn load() -> bool {
    let dir = identity::data_dir();
    let mut store = if identity::get().dev {
        store::Store::load_dev(dir)
    } else {
        store::migrate_legacy_dir(&dir);
        store::Store::load(dir)
    };
    store.note_app_version(&identity::get().version);
    let first_run = store.first_run;
    let clip = clipboard::ClipStore::load(store.dir.join("clipboard"));
    self::core::install(Arc::new(AppCore::new(store, clip)));
    first_run
}

/// Starts the engine and background services, once the UI's shell is installed (Tauri's
/// `setup`, in the same order).
pub fn start() {
    let core = self::core::core();

    let engine = Engine::new(Arc::new(host::Host));
    {
        let store = core.store.lock();
        engine.set_paused(store.settings.paused);
        engine.reload(core.engine_library(&store));
    }
    engine.start();
    *core.engine.lock() = Some(engine);

    let accel = core.store.lock().settings.palette_hotkey.clone();
    if let Err(e) = hotkey::register(Slot::Palette, &accel) {
        log::warn!("palette hotkey: {e}");
    }
    clipboard::apply_enabled();
    clipboard::start();
    agent::apply();
    updater::start();
    cloud::start();
    core.shell().apply_theme(core.store.lock().settings.theme);

    // Daily backups (now, then every 6 h).
    let bg = core.clone();
    std::thread::spawn(move || loop {
        bg.store.lock().daily_backup();
        std::thread::sleep(Duration::from_secs(6 * 3600));
    });
}

/// Quitting: save what's pending and swap in a downloaded update.
pub fn on_exit() {
    clipboard::flush();
    agent::shut_down();
    // Quitting with a downloaded update installs it, so the next launch is current.
    updater::install_on_quit();
}
