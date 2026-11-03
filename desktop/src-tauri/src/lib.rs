//! Kudzu desktop: Tauri shell around kudzu-core (logic) and kudzu-engine (system integration).

mod cloud;
mod commands;
mod host;
mod hotkey;
mod settings;
mod state;
mod store;
mod tray;
mod updater;
mod windows;

use std::sync::Arc;
use std::time::Duration;

use kudzu_engine::Engine;
use parking_lot::Mutex;
use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::state::{core, AppCore};

pub fn run() {
    let started_hidden = std::env::args().any(|a| a == "--hidden");

    tauri::Builder::default()
        .plugin(
            // ~/Library/Logs/sh.saeed.kudzu/Kudzu.log (rotated), plus the terminal in dev.
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info })
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("Kudzu".into()) }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| windows::show_main(app)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        windows::toggle_palette(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let dir = app.path().app_data_dir().expect("app data dir");
            store::migrate_legacy_dir(&dir);
            let mut store = store::Store::load(dir);
            store.note_app_version(&app.package_info().version.to_string());
            let first_run = store.first_run;
            let core = Arc::new(AppCore {
                app: app.handle().clone(),
                store: Mutex::new(store),
                engine: Mutex::new(None),
                palette_target: Mutex::new(None),
                pending_form: Mutex::new(None),
                registered_hotkey: Mutex::new(None),
            });
            app.manage(core.clone());

            // Engine
            let engine = Engine::new(Arc::new(host::Host { app: app.handle().clone() }));
            {
                let store = core.store.lock();
                engine.set_paused(store.settings.paused);
                engine.reload(core.engine_library(&store));
            }
            engine.start();
            *core.engine.lock() = Some(engine);

            // Shell
            tray::create(app.handle())?;
            let accel = core.store.lock().settings.palette_hotkey.clone();
            if let Err(e) = hotkey::register(app.handle(), &accel) {
                log::warn!("palette hotkey: {e}");
            }
            updater::start(app.handle());
            cloud::start(app.handle());
            windows::prewarm(app.handle());
            windows::apply_theme(app.handle(), core.store.lock().settings.theme);

            // Daily backups (now, then every 6 h).
            let bg = core.clone();
            std::thread::spawn(move || loop {
                bg.store.lock().daily_backup();
                std::thread::sleep(Duration::from_secs(6 * 3600));
            });

            if !started_hidden || first_run {
                windows::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                // Closing the window keeps Kudzu running in the menu bar.
                api.prevent_close();
                windows::hide_main(window.app_handle());
            }
            WindowEvent::Focused(false) if window.label() == "palette" => {
                windows::hide_palette(window.app_handle(), false);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::create_snippet,
            commands::save_snippet,
            commands::delete_snippets,
            commands::restore_snippets,
            commands::duplicate_snippet,
            commands::move_snippets,
            commands::find_conflicts,
            commands::create_group,
            commands::save_group,
            commands::move_group,
            commands::delete_group,
            commands::update_settings,
            commands::set_paused,
            commands::engine_status,
            commands::request_permission,
            commands::open_system_settings,
            commands::set_palette_hotkey,
            commands::render_preview,
            commands::list_apps,
            commands::app_icon,
            commands::detect_espanso,
            commands::import_espanso,
            commands::import_json,
            commands::export_json,
            commands::export_espanso,
            commands::list_backups,
            commands::backup_now,
            commands::restore_backup,
            commands::reveal_data_folder,
            commands::show_main,
            commands::hide_palette,
            commands::palette_insert,
            commands::palette_copy,
            commands::palette_create,
            commands::pending_form,
            commands::submit_form,
            commands::cancel_form,
            commands::resize_form,
            commands::app_version,
            commands::update_state,
            commands::check_for_updates,
            commands::restart_to_update,
            commands::sync_status,
            commands::sync_now,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Kudzu")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                windows::show_main(app);
            }
            if let tauri::RunEvent::Exit = &event {
                // Quitting with a downloaded update installs it, so the next launch is current.
                updater::install_on_quit(app);
            }
            if let tauri::RunEvent::ExitRequested { api, code, .. } = &event {
                // Keep running in the menu bar when the last window closes; only explicit Quit exits.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            let _ = (app, core);
        });
}
