//! Main window navigation state, history stack, and pane focus (`PLAN.md` S41).

use uuid::Uuid;

use crate::focus::Focus;
use crate::motion::Arrivals;
use crate::shell::{Route, SettingsPane};

use super::toasts::Toasts;

/// Keyboard-navigable panes in the main window (`Focus<MainPane>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainPane {
    Sidebar,
    List,
    Editor,
}

pub const LIBRARY_PANES: &[MainPane] = &[MainPane::Sidebar, MainPane::List, MainPane::Editor];
pub const TWO_PANES: &[MainPane] = &[MainPane::Sidebar, MainPane::List];
pub const SINGLE_PANE: &[MainPane] = &[MainPane::List];

/// Filter applied to the Library view (`⌘1–4` or a specific group).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LibraryFilter {
    #[default]
    All,
    Favorites,
    Recent,
    Disabled,
    Group(Uuid),
}

/// The filter a Clipboard view shows. It is the core's filter, so the sidebar folders (S42),
/// the history list (S58) and the picker (S60) agree on what each folder holds.
pub use kudzu_core::clipboard::query::ClipFilter;

/// The clipboard folders in sidebar order, as in Tauri's `FILTERS` (`lib/clipboard.ts`).
/// `Other` has no folder of its own: Text covers it.
pub const CLIP_FOLDERS: [ClipFilter; 8] = [
    ClipFilter::All,
    ClipFilter::Pinned,
    ClipFilter::Text,
    ClipFilter::Image,
    ClipFilter::Link,
    ClipFilter::File,
    ClipFilter::Code,
    ClipFilter::Color,
];

/// A folder's on-screen name: "History" for everything, otherwise Tauri's `FILTERS` labels.
pub const fn clip_filter_label(filter: ClipFilter) -> &'static str {
    match filter {
        ClipFilter::All => "History",
        ClipFilter::Pinned => "Pinned",
        ClipFilter::Text => "Text",
        ClipFilter::Image => "Images",
        ClipFilter::Link => "Links",
        ClipFilter::File => "Files",
        ClipFilter::Code => "Code",
        ClipFilter::Color => "Colors",
        ClipFilter::Other => "Other",
    }
}

/// A stable key for a folder, used in element keys and crumb ids (`clip:<key>`).
pub const fn clip_filter_key(filter: ClipFilter) -> &'static str {
    match filter {
        ClipFilter::All => "all",
        ClipFilter::Pinned => "pinned",
        ClipFilter::Text => "text",
        ClipFilter::Image => "image",
        ClipFilter::Link => "link",
        ClipFilter::File => "file",
        ClipFilter::Code => "code",
        ClipFilter::Color => "color",
        ClipFilter::Other => "other",
    }
}

/// Top-level main window route (`PLAN.md` S41 + first-run `Onboarding`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MainView {
    Library(LibraryFilter),
    Insights,
    Clipboard(ClipFilter),
    Settings(SettingsPane),
    Onboarding,
}

impl Default for MainView {
    fn default() -> Self {
        Self::Library(LibraryFilter::All)
    }
}

impl MainView {
    pub const fn in_library(&self) -> bool {
        matches!(self, Self::Library(_))
    }

    pub const fn area(&self) -> crate::actions::main::ScreenArea {
        use crate::actions::main::ScreenArea;
        match self {
            Self::Library(_) => ScreenArea::Library,
            Self::Insights => ScreenArea::Insights,
            Self::Clipboard(_) => ScreenArea::Clipboard,
            Self::Settings(_) => ScreenArea::Settings,
            Self::Onboarding => ScreenArea::Onboarding,
        }
    }

    pub fn pane_order(&self, sidebar_open: bool) -> &'static [MainPane] {
        match self {
            Self::Onboarding => SINGLE_PANE,
            Self::Library(_) if sidebar_open => LIBRARY_PANES,
            Self::Library(_) => &[MainPane::List, MainPane::Editor],
            Self::Insights | Self::Clipboard(_) | Self::Settings(_) if sidebar_open => TWO_PANES,
            Self::Insights | Self::Clipboard(_) | Self::Settings(_) => SINGLE_PANE,
        }
    }
}

/// Human-readable label for a [`SettingsPane`] (used in breadcrumbs and headings).
pub const fn settings_pane_label(pane: SettingsPane) -> &'static str {
    match pane {
        SettingsPane::General => "General",
        SettingsPane::Expansion => "Expansion",
        SettingsPane::Apps => "Apps",
        SettingsPane::Clipboard => "Clipboard",
        SettingsPane::Agents => "Agents",
        SettingsPane::Data => "Data & Sync",
        SettingsPane::Activity => "Activity",
        SettingsPane::About => "About",
    }
}

/// Bounded back/forward navigation stack (`⌘[` / `⌘]`, capped at 64 entries).
pub const MAX_NAV_HISTORY: usize = 64;

#[derive(Clone, Debug)]
pub struct NavHistory {
    entries: Vec<MainView>,
    index: usize,
}

impl Default for NavHistory {
    fn default() -> Self {
        Self::new(MainView::default())
    }
}

impl NavHistory {
    pub fn new(initial: MainView) -> Self {
        Self {
            entries: vec![initial],
            index: 0,
        }
    }

    pub fn current(&self) -> &MainView {
        &self.entries[self.index]
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }

    /// Pushes `next` if distinct from the current entry, truncating any forward branch and keeping
    /// at most [`MAX_NAV_HISTORY`] entries.
    pub fn push(&mut self, next: MainView) -> bool {
        if self.entries[self.index] == next {
            return false;
        }
        self.entries.truncate(self.index + 1);
        self.entries.push(next);
        if self.entries.len() > MAX_NAV_HISTORY {
            let overflow = self.entries.len() - MAX_NAV_HISTORY;
            self.entries.drain(0..overflow);
        }
        self.index = self.entries.len() - 1;
        true
    }

    /// Replaces the current entry in place without adding a back-stack step (e.g. onboarding ->
    /// library transition).
    pub fn replace_current(&mut self, view: MainView) {
        self.entries[self.index] = view;
    }

    pub fn go_back(&mut self) -> Option<MainView> {
        if !self.can_go_back() {
            return None;
        }
        self.index -= 1;
        Some(self.entries[self.index].clone())
    }

    pub fn go_forward(&mut self) -> Option<MainView> {
        if !self.can_go_forward() {
            return None;
        }
        self.index += 1;
        Some(self.entries[self.index].clone())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Frame-owned UI state for the `main` window (`PLAN.md` S41).
pub struct MainFrame<S: 'static = crate::shell::MainState> {
    pub view: MainView,
    pub history: NavHistory,
    pub sidebar_open: bool,
    pub focus: Focus<MainPane>,
    pub selected_snippet: Option<Uuid>,
    /// Modal flags coordinated at window level (`Escape`, shortcut blocking).
    pub command_open: bool,
    pub shortcuts_open: bool,
    pub group_settings_id: Option<Uuid>,
    pub breadcrumb_overflow_open: bool,
    /// Monotonic UI clock (seconds) updated on poll for toast timers and breadcrumb burst checks.
    pub clock_secs: f64,
    /// Entrance counter for the leaf breadcrumb (`Arrivals` skips mount, renames, and `< 300ms` bursts).
    pub breadcrumb_arrivals: Arrivals,
    /// Last drawn `[x, y, width, height]` of the sidebar slot fed to `SidebarMaterial::sync`, or
    /// `None` when Onboarding hides the chrome.
    pub sidebar_material_rect: Option<[f64; 4]>,
    /// Version string of the last ready update announced as a toast (`SHL-10`).
    pub announced_update: Option<String>,
    /// A route `Shell::open_main` asked for that the next poll applies. Taken once, so the
    /// same route sent twice applies twice.
    pub pending_route: Option<Route>,
    /// Sonner-style toast host.
    pub toasts: Toasts<S>,
}

impl<S: 'static> MainFrame<S> {
    pub fn new(initial: MainView) -> Self {
        let mut focus = Focus::new(LIBRARY_PANES);
        focus.move_to(MainPane::List);
        let mut frame = Self {
            history: NavHistory::new(initial.clone()),
            view: initial,
            sidebar_open: true,
            focus,
            selected_snippet: None,
            command_open: false,
            shortcuts_open: false,
            group_settings_id: None,
            breadcrumb_overflow_open: false,
            clock_secs: 0.0,
            breadcrumb_arrivals: Arrivals::default(),
            sidebar_material_rect: None,
            announced_update: None,
            pending_route: None,
            toasts: Toasts::new(),
        };
        frame.sync_focus_order();
        frame
    }

    /// True if any window-level modal overlay is currently open.
    pub fn any_modal_open(&self) -> bool {
        self.command_open
            || self.shortcuts_open
            || self.group_settings_id.is_some()
            || self.breadcrumb_overflow_open
    }

    /// Closes the topmost open modal overlay. Returns `true` if one was dismissed.
    pub fn close_top_modal(&mut self) -> bool {
        if self.breadcrumb_overflow_open {
            self.breadcrumb_overflow_open = false;
            return true;
        }
        if self.command_open {
            self.command_open = false;
            return true;
        }
        if self.shortcuts_open {
            self.shortcuts_open = false;
            return true;
        }
        if self.group_settings_id.is_some() {
            self.group_settings_id = None;
            return true;
        }
        false
    }

    /// Synchronizes `Focus<MainPane>` order when `view` or `sidebar_open` changes so a hidden
    /// sidebar or non-Library editor pane never retains keyboard focus.
    pub fn sync_focus_order(&mut self) {
        let active = self.focus.active();
        let order = self.view.pane_order(self.sidebar_open);
        let fallback = if order.contains(&active) {
            active
        } else if order.contains(&MainPane::List) {
            MainPane::List
        } else {
            order[0]
        };
        let mut next = Focus::new(order);
        next.move_to(fallback);
        self.focus = next;
    }

    /// Navigates to `target`, pushing onto [`NavHistory`] unless already on `target`.
    pub fn navigate(&mut self, target: MainView) -> bool {
        let changed = self.history.push(target.clone());
        self.view = target;
        self.breadcrumb_overflow_open = false;
        if matches!(self.view, MainView::Onboarding) {
            self.sidebar_material_rect = None;
        }
        self.sync_focus_order();
        changed
    }

    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.history.go_back() {
            self.view = prev;
            self.breadcrumb_overflow_open = false;
            self.sync_focus_order();
            true
        } else {
            false
        }
    }

    pub fn go_forward(&mut self) -> bool {
        if let Some(next) = self.history.go_forward() {
            self.view = next;
            self.breadcrumb_overflow_open = false;
            self.sync_focus_order();
            true
        } else {
            false
        }
    }
}
