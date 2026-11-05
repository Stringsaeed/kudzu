//! IPC commands called from the webviews (see `lib/backend.ts`).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use base64::Engine as _;
use chrono::{Local, Utc};
use kudzu_core::interchange::backup::{BackupInfo, BackupReason};
use kudzu_core::interchange::{coordinator, espanso, kudzu_json};
use kudzu_core::matching::{find_conflicts as core_conflicts, TriggerConflict};
use kudzu_core::template::{self, RenderContext, RenderResult};
use kudzu_core::{GroupRecord, KudzuLibrary, SnippetRecord};
use kudzu_engine::{system, AppInfo, EngineStatus};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::settings::Settings;
use crate::state::{AppCore, EV_NAVIGATE, EV_NEW_SNIPPET};
use crate::{hotkey, tray, windows};

type Core<'a> = State<'a, std::sync::Arc<AppCore>>;
type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatePayload {
    library: KudzuLibrary,
    settings: Settings,
    status: EngineStatus,
    platform: &'static str,
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn status_of(core: &AppCore) -> EngineStatus {
    core.engine().map(|e| e.status()).unwrap_or(EngineStatus {
        can_post_events: false,
        can_expand: false,
        paused: core.store.lock().settings.paused,
        tap: kudzu_engine::TapMode::Off,
        permissions: system::permissions(),
        secure_input: false,
        secure_input_owner: None,
        summary: "Starting…".into(),
    })
}

#[tauri::command]
pub fn get_state(core: Core) -> AppStatePayload {
    let (library, settings) = {
        let s = core.store.lock();
        (s.library.clone(), s.settings.clone())
    };
    AppStatePayload { library, settings, status: status_of(&core), platform: platform() }
}

// MARK: Snippets

#[tauri::command]
pub fn create_snippet(core: Core, draft: serde_json::Value) -> Res<SnippetRecord> {
    let mut snippet = SnippetRecord::new("", vec![], "");
    if let serde_json::Value::Object(map) = draft {
        let mut base = serde_json::to_value(&snippet).map_err(err)?;
        if let serde_json::Value::Object(ref mut b) = base {
            for (k, v) in map {
                if k != "id" && k != "createdAt" && k != "updatedAt" && !v.is_null() {
                    b.insert(k, v);
                }
            }
        }
        snippet = serde_json::from_value(base).map_err(err)?;
    }
    {
        let mut s = core.store.lock();
        s.library.snippets.insert(0, snippet.clone());
    }
    core.library_changed();
    Ok(snippet)
}

#[tauri::command]
pub fn save_snippet(core: Core, mut snippet: SnippetRecord) -> Res<SnippetRecord> {
    {
        let mut s = core.store.lock();
        let Some(existing) = s.library.snippets.iter_mut().find(|x| x.id == snippet.id) else {
            return Err("That snippet no longer exists.".into());
        };
        snippet.created_at = existing.created_at;
        snippet.updated_at = Utc::now();
        snippet.triggers.retain(|t| !t.trim().is_empty());
        *existing = snippet.clone();
    }
    core.library_changed();
    Ok(snippet)
}

#[tauri::command]
pub fn delete_snippets(core: Core, ids: Vec<Uuid>) {
    {
        let ids: HashSet<Uuid> = ids.into_iter().collect();
        let mut s = core.store.lock();
        s.library.snippets.retain(|x| !ids.contains(&x.id));
        let keep: HashSet<Uuid> = s.library.snippets.iter().map(|x| x.id).collect();
        s.form_memory.retain_snippets(&keep);
        s.save_form_memory();
    }
    core.library_changed();
}

#[tauri::command]
pub fn restore_snippets(core: Core, snippets: Vec<SnippetRecord>) {
    {
        let mut s = core.store.lock();
        for snip in snippets {
            if !s.library.snippets.iter().any(|x| x.id == snip.id) {
                s.library.snippets.push(snip);
            }
        }
    }
    core.library_changed();
}

#[tauri::command]
pub fn duplicate_snippet(core: Core, id: Uuid) -> Res<SnippetRecord> {
    let copy = {
        let mut s = core.store.lock();
        let src = s.library.snippets.iter().find(|x| x.id == id).cloned().ok_or("Snippet not found")?;
        let taken: HashSet<String> = s.library.snippets.iter().flat_map(|x| x.triggers.iter().map(|t| t.to_lowercase())).collect();
        let now = Utc::now();
        let triggers = src
            .triggers
            .iter()
            .map(|t| (2..).map(|n| format!("{t}{n}")).find(|c| !taken.contains(&c.to_lowercase())).unwrap())
            .collect();
        let copy = SnippetRecord {
            id: Uuid::new_v4(),
            name: if src.name.is_empty() { String::new() } else { format!("{} copy", src.name) },
            triggers,
            created_at: now,
            updated_at: now,
            ..src.clone()
        };
        let at = s.library.snippets.iter().position(|x| x.id == id).map(|i| i + 1).unwrap_or(0);
        s.library.snippets.insert(at, copy.clone());
        copy
    };
    core.library_changed();
    Ok(copy)
}

#[tauri::command]
pub fn move_snippets(core: Core, ids: Vec<Uuid>, group_id: Option<Uuid>) {
    {
        let ids: HashSet<Uuid> = ids.into_iter().collect();
        let mut s = core.store.lock();
        let now = Utc::now();
        for snip in s.library.snippets.iter_mut().filter(|x| ids.contains(&x.id)) {
            snip.group_id = group_id;
            snip.updated_at = now;
        }
    }
    core.library_changed();
}

#[tauri::command]
pub fn find_conflicts(core: Core) -> Vec<TriggerConflict> {
    let s = core.store.lock();
    core_conflicts(&s.library.snippets, &s.library.groups)
}

// MARK: Groups

#[tauri::command]
pub fn create_group(core: Core, name: String, parent_id: Option<Uuid>) -> GroupRecord {
    let group = {
        let mut s = core.store.lock();
        let sort_index = s.library.groups.iter().filter(|g| g.parent_id == parent_id).map(|g| g.sort_index + 1).max().unwrap_or(0);
        let g = GroupRecord { parent_id, sort_index, ..GroupRecord::new(name) };
        s.library.groups.push(g.clone());
        g
    };
    core.library_changed();
    group
}

fn would_cycle(groups: &[GroupRecord], id: Uuid, parent: Option<Uuid>) -> bool {
    let mut cur = parent;
    let mut seen = HashSet::new();
    while let Some(p) = cur {
        if p == id || !seen.insert(p) {
            return true;
        }
        cur = groups.iter().find(|g| g.id == p).and_then(|g| g.parent_id);
    }
    false
}

#[tauri::command]
pub fn save_group(core: Core, group: GroupRecord) -> Res<GroupRecord> {
    {
        let mut s = core.store.lock();
        if would_cycle(&s.library.groups, group.id, group.parent_id) {
            return Err("A group can't be moved inside itself.".into());
        }
        let g = s.library.groups.iter_mut().find(|g| g.id == group.id).ok_or("Group not found")?;
        *g = group.clone();
    }
    core.library_changed();
    Ok(group)
}

#[tauri::command]
pub fn move_group(core: Core, id: Uuid, parent_id: Option<Uuid>, index: i64) -> Res<()> {
    {
        let mut s = core.store.lock();
        if would_cycle(&s.library.groups, id, parent_id) {
            return Err("A group can't be moved inside itself.".into());
        }
        let mut siblings: Vec<Uuid> = {
            let mut v: Vec<&GroupRecord> = s.library.groups.iter().filter(|g| g.parent_id == parent_id && g.id != id).collect();
            v.sort_by_key(|g| g.sort_index);
            v.into_iter().map(|g| g.id).collect()
        };
        let at = (index.max(0) as usize).min(siblings.len());
        siblings.insert(at, id);
        for g in s.library.groups.iter_mut() {
            if let Some(pos) = siblings.iter().position(|x| *x == g.id) {
                g.sort_index = pos as i64;
                if g.id == id {
                    g.parent_id = parent_id;
                }
            }
        }
    }
    core.library_changed();
    Ok(())
}

#[tauri::command]
pub fn delete_group(core: Core, id: Uuid, delete_snippets: bool) {
    {
        let mut s = core.store.lock();
        let mut doomed: HashSet<Uuid> = HashSet::from([id]);
        loop {
            let before = doomed.len();
            for g in &s.library.groups {
                if g.parent_id.is_some_and(|p| doomed.contains(&p)) {
                    doomed.insert(g.id);
                }
            }
            if doomed.len() == before {
                break;
            }
        }
        s.library.groups.retain(|g| !doomed.contains(&g.id));
        if delete_snippets {
            s.library.snippets.retain(|x| !x.group_id.is_some_and(|g| doomed.contains(&g)));
        } else {
            for snip in s.library.snippets.iter_mut() {
                if snip.group_id.is_some_and(|g| doomed.contains(&g)) {
                    snip.group_id = None;
                }
            }
        }
    }
    core.library_changed();
}

// MARK: Settings & engine

#[tauri::command]
pub fn update_settings(app: AppHandle, core: Core, patch: serde_json::Value) -> Res<Settings> {
    let (old, new) = {
        let mut s = core.store.lock();
        let old = s.settings.clone();
        let mut value = serde_json::to_value(&s.settings).map_err(err)?;
        if let (serde_json::Value::Object(base), serde_json::Value::Object(p)) = (&mut value, patch) {
            for (k, v) in p {
                base.insert(k, v);
            }
        }
        s.settings = serde_json::from_value(value).map_err(err)?;
        (old, s.settings.clone())
    };
    if old.launch_at_login != new.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let r = if new.launch_at_login { app.autolaunch().enable() } else { app.autolaunch().disable() };
        if let Err(e) = r {
            log::warn!("autostart: {e}");
        }
    }
    #[cfg(target_os = "macos")]
    if old.show_in_dock != new.show_in_dock {
        let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        if visible {
            let _ = app.set_activation_policy(if new.show_in_dock { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory });
        }
    }
    if old.icloud_sync != new.icloud_sync {
        if new.icloud_sync {
            crate::cloud::start(&app);
        } else {
            crate::cloud::stop(&app);
        }
    }
    if old.theme != new.theme {
        windows::apply_theme(&app, new.theme);
    }
    core.settings_changed();
    if old.clipboard_enabled != new.clipboard_enabled {
        crate::clipboard::apply_enabled(&app);
        tray::set_clipboard_enabled(&app, new.clipboard_enabled);
    }
    if old.clipboard_retention_days != new.clipboard_retention_days {
        let c = core.inner().clone();
        std::thread::spawn(move || crate::clipboard::prune(&c));
    }
    Ok(new)
}

#[tauri::command]
pub fn set_paused(app: AppHandle, core: Core, paused: bool) -> EngineStatus {
    core.store.lock().settings.paused = paused;
    core.settings_changed();
    let status = status_of(&core);
    let _ = app.emit(crate::state::EV_STATUS, &status);
    tray::update(&app, &status);
    status
}

#[tauri::command]
pub fn engine_status(core: Core) -> EngineStatus {
    status_of(&core)
}

#[tauri::command]
pub fn request_permission(kind: String) {
    match kind.as_str() {
        "accessibility" => system::request_accessibility(),
        _ => system::request_input_monitoring(),
    }
}

#[tauri::command]
pub fn open_system_settings(pane: String) {
    system::open_settings(&pane);
}

#[tauri::command]
pub fn set_palette_hotkey(app: AppHandle, core: Core, accelerator: String) -> Res<Settings> {
    hotkey::register(&app, hotkey::Slot::Palette, &accelerator)?;
    let settings = {
        let mut s = core.store.lock();
        s.settings.palette_hotkey = accelerator.clone();
        s.settings.clone()
    };
    core.settings_changed();
    tray::set_hotkey_hint(&app, &accelerator);
    Ok(settings)
}

// MARK: Templates

#[tauri::command]
pub fn render_preview(core: Core, content: String, snippet_id: Option<Uuid>, form_values: HashMap<String, String>) -> RenderResult {
    let sources: HashMap<Uuid, String> = core.store.lock().library.snippets.iter().map(|s| (s.id, s.content.clone())).collect();
    let resolve = |id: Uuid| sources.get(&id).cloned();
    let ctx = RenderContext {
        now: Local::now().fixed_offset(),
        locale: system::locale(),
        clipboard: system::read_clipboard(),
        form_values,
        resolve_snippet: &resolve,
        root_snippet_id: snippet_id,
    };
    template::render(&content, &ctx)
}

// MARK: Apps

#[tauri::command]
pub async fn list_apps() -> Vec<AppInfo> {
    tauri::async_runtime::spawn_blocking(system::installed_apps).await.unwrap_or_default()
}

#[tauri::command]
pub async fn app_icon(path: String) -> Option<String> {
    let png = tauri::async_runtime::spawn_blocking(move || system::app_icon_png(&path, 64)).await.ok().flatten()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

// MARK: Import / export / backups

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EspansoDetection {
    path: PathBuf,
    snippet_count: usize,
    group_count: usize,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

#[tauri::command]
pub async fn detect_espanso() -> Vec<EspansoDetection> {
    tauri::async_runtime::spawn_blocking(|| {
        coordinator::detect_espanso_dirs(&home())
            .into_iter()
            .filter(|p| p.snippet_count > 0)
            .map(|p| EspansoDetection { path: p.path, snippet_count: p.snippet_count, group_count: p.group_count })
            .collect()
    })
    .await
    .unwrap_or_default()
}

fn apply_import(core: &AppCore, result: Result<(KudzuLibrary, coordinator::ImportSummary), kudzu_core::interchange::InterchangeError>) -> Res<coordinator::ImportSummary> {
    let (library, summary) = result.map_err(err)?;
    core.store.lock().library = library;
    core.library_changed();
    Ok(summary)
}

#[tauri::command]
pub fn import_espanso(core: Core, path: PathBuf) -> Res<coordinator::ImportSummary> {
    let parsed = if path.is_dir() { espanso::import_dir(&path) } else { espanso::import_files(&[path]) }.map_err(err)?;
    let result = {
        let s = core.store.lock();
        coordinator::import_espanso(&s.library, &parsed, &s.backups(), Utc::now())
    };
    apply_import(&core, result)
}

#[tauri::command]
pub fn import_json(core: Core, path: PathBuf) -> Res<coordinator::ImportSummary> {
    let json = std::fs::read_to_string(&path).map_err(err)?;
    let result = {
        let s = core.store.lock();
        coordinator::import_kudzu_json(&s.library, &json, &s.backups(), Utc::now())
    };
    apply_import(&core, result)
}

#[tauri::command]
pub fn export_json(core: Core, path: PathBuf) -> Res<()> {
    let json = kudzu_json::encode(&core.store.lock().library).map_err(err)?;
    std::fs::write(path, json).map_err(err)
}

#[tauri::command]
pub fn export_espanso(core: Core, dir: PathBuf) -> Res<usize> {
    let library = core.store.lock().library.clone();
    coordinator::export_espanso_to_dir(&library, &dir).map(|files| files.len()).map_err(err)
}

#[tauri::command]
pub fn list_backups(core: Core) -> Res<Vec<BackupInfo>> {
    let mut list = core.store.lock().backups().list().map_err(err)?;
    list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    Ok(list)
}

#[tauri::command]
pub fn backup_now(core: Core) -> Res<BackupInfo> {
    let s = core.store.lock();
    let info = s.backups().write(&s.library, BackupReason::Manual, Utc::now()).map_err(err)?;
    let _ = s.backups().prune(14);
    Ok(info)
}

#[tauri::command]
pub fn restore_backup(core: Core, path: PathBuf) -> Res<()> {
    let restored = {
        let s = core.store.lock();
        coordinator::restore_backup(&s.library, &path, &s.backups(), Utc::now()).map_err(err)?
    };
    core.store.lock().library = restored.0;
    core.library_changed();
    Ok(())
}

#[tauri::command]
pub fn reveal_data_folder(app: AppHandle, core: Core) {
    use tauri_plugin_opener::OpenerExt;
    let path = core.store.lock().dir.join("library.json");
    let _ = app.opener().reveal_item_in_dir(path);
}

// MARK: Windows

#[tauri::command]
pub fn show_main(app: AppHandle, snippet_id: Option<Uuid>) {
    windows::hide_palette(&app, false);
    windows::show_main(&app);
    if snippet_id.is_some() {
        let _ = app.emit(EV_NAVIGATE, serde_json::json!({ "snippetId": snippet_id }));
    }
}

#[tauri::command]
pub fn show_main_view(app: AppHandle, view: serde_json::Value) {
    windows::hide_clipboard(&app, false);
    windows::show_main(&app);
    let _ = app.emit(EV_NAVIGATE, serde_json::json!({ "view": view }));
}

#[tauri::command]
pub fn hide_palette(app: AppHandle) {
    windows::hide_palette(&app, true);
}

#[tauri::command]
pub fn palette_insert(app: AppHandle, core: Core, snippet_id: Uuid) {
    windows::hide_palette(&app, false);
    let target = core.palette_target.lock().clone();
    if let Some(engine) = core.engine() {
        engine.insert_snippet(snippet_id, target);
    }
}

#[tauri::command]
pub fn palette_copy(app: AppHandle, core: Core, snippet_id: Uuid) -> Res<()> {
    let engine = core.engine().ok_or("Engine not running")?;
    let text = engine.render_snippet(snippet_id, HashMap::new()).ok_or("Snippet not found")?;
    system::write_clipboard(&text);
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(260));
        let a = app2.clone();
        let _ = app2.run_on_main_thread(move || windows::hide_palette(&a, true));
    });
    Ok(())
}

#[tauri::command]
pub fn palette_create(app: AppHandle, core: Core, query: String) {
    windows::hide_palette(&app, false);
    windows::show_main(&app);
    let prefix = core.store.lock().settings.default_trigger_prefix.clone();
    let looks_like_trigger = !query.is_empty() && !query.contains(' ') && (!prefix.is_empty() && query.starts_with(&prefix));
    let payload = if looks_like_trigger {
        serde_json::json!({ "trigger": query })
    } else {
        serde_json::json!({ "name": query })
    };
    let _ = app.emit(EV_NEW_SNIPPET, payload);
}

#[tauri::command]
pub fn pending_form(core: Core) -> Option<crate::state::FormPayload> {
    core.pending_form.lock().clone()
}

#[tauri::command]
pub fn submit_form(app: AppHandle, core: Core, token: u64, values: HashMap<String, String>) {
    let pending = core.pending_form.lock().take();
    if let Some(p) = &pending {
        let mut s = core.store.lock();
        s.form_memory.record(values.clone(), p.snippet_id);
        s.save_form_memory();
    }
    windows::hide_form(&app);
    if let Some(engine) = core.engine() {
        engine.set_suppressed(false);
        engine.submit_form(token, values);
    }
}

#[tauri::command]
pub fn cancel_form(app: AppHandle, core: Core, token: u64) {
    let pending = core.pending_form.lock().take();
    windows::hide_form(&app);
    if let Some(engine) = core.engine() {
        engine.set_suppressed(false);
        engine.cancel_form(token);
    }
    if let Some(target) = pending.and_then(|p| p.target_app) {
        system::activate_app(&target);
    }
}

#[tauri::command]
pub fn resize_form(app: AppHandle, height: f64) {
    windows::resize_form(&app, height);
}

// ── Updates ──────────────────────────────────────────────────────────────────

/// The running app's version (from tauri.conf.json, i.e. package.json).
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn update_state(app: AppHandle) -> crate::updater::UpdateState {
    crate::updater::state(&app)
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<crate::updater::UpdateState, String> {
    crate::updater::check(app, false).await
}

/// Installs the downloaded update and relaunches.
#[tauri::command]
pub async fn restart_to_update(app: AppHandle) -> Result<(), String> {
    crate::updater::restart_to_update(app).await
}

// ── iCloud sync ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn sync_status(app: AppHandle) -> crate::cloud::SyncStatus {
    crate::cloud::status(&app)
}

#[tauri::command]
pub fn sync_now(app: AppHandle) {
    crate::cloud::sync_now(&app);
}
