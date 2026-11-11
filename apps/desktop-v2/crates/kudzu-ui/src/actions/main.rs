//! Main window shortcut table (`LIB-14`, `PLAN.md` §1.2, S41).
//!
//! Append-only action registry implementing [`crate::actions::Action`] for [`MainState`].
//! Context guards enforce `LIB-14` (`PLAN.md` S41):
//! - Shortcuts do nothing during IME composition (`composing`).
//! - Open modal dialogs (`command_open`, `shortcuts_open`, `group_settings_id`) capture keys.
//! - Plain `?` (`shift+/`) and `⌘⌫` (`mod+backspace`) only fire outside editable text inputs,
//!   while command chords (`⌘N`, `⌘F`, `⌘L`, `⌘E`, `⌘1–5`, `⌘,`, `⌘/`, `⌘K`, `⌘⇧P`, `⌘D`,
//!   `⌘⇧F`, `⌘⇧E`, `⌘[`, `⌘]`, `⌘\`) work inside inputs too (`allowInInput: true`).

use rune::{Key, KeyEvent, Keybind, Tx};

use super::Action;
use crate::shell::MainState;

/// Active top-level screen area for scoping area-specific shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenArea {
    Library,
    Insights,
    Clipboard,
    Settings,
    Onboarding,
}

/// Snapshot of window context used to guard shortcut resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionContext {
    pub area: ScreenArea,
    pub has_selected_snippet: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub in_text_input: bool,
    pub modal_open: bool,
    pub composing: bool,
}

/// Every window-level shortcut in the main window (`LIB-14`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainAction {
    CommandMenu,
    NewSnippet,
    NewGroup,
    FocusFilter,
    FocusList,
    EditContent,
    DuplicateSnippet,
    ToggleFavorite,
    ToggleEnabled,
    DeleteSnippet,
    TogglePause,
    ViewAll,
    ViewFavorites,
    ViewRecent,
    ViewInsights,
    ViewClipboard,
    OpenSettings,
    OpenShortcuts,
    Back,
    Forward,
    ToggleSidebar,
}

impl MainAction {
    pub const ALL: &'static [MainAction] = &[
        MainAction::CommandMenu,
        MainAction::NewSnippet,
        MainAction::NewGroup,
        MainAction::FocusFilter,
        MainAction::FocusList,
        MainAction::EditContent,
        MainAction::DuplicateSnippet,
        MainAction::ToggleFavorite,
        MainAction::ToggleEnabled,
        MainAction::DeleteSnippet,
        MainAction::TogglePause,
        MainAction::ViewAll,
        MainAction::ViewFavorites,
        MainAction::ViewRecent,
        MainAction::ViewInsights,
        MainAction::ViewClipboard,
        MainAction::OpenSettings,
        MainAction::OpenShortcuts,
        MainAction::Back,
        MainAction::Forward,
        MainAction::ToggleSidebar,
    ];

    /// Whether this shortcut fires even while typing in a focused text input (`allowInInput`).
    /// Note that `OpenShortcuts` allows `⌘/` in inputs, while bare `?` is guarded separately.
    pub fn allow_in_input(self) -> bool {
        match self {
            MainAction::DeleteSnippet => false,
            MainAction::CommandMenu
            | MainAction::NewSnippet
            | MainAction::NewGroup
            | MainAction::FocusFilter
            | MainAction::FocusList
            | MainAction::EditContent
            | MainAction::DuplicateSnippet
            | MainAction::ToggleFavorite
            | MainAction::ToggleEnabled
            | MainAction::TogglePause
            | MainAction::ViewAll
            | MainAction::ViewFavorites
            | MainAction::ViewRecent
            | MainAction::ViewInsights
            | MainAction::ViewClipboard
            | MainAction::OpenSettings
            | MainAction::OpenShortcuts
            | MainAction::Back
            | MainAction::Forward
            | MainAction::ToggleSidebar => true,
        }
    }

    /// Whether this action is enabled in the given screen & selection state.
    pub fn enabled_in(self, ctx: ActionContext) -> bool {
        let in_library = ctx.area == ScreenArea::Library;
        let in_onboarding = ctx.area == ScreenArea::Onboarding;
        match self {
            MainAction::FocusList => in_library || ctx.area == ScreenArea::Clipboard,
            MainAction::EditContent
            | MainAction::DuplicateSnippet
            | MainAction::ToggleFavorite
            | MainAction::ToggleEnabled
            | MainAction::DeleteSnippet => in_library && ctx.has_selected_snippet,
            MainAction::Back => !in_onboarding && ctx.can_back,
            MainAction::Forward => !in_onboarding && ctx.can_forward,
            MainAction::ToggleSidebar => !in_onboarding,
            MainAction::CommandMenu
            | MainAction::NewSnippet
            | MainAction::NewGroup
            | MainAction::FocusFilter
            | MainAction::TogglePause
            | MainAction::ViewAll
            | MainAction::ViewFavorites
            | MainAction::ViewRecent
            | MainAction::ViewInsights
            | MainAction::ViewClipboard
            | MainAction::OpenSettings
            | MainAction::OpenShortcuts => !in_onboarding,
        }
    }

    /// Returns the raw keybind specification string for `Keybind::new`.
    pub const fn chord(self) -> &'static str {
        match self {
            MainAction::CommandMenu => "mod+k",
            MainAction::NewSnippet => "mod+n",
            MainAction::NewGroup => "mod+shift+n",
            MainAction::FocusFilter => "mod+f",
            MainAction::FocusList => "mod+l",
            MainAction::EditContent => "mod+e",
            MainAction::DuplicateSnippet => "mod+d",
            MainAction::ToggleFavorite => "mod+shift+f",
            MainAction::ToggleEnabled => "mod+shift+e",
            MainAction::DeleteSnippet => "mod+backspace",
            MainAction::TogglePause => "mod+shift+p",
            MainAction::ViewAll => "mod+1",
            MainAction::ViewFavorites => "mod+2",
            MainAction::ViewRecent => "mod+3",
            MainAction::ViewInsights => "mod+4",
            MainAction::ViewClipboard => "mod+5",
            // `Keybind::new` splits alternatives on `,`, so literal comma must be `"mod+comma"`.
            MainAction::OpenSettings => "mod+comma",
            MainAction::OpenShortcuts => "mod+/, shift+/",
            MainAction::Back => "mod+[",
            MainAction::Forward => "mod+]",
            MainAction::ToggleSidebar => "mod+\\",
        }
    }

    /// Resolves a key event against all guards (`composing`, `modal_open`, `in_text_input`, and
    /// per-screen enablement).
    pub fn resolve_in_context(ev: &KeyEvent, ctx: ActionContext) -> Option<MainAction> {
        if ctx.composing || ctx.modal_open {
            return None;
        }
        if is_bare_shortcuts_question(ev) {
            if ctx.in_text_input {
                return None;
            }
            let action = MainAction::OpenShortcuts;
            return action.enabled_in(ctx).then_some(action);
        }
        for &action in Self::ALL {
            if !action.enabled_in(ctx) {
                continue;
            }
            if ctx.in_text_input && !action.allow_in_input() {
                continue;
            }
            if action.keys().matches(ev) {
                // `Keybind::new("mod+/, shift+/")` also matches `Shift+/` without `Cmd`; guard bare
                // `?` inside text inputs.
                if action == MainAction::OpenShortcuts
                    && ctx.in_text_input
                    && !ev.mods.cmd
                    && !ev.mods.ctrl
                {
                    continue;
                }
                return Some(action);
            }
        }
        None
    }
}

/// Matches a bare `?` keypress (arrives as `Key::Text("?")` with or without `Shift`, or
/// `Shift + '/'`, with no `Cmd` / `Ctrl` / `Alt`).
pub fn is_bare_shortcuts_question(ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
        return false;
    }
    match &ev.key {
        Key::Text(s) => s == "?" || (s == "/" && ev.mods.shift),
        _ => false,
    }
}

impl Action<MainState> for MainAction {
    const ALL: &'static [Self] = Self::ALL;

    fn label(self) -> &'static str {
        match self {
            MainAction::CommandMenu => "Search & commands",
            MainAction::NewSnippet => "New snippet",
            MainAction::NewGroup => "New group",
            MainAction::FocusFilter => "Filter snippets",
            MainAction::FocusList => "Focus the list",
            MainAction::EditContent => "Edit content",
            MainAction::DuplicateSnippet => "Duplicate snippet",
            MainAction::ToggleFavorite => "Toggle favorite",
            MainAction::ToggleEnabled => "Turn on / off",
            MainAction::DeleteSnippet => "Delete snippet",
            MainAction::TogglePause => "Pause / resume expanding",
            MainAction::ViewAll => "All snippets",
            MainAction::ViewFavorites => "Favorites",
            MainAction::ViewRecent => "Recently used",
            MainAction::ViewInsights => "Insights",
            MainAction::ViewClipboard => "Clipboard history",
            MainAction::OpenSettings => "Settings",
            MainAction::OpenShortcuts => "Keyboard shortcuts",
            MainAction::Back => "Back",
            MainAction::Forward => "Forward",
            MainAction::ToggleSidebar => "Toggle sidebar",
        }
    }

    fn keys(self) -> Keybind {
        Keybind::new(self.chord())
    }

    fn enabled(self, s: &MainState) -> bool {
        let ctx = ActionContext {
            area: s.frame.view.area(),
            has_selected_snippet: s.frame.selected_snippet.is_some(),
            can_back: s.frame.history.can_go_back(),
            can_forward: s.frame.history.can_go_forward(),
            in_text_input: s.is_editing_text(),
            modal_open: s.frame.any_modal_open(),
            composing: s.is_composing(),
        };
        !ctx.composing && !ctx.modal_open && self.enabled_in(ctx)
    }

    fn run(self, s: &mut MainState, tx: &mut Tx) {
        s.frame.clock_secs = tx.clock();
        s.dispatch_action(self);
        crate::main::keys::apply_focus_after_action(s, self, tx);
    }
}

#[cfg(test)]
mod tests {
    use rune::{Key, KeyEvent, Mods};

    use super::{ActionContext, MainAction, ScreenArea};
    use crate::actions::Action;
    use crate::shell::MainState;

    fn cmd(ch: char) -> KeyEvent {
        KeyEvent {
            key: Key::Command(ch),
            mods: Mods {
                cmd: true,
                ..Mods::default()
            },
        }
    }

    fn cmd_shift(ch: char) -> KeyEvent {
        KeyEvent {
            key: Key::Command(ch),
            mods: Mods {
                cmd: true,
                shift: true,
                ..Mods::default()
            },
        }
    }

    fn base_ctx() -> ActionContext {
        ActionContext {
            area: ScreenArea::Library,
            has_selected_snippet: true,
            can_back: true,
            can_forward: true,
            in_text_input: false,
            modal_open: false,
            composing: false,
        }
    }

    #[test]
    fn every_declared_shortcut_parses_in_rune_keybind() {
        for &action in <MainAction as Action<MainState>>::ALL {
            assert!(!action.label().is_empty());
            let _ = action.keys();
        }
    }

    #[test]
    fn resolves_all_lib_14_shortcuts() {
        let ctx = base_ctx();
        let cases: &[(KeyEvent, MainAction)] = &[
            (cmd('k'), MainAction::CommandMenu),
            (cmd('n'), MainAction::NewSnippet),
            (cmd_shift('n'), MainAction::NewGroup),
            (cmd('f'), MainAction::FocusFilter),
            (cmd('l'), MainAction::FocusList),
            (cmd('e'), MainAction::EditContent),
            (cmd('d'), MainAction::DuplicateSnippet),
            (cmd_shift('f'), MainAction::ToggleFavorite),
            (cmd_shift('e'), MainAction::ToggleEnabled),
            (
                KeyEvent {
                    key: Key::Backspace,
                    mods: Mods {
                        cmd: true,
                        ..Mods::default()
                    },
                },
                MainAction::DeleteSnippet,
            ),
            (cmd_shift('p'), MainAction::TogglePause),
            (cmd('1'), MainAction::ViewAll),
            (cmd('2'), MainAction::ViewFavorites),
            (cmd('3'), MainAction::ViewRecent),
            (cmd('4'), MainAction::ViewInsights),
            (cmd('5'), MainAction::ViewClipboard),
            (cmd(','), MainAction::OpenSettings),
            (cmd('/'), MainAction::OpenShortcuts),
            (
                KeyEvent {
                    key: Key::Text("?".into()),
                    mods: Mods {
                        shift: true,
                        ..Mods::default()
                    },
                },
                MainAction::OpenShortcuts,
            ),
            (cmd('['), MainAction::Back),
            (cmd(']'), MainAction::Forward),
            (cmd('\\'), MainAction::ToggleSidebar),
        ];

        for (ev, expected) in cases {
            assert_eq!(
                MainAction::resolve_in_context(ev, ctx),
                Some(*expected),
                "event {ev:?} should resolve to {expected:?}"
            );
        }
    }

    #[test]
    fn respects_text_input_modal_and_ime_guards() {
        let mut in_input = base_ctx();
        in_input.in_text_input = true;

        let question = KeyEvent {
            key: Key::Text("?".into()),
            mods: Mods {
                shift: true,
                ..Mods::default()
            },
        };
        let cmd_backspace = KeyEvent {
            key: Key::Backspace,
            mods: Mods {
                cmd: true,
                ..Mods::default()
            },
        };

        // Bare `?` and `⌘⌫` never fire inside editable text inputs.
        assert_eq!(MainAction::resolve_in_context(&question, in_input), None);
        assert_eq!(
            MainAction::resolve_in_context(&cmd_backspace, in_input),
            None
        );

        // Command chords (`⌘/`, `⌘L`, `⌘N`) still work inside text inputs.
        assert_eq!(
            MainAction::resolve_in_context(&cmd('/'), in_input),
            Some(MainAction::OpenShortcuts)
        );
        assert_eq!(
            MainAction::resolve_in_context(&cmd('l'), in_input),
            Some(MainAction::FocusList)
        );
        assert_eq!(
            MainAction::resolve_in_context(&cmd('n'), in_input),
            Some(MainAction::NewSnippet)
        );

        // Modal open and IME composition suppress all window shortcuts.
        let mut modal = base_ctx();
        modal.modal_open = true;
        assert_eq!(MainAction::resolve_in_context(&cmd('n'), modal), None);

        let mut composing = base_ctx();
        composing.composing = true;
        assert_eq!(MainAction::resolve_in_context(&cmd('n'), composing), None);
        assert_eq!(MainAction::resolve_in_context(&cmd('k'), composing), None);
    }

    #[test]
    fn library_and_list_actions_follow_active_area() {
        let mut settings = base_ctx();
        settings.area = ScreenArea::Settings;
        assert_eq!(MainAction::resolve_in_context(&cmd('d'), settings), None);
        assert_eq!(MainAction::resolve_in_context(&cmd('e'), settings), None);
        assert_eq!(MainAction::resolve_in_context(&cmd('l'), settings), None);
        assert_eq!(
            MainAction::resolve_in_context(&cmd('1'), settings),
            Some(MainAction::ViewAll)
        );

        let mut clipboard = base_ctx();
        clipboard.area = ScreenArea::Clipboard;
        assert_eq!(
            MainAction::resolve_in_context(&cmd('l'), clipboard),
            Some(MainAction::FocusList)
        );
        assert_eq!(MainAction::resolve_in_context(&cmd('e'), clipboard), None);
    }
}
