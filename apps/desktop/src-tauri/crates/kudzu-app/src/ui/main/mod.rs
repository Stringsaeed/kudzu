//! The main window (`components/app/main/main-app.tsx` with `lib/store.ts`): the toolbar,
//! sidebar, list, editor, clipboard history, insights, settings and onboarding, plus the
//! command menu, shortcuts sheet, group settings and toasts over them.
//!
//! [`MainUi`] is `lib/store.ts`: it mirrors the backend's library, settings and status (kept
//! fresh by bus events), holds view state, and makes mutations optimistic then confirmed, as the
//! zustand store did. Each part of the window keeps its own state in a field
//! (`s.list`, `s.editor`, …) and its view in its own module.

pub mod command_menu;
pub mod frame;
pub mod group_settings;
pub mod insights;
pub mod library;
pub mod onboarding;
pub mod settings;
pub mod shortcuts;
pub mod sidebar;
pub mod snippet_list;
pub mod status;
pub mod toolbar;


use std::collections::HashMap;
use std::sync::Arc;

use kudzu_core::matching::TriggerConflict;
use kudzu_core::{GroupRecord, KudzuLibrary, SnippetRecord};
use kudzu_engine::EngineStatus;
use rune::{AppHandle, Cx, El, KeyEvent, Tx};
use uuid::Uuid;

use crate::backend::cloud::SyncStatus;
use crate::backend::commands;
use crate::backend::core::{Event, NewSnippetDraft};
use crate::backend::updater::UpdateState;
use crate::settings::Settings;
use crate::ui::kit::toast::{Toast, Toasts};
use crate::ui::native::tray::Action;

/// Which page of the main window shows (`View` in lib/store.ts).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    All,
    Favorites,
    Recent,
    Disabled,
    Group(Uuid),
    Insights,
    Clipboard(ClipFilter),
    Settings(Option<SettingsSection>),
}

impl View {
    /// The library views (list + editor) as opposed to full-pane pages.
    pub fn in_library(&self) -> bool {
        !matches!(self, View::Insights | View::Settings(_) | View::Clipboard(_))
    }

    /// `kind` or `kind:arg` as the backend and the web's `?view=` wrote it.
    pub fn parse(s: &str) -> Option<View> {
        let (kind, arg) = s.split_once(':').map_or((s, None), |(k, a)| (k, Some(a)));
        Some(match kind {
            "all" => View::All,
            "favorites" => View::Favorites,
            "recent" => View::Recent,
            "disabled" => View::Disabled,
            "insights" => View::Insights,
            "group" => View::Group(arg?.parse().ok()?),
            "clipboard" => View::Clipboard(arg.and_then(ClipFilter::parse).unwrap_or_default()),
            "settings" => View::Settings(arg.and_then(SettingsSection::parse)),
            _ => return None,
        })
    }
}

/// Clipboard history's sidebar folders (`ClipFilter` in lib/clipboard.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ClipFilter {
    #[default]
    All,
    Pinned,
    Text,
    Image,
    Link,
    File,
    Code,
    Color,
    Other,
}

impl ClipFilter {
    pub fn parse(s: &str) -> Option<ClipFilter> {
        Some(match s {
            "all" => ClipFilter::All,
            "pinned" => ClipFilter::Pinned,
            "text" => ClipFilter::Text,
            "image" => ClipFilter::Image,
            "link" => ClipFilter::Link,
            "file" => ClipFilter::File,
            "code" => ClipFilter::Code,
            "color" => ClipFilter::Color,
            "other" => ClipFilter::Other,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    General,
    Expansion,
    Clipboard,
    Agents,
    Apps,
    Data,
    Shortcuts,
    About,
}

impl SettingsSection {
    pub fn parse(s: &str) -> Option<SettingsSection> {
        Some(match s {
            "general" => SettingsSection::General,
            "expansion" => SettingsSection::Expansion,
            "clipboard" => SettingsSection::Clipboard,
            "agents" => SettingsSection::Agents,
            "apps" => SettingsSection::Apps,
            "data" => SettingsSection::Data,
            "shortcuts" => SettingsSection::Shortcuts,
            "about" => SettingsSection::About,
            _ => return None,
        })
    }
}

/// Where keyboard focus is in the library (`Focus` in lib/store.ts).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    Sidebar,
    #[default]
    List,
    Editor,
}

pub struct MainUi {
    pub me: Option<AppHandle<MainUi>>,
    pub ready: bool,
    pub library: Arc<KudzuLibrary>,
    pub settings: Arc<Settings>,
    pub status: EngineStatus,
    pub usage: HashMap<Uuid, kudzu_core::client::stats::SnippetUsage>,
    pub conflicts: Vec<TriggerConflict>,
    pub update: UpdateState,
    pub sync: SyncStatus,
    pub dev: bool,

    pub view: View,
    pub selected: Option<Uuid>,
    pub command_open: bool,
    pub shortcuts_open: bool,
    /// Snippet whose name field takes focus on the next render (new snippets).
    pub focus_name_of: Option<Uuid>,
    pub sidebar_open: bool,
    pub group_settings: Option<Uuid>,
    pub toasts: Toasts<MainUi>,
    /// The update version a toast has already announced.
    pub announced_update: Option<String>,

    pub sidebar: sidebar::SidebarState,
    pub toolbar: toolbar::ToolbarState,
    pub list: snippet_list::ListState,
    pub editor: crate::ui::editor::EditorState,
    pub clip: crate::ui::clipboard::ClipData,
    pub clip_view: crate::ui::clipboard::view::ClipViewState,
    pub settings_page: settings::SettingsState,
    pub onboarding: onboarding::OnboardingState,
    pub command: command_menu::CommandState,
    pub group_dialog: group_settings::GroupSettingsState,
    pub gallery: crate::ui::kit::gallery::GalleryState,
}

impl MainUi {
    /// `init` in lib/store.ts: the state the backend has now.
    pub fn new(me: Option<AppHandle<MainUi>>) -> MainUi {
        let state = commands::get_state();
        let library = Arc::new(state.library);
        let mut s = MainUi {
            me,
            ready: true,
            usage: kudzu_core::client::stats::usage_by_id(&library),
            selected: library.snippets.first().map(|x| x.id),
            library,
            settings: Arc::new(state.settings),
            status: state.status,
            conflicts: Vec::new(),
            update: crate::backend::updater::state(),
            sync: crate::backend::cloud::status(),
            dev: crate::backend::identity::get().dev,
            view: View::All,
            command_open: false,
            shortcuts_open: false,
            focus_name_of: None,
            sidebar_open: true,
            group_settings: None,
            toasts: Toasts::default(),
            announced_update: None,
            sidebar: sidebar::SidebarState::load(),
            toolbar: Default::default(),
            list: Default::default(),
            editor: Default::default(),
            clip: Default::default(),
            clip_view: Default::default(),
            settings_page: Default::default(),
            onboarding: Default::default(),
            command: Default::default(),
            group_dialog: Default::default(),
            gallery: Default::default(),
        };
        s.refresh_conflicts();
        crate::ui::clipboard::load(&mut s);
        s
    }

    /// `AppSpec::init` placeholder; the app builds its state with [`MainUi::new`] and a handle.
    pub fn init() -> MainUi {
        MainUi::new(None)
    }

    pub fn snippet(&self, id: Uuid) -> Option<&SnippetRecord> {
        self.library.snippets.iter().find(|x| x.id == id)
    }

    pub fn group(&self, id: Uuid) -> Option<&GroupRecord> {
        self.library.groups.iter().find(|g| g.id == id)
    }

    pub fn selected_snippet(&self) -> Option<&SnippetRecord> {
        self.selected.and_then(|id| self.snippet(id))
    }

    fn set_library(&mut self, library: Arc<KudzuLibrary>) {
        self.usage = kudzu_core::client::stats::usage_by_id(&library);
        self.library = library;
        self.refresh_conflicts();
    }

    /// Recomputes trigger conflicts (the web debounced this 250 ms; it costs well under a frame).
    pub fn refresh_conflicts(&mut self) {
        self.conflicts = kudzu_core::matching::find_conflicts(&self.library.snippets, &self.library.groups);
    }

    fn library_mut(&mut self) -> &mut KudzuLibrary {
        Arc::make_mut(&mut self.library)
    }

    /// Replaces a snippet locally (optimistic), before the backend confirms.
    pub fn upsert_local(&mut self, snippet: SnippetRecord) {
        if let Some(x) = self.library_mut().snippets.iter_mut().find(|x| x.id == snippet.id) {
            *x = snippet;
        }
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    /// `newSnippet`: creates a snippet in the current group (a favorite in Favorites), selects
    /// it and focuses its name.
    pub fn new_snippet(&mut self, mut draft: serde_json::Map<String, serde_json::Value>) -> Option<SnippetRecord> {
        if !draft.contains_key("groupID")
            && let View::Group(id) = self.view
        {
            draft.insert("groupID".into(), serde_json::json!(id));
        }
        draft.entry("isFavorite").or_insert(serde_json::json!(self.view == View::Favorites));
        let snippet = commands::create_snippet(serde_json::Value::Object(draft)).ok()?;
        if !self.library.snippets.iter().any(|x| x.id == snippet.id) {
            self.library_mut().snippets.insert(0, snippet.clone());
        }
        self.selected = Some(snippet.id);
        self.focus_name_of = Some(snippet.id);
        self.list.clear_filter();
        if matches!(self.view, View::Insights | View::Settings(_) | View::Clipboard(_) | View::Disabled | View::Recent) {
            self.view = View::All;
        }
        Some(snippet)
    }

    /// The `newSnippet` event (palette, clipboard, tray): adds the trigger prefix.
    pub fn new_snippet_from(&mut self, d: &NewSnippetDraft) {
        let prefix = self.settings.default_trigger_prefix.clone();
        let mut draft = serde_json::Map::new();
        draft.insert("content".into(), serde_json::json!(d.content.clone().unwrap_or_default()));
        draft.insert("name".into(), serde_json::json!(d.name.clone().unwrap_or_default()));
        let triggers: Vec<String> = d.trigger.iter().map(|t| if t.starts_with(&prefix) { t.clone() } else { format!("{prefix}{t}") }).collect();
        draft.insert("triggers".into(), serde_json::json!(triggers));
        self.new_snippet(draft);
    }

    pub fn save_snippet(&mut self, snippet: SnippetRecord) {
        self.upsert_local(snippet.clone());
        if let Ok(saved) = commands::save_snippet(snippet) {
            self.upsert_local(saved);
        }
    }

    /// `deleteSnippets`: removes them (selecting the next row) and returns what was removed,
    /// for Undo.
    pub fn delete_snippets(&mut self, ids: &[Uuid]) -> Vec<SnippetRecord> {
        let removed: Vec<SnippetRecord> = self.library.snippets.iter().filter(|s| ids.contains(&s.id)).cloned().collect();
        let index = self.selected.and_then(|sel| self.library.snippets.iter().position(|s| s.id == sel));
        self.library_mut().snippets.retain(|s| !ids.contains(&s.id));
        if let (Some(sel), Some(i)) = (self.selected, index)
            && ids.contains(&sel)
        {
            let n = self.library.snippets.len();
            self.selected = if n == 0 { None } else { Some(self.library.snippets[i.min(n - 1)].id) };
        }
        commands::delete_snippets(ids.to_vec());
        removed
    }

    pub fn restore_snippets(&mut self, snippets: Vec<SnippetRecord>) {
        let first = snippets.first().map(|s| s.id);
        commands::restore_snippets(snippets);
        if first.is_some() {
            self.selected = first;
        }
    }

    /// Deletes with an "Undo" toast (`requestDelete` in main-app.tsx).
    pub fn request_delete(&mut self, ids: Vec<Uuid>, tx: &mut Tx) {
        if ids.is_empty() {
            return;
        }
        let removed = self.delete_snippets(&ids);
        let label = if removed.len() == 1 {
            let name = &removed[0].name;
            format!("Deleted “{}”", if name.is_empty() { "Untitled" } else { name })
        } else {
            format!("Deleted {} snippets", removed.len())
        };
        let restore = removed.clone();
        self.toasts.push(
            Toast::new(label).duration_ms(6000).action("Undo", move |s: &mut MainUi, _| s.restore_snippets(restore.clone())),
        );
        self.list.focus_list(tx);
    }

    pub fn duplicate(&mut self, id: Uuid) {
        if let Ok(copy) = commands::duplicate_snippet(id) {
            if !self.library.snippets.iter().any(|x| x.id == copy.id) {
                let at = self.library.snippets.iter().position(|x| x.id == id).map_or(0, |i| i + 1);
                self.library_mut().snippets.insert(at, copy.clone());
            }
            self.selected = Some(copy.id);
            self.focus_name_of = Some(copy.id);
        }
    }

    pub fn toggle_favorite(&mut self, id: Uuid) {
        if let Some(mut s) = self.snippet(id).cloned() {
            s.is_favorite = !s.is_favorite;
            self.save_snippet(s);
        }
    }

    pub fn toggle_enabled(&mut self, id: Uuid) {
        if let Some(mut s) = self.snippet(id).cloned() {
            s.is_enabled = !s.is_enabled;
            self.save_snippet(s);
        }
    }

    pub fn move_snippets(&mut self, ids: &[Uuid], group: Option<Uuid>) {
        for s in self.library_mut().snippets.iter_mut().filter(|s| ids.contains(&s.id)) {
            s.group_id = group;
        }
        commands::move_snippets(ids.to_vec(), group);
    }

    pub fn create_group(&mut self, name: &str, parent: Option<Uuid>) -> GroupRecord {
        let group = commands::create_group(name.to_string(), parent);
        if !self.library.groups.iter().any(|g| g.id == group.id) {
            self.library_mut().groups.push(group.clone());
        }
        self.view = View::Group(group.id);
        group
    }

    pub fn save_group(&mut self, group: GroupRecord) {
        if let Some(g) = self.library_mut().groups.iter_mut().find(|g| g.id == group.id) {
            *g = group.clone();
        }
        if let Err(e) = commands::save_group(group) {
            self.toasts.push(Toast::error(e));
        }
    }

    pub fn delete_group(&mut self, id: Uuid, delete_snippets: bool) {
        if self.view == View::Group(id) {
            self.view = View::All;
        }
        commands::delete_group(id, delete_snippets);
    }

    pub fn update_settings(&mut self, patch: serde_json::Value) {
        // Optimistic: merge the patch locally first.
        if let (Ok(serde_json::Value::Object(mut base)), serde_json::Value::Object(p)) = (serde_json::to_value(&*self.settings), patch.clone()) {
            base.extend(p);
            if let Ok(s) = serde_json::from_value(serde_json::Value::Object(base)) {
                self.settings = Arc::new(s);
            }
        }
        match commands::update_settings(patch) {
            Ok(s) => self.settings = Arc::new(s),
            Err(e) => self.toasts.push(Toast::error(e)),
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        Arc::make_mut(&mut self.settings).paused = paused;
        self.status.paused = paused;
        self.status = commands::set_paused(paused);
    }

    /// Runs `work` on a worker thread and lands its result on this window.
    pub fn spawn<T: Send + 'static>(&self, work: impl FnOnce() -> T + Send + 'static, land: impl FnOnce(&mut MainUi, T, &mut Tx) + Send + 'static) {
        let Some(me) = self.me.clone() else { return };
        std::thread::spawn(move || {
            let value = work();
            me.update(move |s, tx| land(s, value, tx));
        });
    }
}

/// Bus events (`on(Events.*)` in lib/store.ts and main-app.tsx). True when anything changed.
pub fn apply(s: &mut MainUi, ev: &Event, _tx: &mut Tx) -> bool {
    match ev {
        Event::Library(library) => s.set_library(library.clone()),
        Event::Settings(settings) => s.settings = settings.clone(),
        Event::Status(status) => s.status = status.clone(),
        Event::Navigate { snippet_id, view } => {
            if let Some(v) = view.as_deref().and_then(View::parse) {
                s.view = v;
            }
            if let Some(id) = snippet_id {
                let group = s.snippet(*id).and_then(|x| x.group_id);
                s.selected = Some(*id);
                s.view = group.map_or(View::All, View::Group);
                s.list.clear_filter();
            }
        }
        Event::NewSnippet(draft) => s.new_snippet_from(draft),
        Event::Update(state) => {
            s.update = state.clone();
            announce_update(s, state);
        }
        Event::Sync(status) => s.sync = status.clone(),
        Event::Clipboard(ev) => crate::ui::clipboard::apply(s, ev),
        Event::PaletteShow { .. } | Event::ClipboardShow { .. } | Event::FormRequest(_) => return false,
    }
    true
}

/// An update finished downloading in the background: mention it once per version.
fn announce_update(s: &mut MainUi, state: &UpdateState) {
    let UpdateState::Ready { version, .. } = state else { return };
    if s.announced_update.as_deref() == Some(version.as_str()) {
        return;
    }
    s.announced_update = Some(version.clone());
    if matches!(s.view, View::Settings(_)) {
        return; // the About page already shows it
    }
    s.toasts.push(
        Toast::new(format!("Kudzu {version} is ready"))
            .description("It installs when you quit, or restart now.")
            .duration_ms(15000)
            .action("Restart", |_, _| {
                let _ = commands::restart_to_update();
            })
            .cancel("What's new", |s: &mut MainUi, _| s.view = View::Settings(Some(SettingsSection::About))),
    );
}

/// The tray's menu, on the main thread.
pub fn tray_action(action: Action) {
    use crate::backend::{core::core, windows};
    match action {
        Action::Pause => {
            let core = core();
            let paused = {
                let mut st = core.store.lock();
                st.settings.paused = !st.settings.paused;
                st.settings.paused
            };
            core.settings_changed();
            if let Some(engine) = core.engine() {
                engine.set_paused(paused);
                let status = engine.status();
                core.emit(Event::Status(status.clone()));
                crate::ui::native::tray::update(&status);
            }
        }
        Action::Search => windows::toggle_palette(),
        Action::Clipboard => windows::toggle_clipboard(),
        Action::FromSelection => {
            std::thread::spawn(|| {
                let text = kudzu_engine::system::copy_selection().unwrap_or_default();
                windows::show_main();
                core().emit(Event::NewSnippet(NewSnippetDraft { content: Some(text), ..Default::default() }));
            });
        }
        Action::Open => windows::show_main(),
        Action::Settings => {
            windows::show_main();
            core().emit(Event::Navigate { snippet_id: None, view: Some("settings".into()) });
        }
        Action::Updates => match crate::backend::updater::state() {
            // Downloaded and verified: this item reads "Restart to Update to x.y.z".
            UpdateState::Ready { .. } => {
                let _ = crate::backend::updater::restart_to_update();
            }
            state => {
                windows::show_main();
                core().emit(Event::Navigate { snippet_id: None, view: Some("settings:about".into()) });
                if !matches!(state, UpdateState::Downloading { .. } | UpdateState::Checking) {
                    std::thread::spawn(|| crate::backend::updater::check(false));
                }
            }
        },
        Action::Quit => {
            if let Some(engine) = core().engine() {
                engine.stop();
            }
            core().shell().quit();
        }
    }
}

/// The web editor saved on blur: losing the window saves a pending edit at once.
pub fn on_focus(s: &mut MainUi, focused: bool, _tx: &mut Tx) {
    if !focused {
        crate::ui::editor::flush(s);
    }
}

/// The window's shortcuts (`useHotkeys` in main-app.tsx), for keys the focused control passed on.
pub fn on_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    frame::on_key(s, ev, tx)
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    frame::view(s, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_parse_like_the_web_query_param() {
        assert_eq!(View::parse("insights"), Some(View::Insights));
        assert_eq!(View::parse("settings:apps"), Some(View::Settings(Some(SettingsSection::Apps))));
        assert_eq!(View::parse("settings"), Some(View::Settings(None)));
        assert_eq!(View::parse("clipboard:image"), Some(View::Clipboard(ClipFilter::Image)));
        assert_eq!(View::parse("clipboard"), Some(View::Clipboard(ClipFilter::All)));
        assert_eq!(View::parse("nope"), None);
    }
}
