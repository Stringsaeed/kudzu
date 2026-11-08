//! The app's commands. Ported from the Tauri shell's `commands.rs` (the IPC surface
//! `lib/backend.ts` called); the logic is unchanged, and Rune views call these directly.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::{Local, Utc};
use kudzu_core::interchange::backup::{BackupInfo, BackupReason};
use kudzu_core::interchange::{coordinator, espanso, kudzu_json};
use kudzu_core::template::{self, RenderContext, RenderResult};
use kudzu_core::{GroupRecord, KudzuLibrary, SnippetRecord};
use kudzu_engine::{system, AppInfo, EngineStatus};
use serde::Serialize;
use uuid::Uuid;

use super::core::{core, AppCore, Event, NewSnippetDraft, Slot, TrayUpdate};
use super::{hotkey, windows};
use crate::settings::Settings;
type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppStatePayload {
    pub library: KudzuLibrary,
    pub settings: Settings,
    pub status: EngineStatus,
    pub platform: &'static str,
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

pub fn status_of(core: &AppCore) -> EngineStatus {
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

pub fn get_state() -> AppStatePayload {
    let core = core();
    let (library, settings) = {
        let s = core.store.lock();
        (s.library.clone(), s.settings.clone())
    };
    AppStatePayload { library, settings, status: status_of(&core), platform: platform() }
}

// MARK: Snippets

pub fn create_snippet(draft: serde_json::Value) -> Res<SnippetRecord> {
    let core = core();
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

pub fn save_snippet(mut snippet: SnippetRecord) -> Res<SnippetRecord> {
    let core = core();
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

pub fn delete_snippets(ids: Vec<Uuid>) {
    let core = core();
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

pub fn restore_snippets(snippets: Vec<SnippetRecord>) {
    let core = core();
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

pub fn duplicate_snippet(id: Uuid) -> Res<SnippetRecord> {
    let core = core();
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

pub fn move_snippets(ids: Vec<Uuid>, group_id: Option<Uuid>) {
    let core = core();
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

// MARK: Groups

pub fn create_group(name: String, parent_id: Option<Uuid>) -> GroupRecord {
    let core = core();
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

pub fn save_group(group: GroupRecord) -> Res<GroupRecord> {
    let core = core();
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

pub fn delete_group(id: Uuid, delete_snippets: bool) {
    let core = core();
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

pub fn update_settings(patch: serde_json::Value) -> Res<Settings> {
    let core = core();
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
    if old.launch_at_login != new.launch_at_login
        && let Err(e) = super::system::set_launch_at_login(new.launch_at_login) {
            log::warn!("autostart: {e}");
        }
    #[cfg(target_os = "macos")]
    if old.show_in_dock != new.show_in_dock
        && core.shell().is_visible(super::core::Win::Main) {
            core.shell().set_dock_icon(new.show_in_dock);
        }
    if old.icloud_sync != new.icloud_sync {
        if new.icloud_sync {
            super::cloud::start();
        } else {
            super::cloud::stop();
        }
    }
    if old.theme != new.theme {
        core.shell().apply_theme(new.theme);
    }
    core.settings_changed();
    if old.clipboard_enabled != new.clipboard_enabled {
        super::clipboard::apply_enabled();
        core.shell().tray(TrayUpdate::ClipboardEnabled(new.clipboard_enabled));
    }
    if old.agent_access != new.agent_access {
        super::agent::apply();
    }
    if old.clipboard_retention_days != new.clipboard_retention_days {
        let c = core.clone();
        std::thread::spawn(move || super::clipboard::prune(&c));
    }
    Ok(new)
}

pub fn set_paused(paused: bool) -> EngineStatus {
    let core = core();
    core.store.lock().settings.paused = paused;
    core.settings_changed();
    let status = status_of(&core);
    core.emit(Event::Status(status.clone()));
    core.shell().tray(TrayUpdate::Status(status.clone()));
    status
}

pub fn engine_status() -> EngineStatus {
    let core = core();
    status_of(&core)
}

pub fn set_palette_hotkey(accelerator: String) -> Res<Settings> {
    let core = core();
    hotkey::register(Slot::Palette, &accelerator)?;
    let settings = {
        let mut s = core.store.lock();
        s.settings.palette_hotkey = accelerator.clone();
        s.settings.clone()
    };
    core.settings_changed();
    core.shell().tray(TrayUpdate::PaletteHint(accelerator.clone()));
    Ok(settings)
}

// MARK: Templates

pub fn render_preview(content: String, snippet_id: Option<Uuid>, form_values: HashMap<String, String>, formatting: Option<kudzu_core::formatting::Formatting>) -> RenderResult {
    let snippets: HashMap<_, _> = core().store.lock().library.snippets.iter().map(|s| (s.id, s.clone())).collect();
    let resolve = |id: Uuid| snippets.get(&id).map(|s| s.content.clone());
    let resolve_formatting = |id| snippets.get(&id).and_then(|s| s.formatting.clone());
    let ctx = RenderContext {
        resolve_formatting: Some(&resolve_formatting),
        now: Local::now().fixed_offset(),
        locale: system::locale(),
        clipboard: system::read_clipboard(),
        form_values,
        resolve_snippet: &resolve,
        root_snippet_id: snippet_id,
    };
    template::render_formatted(&content, formatting.as_ref(), &ctx)
}

pub fn copy_preview(content: String, snippet_id: Option<Uuid>, formatting: Option<kudzu_core::formatting::Formatting>) {
    let result = render_preview(content, snippet_id, HashMap::new(), formatting);
    copy_rendered(result);
}

fn copy_rendered(result: RenderResult) {
    let mut item = vec![("public.utf8-plain-text".into(), result.text.as_bytes().to_vec())];
    if let Some(spans) = &result.formatting {
        item.push(("public.html".into(), kudzu_core::formatting::html(&result.text, spans).into_bytes()));
    }
    kudzu_engine::clipboard::write(&[item]);
}

// MARK: Apps

/// Blocking: call it off the UI thread.
pub fn list_apps() -> Vec<AppInfo> {
    system::installed_apps()
}

/// The app's icon as PNG bytes. Blocking: call it off the UI thread.
pub fn app_icon(path: String) -> Option<Vec<u8>> {
    system::app_icon_png(&path, 64)
}

// MARK: Import / export / backups

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EspansoDetection {
    pub path: PathBuf,
    pub snippet_count: usize,
    pub group_count: usize,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

/// Blocking: call it off the UI thread.
pub fn detect_espanso() -> Vec<EspansoDetection> {
    coordinator::detect_espanso_dirs(&home())
        .into_iter()
        .filter(|p| p.snippet_count > 0)
        .map(|p| EspansoDetection { path: p.path, snippet_count: p.snippet_count, group_count: p.group_count })
        .collect()
}

fn apply_import(core: &AppCore, result: Result<(KudzuLibrary, coordinator::ImportSummary), kudzu_core::interchange::InterchangeError>) -> Res<coordinator::ImportSummary> {
    let (library, summary) = result.map_err(err)?;
    core.store.lock().library = library;
    core.library_changed();
    Ok(summary)
}

pub fn import_espanso(path: PathBuf) -> Res<coordinator::ImportSummary> {
    let core = core();
    let parsed = if path.is_dir() { espanso::import_dir(&path) } else { espanso::import_files(&[path]) }.map_err(err)?;
    let result = {
        let s = core.store.lock();
        coordinator::import_espanso(&s.library, &parsed, &s.backups(), Utc::now())
    };
    apply_import(&core, result)
}

/// Imports a file or folder from any supported app, detected from its extension and contents:
/// Kudzu or Raycast `.json`, Espanso `.yml` or a folder, TextExpander `.textexpander` or `.csv`.
pub fn import_file(path: PathBuf) -> Res<coordinator::ImportSummary> {
    let core = core();
    let parsed = coordinator::load_import(&path).map_err(err)?;
    let result = {
        let s = core.store.lock();
        coordinator::import_result(&s.library, &parsed, &s.backups(), Utc::now())
    };
    apply_import(&core, result)
}

pub fn export_json(path: PathBuf) -> Res<()> {
    let core = core();
    let json = kudzu_json::encode(&core.store.lock().library).map_err(err)?;
    std::fs::write(path, json).map_err(err)
}

pub fn export_espanso(dir: PathBuf) -> Res<usize> {
    let core = core();
    let library = core.store.lock().library.clone();
    coordinator::export_espanso_to_dir(&library, &dir).map(|files| files.len()).map_err(err)
}

pub fn list_backups() -> Res<Vec<BackupInfo>> {
    let core = core();
    let mut list = core.store.lock().backups().list().map_err(err)?;
    list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    Ok(list)
}

pub fn backup_now() -> Res<BackupInfo> {
    let core = core();
    let s = core.store.lock();
    let info = s.backups().write(&s.library, BackupReason::Manual, Utc::now()).map_err(err)?;
    let _ = s.backups().prune(14);
    Ok(info)
}

pub fn restore_backup(path: PathBuf) -> Res<()> {
    let core = core();
    let restored = {
        let s = core.store.lock();
        coordinator::restore_backup(&s.library, &path, &s.backups(), Utc::now()).map_err(err)?
    };
    core.store.lock().library = restored.0;
    core.library_changed();
    Ok(())
}

pub fn reveal_data_folder() {
    let path = core().store.lock().dir.join("library.json");
    let _ = super::system::reveal(path);
}

// MARK: Windows

pub fn show_main(snippet_id: Option<Uuid>) {
    windows::hide_palette(false);
    windows::show_main();
    if snippet_id.is_some() {
        core().emit(Event::Navigate { snippet_id, view: None });
    }
}

/// `view` is `kind` or `kind:arg` (`settings:about`, `clipboard:image`).
pub fn show_main_view(view: String) {
    windows::hide_clipboard(false);
    windows::show_main();
    core().emit(Event::Navigate { snippet_id: None, view: Some(view) });
}

pub fn hide_palette() {
    windows::hide_palette(true);
}

pub fn palette_insert(snippet_id: Uuid) {
    let core = core();
    windows::hide_palette(false);
    let target = core.palette_target.lock().clone();
    if let Some(engine) = core.engine() {
        engine.insert_snippet(snippet_id, target);
    }
}

pub fn palette_copy(snippet_id: Uuid) -> Res<()> {
    let engine = core().engine().ok_or("Engine not running")?;
    let result = engine.render_snippet_formatted(snippet_id, HashMap::new()).ok_or("Snippet not found")?;
    copy_rendered(result);
    // Long enough for the palette's "Copied" confirmation to show.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(260));
        windows::hide_palette(true);
    });
    Ok(())
}

pub fn palette_create(query: String) {
    let core = core();
    windows::hide_palette(false);
    windows::show_main();
    let prefix = core.store.lock().settings.default_trigger_prefix.clone();
    let looks_like_trigger = !query.is_empty() && !query.contains(' ') && (!prefix.is_empty() && query.starts_with(&prefix));
    let draft = if looks_like_trigger {
        NewSnippetDraft { trigger: Some(query), ..Default::default() }
    } else {
        NewSnippetDraft { name: Some(query), ..Default::default() }
    };
    core.emit(Event::NewSnippet(draft));
}

pub fn pending_form() -> Option<super::core::FormPayload> {
    core().pending_form.lock().clone()
}

pub fn submit_form(token: u64, values: HashMap<String, String>) {
    let core = core();
    let pending = core.pending_form.lock().take();
    if let Some(p) = &pending {
        let mut s = core.store.lock();
        s.form_memory.record(values.clone(), p.snippet_id);
        s.save_form_memory();
    }
    windows::hide_form();
    if let Some(engine) = core.engine() {
        engine.set_suppressed(false);
        engine.submit_form(token, values);
    }
}

pub fn cancel_form(token: u64) {
    let core = core();
    let pending = core.pending_form.lock().take();
    windows::hide_form();
    if let Some(engine) = core.engine() {
        engine.set_suppressed(false);
        engine.cancel_form(token);
    }
    if let Some(target) = pending.and_then(|p| p.target_app) {
        system::activate_app(&target);
    }
}

// ── Updates ──────────────────────────────────────────────────────────────────

/// The running app's version (from package.json).
pub fn app_version() -> String {
    super::identity::get().version.clone()
}

/// Blocking (network): call it off the UI thread.
pub fn check_for_updates() -> Result<super::updater::UpdateState, String> {
    super::updater::check(false)
}

/// Installs the downloaded update and relaunches.
pub fn restart_to_update() -> Result<(), String> {
    super::updater::restart_to_update()
}

// ── iCloud sync ──────────────────────────────────────────────────────────────

pub fn sync_now() {
    super::cloud::sync_now();
}
