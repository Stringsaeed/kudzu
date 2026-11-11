//! Key handling for the palette, matching the reference's `onKeyDown`.
//!
//! Control+N and Control+P move the selection even when Command is also held, and
//! they win over ⌘N. Escape clears a non-empty query (whitespace counts) before it
//! dismisses. ⌘1–9 insert that hit, never the create row.

use rune::{Key, KeyEvent, Modifier};
use uuid::Uuid;

use super::query::{create_seed, CreateSeed};
use super::rank::Scope;

/// What a key asks the shell to do. The shell owns the session, the hide, and the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// The query was cleared. The field has to drop its text too.
    QueryCleared,
    Insert(Uuid),
    Copy(Uuid),
    Edit(Uuid),
    Create(CreateSeed),
    /// Hide. `restore` returns focus to the captured app.
    Dismiss {
        restore: bool,
    },
}

/// Query, scope, selection, the number hints, and the "Copied" flash.
///
/// `query` is the field's text. It is not printed: `Debug` records its length.
pub struct PaletteKeys {
    pub query: String,
    pub scope: Scope,
    pub active: usize,
    pub show_numbers: bool,
    pub flash: Option<&'static str>,
}

impl Default for PaletteKeys {
    fn default() -> Self {
        Self {
            query: String::new(),
            scope: Scope::All,
            active: 0,
            show_numbers: false,
            flash: None,
        }
    }
}

impl std::fmt::Debug for PaletteKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaletteKeys")
            .field("query_len", &self.query.len())
            .field("scope", &self.scope)
            .field("active", &self.active)
            .field("show_numbers", &self.show_numbers)
            .field("flash", &self.flash)
            .finish()
    }
}

impl PaletteKeys {
    /// A fresh opening: query, scope, selection, flash and the number hints.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_query(&mut self, query: String) {
        if self.query == query {
            return;
        }
        self.query = query;
        self.active = 0;
    }

    pub fn set_scope(&mut self, scope: Scope) {
        if self.scope == scope {
            return;
        }
        self.scope = scope;
        self.active = 0;
    }

    /// Pointer hover. The index may be the create row.
    pub fn hover(&mut self, index: usize) {
        self.active = index;
    }

    /// Focus left the panel: the number hints go out with it.
    pub fn blur(&mut self) {
        self.show_numbers = false;
    }

    /// `rows` are the hit ids, not the create row. `create` is that extra row.
    /// `prefix` is the default trigger prefix. `main_focused` skips restoring the target.
    pub fn on_key(
        &mut self,
        key: &KeyEvent,
        rows: &[Uuid],
        create: bool,
        prefix: &str,
        main_focused: bool,
    ) -> Effect {
        if let Key::Modifier { modifier, pressed } = key.key {
            if !pressed && releases_numbers(modifier) {
                self.show_numbers = false;
            } else if pressed && (key.mods.cmd || key.mods.ctrl) {
                self.show_numbers = true;
            }
            return Effect::None;
        }
        if key.mods.cmd || key.mods.ctrl {
            self.show_numbers = true;
        }
        let total = rows.len() + usize::from(create);
        if ctrl_letter(key, 'n') {
            self.active = move_by(self.active, total, 1);
            return Effect::None;
        }
        if ctrl_letter(key, 'p') {
            self.active = move_by(self.active, total, -1);
            return Effect::None;
        }
        match &key.key {
            Key::Down => {
                self.active = move_by(self.active, total, 1);
                Effect::None
            }
            Key::Up => {
                self.active = move_by(self.active, total, -1);
                Effect::None
            }
            Key::PageDown => {
                self.active = page(self.active, total, 7);
                Effect::None
            }
            Key::PageUp => {
                self.active = page(self.active, total, -7);
                Effect::None
            }
            Key::Enter => self.activate(rows, create, prefix, key.mods.cmd),
            Key::Tab => {
                self.scope = self.scope.cycle(key.mods.shift);
                self.active = 0;
                Effect::None
            }
            Key::Escape => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.active = 0;
                    Effect::QueryCleared
                } else {
                    Effect::Dismiss {
                        restore: !main_focused,
                    }
                }
            }
            _ => self.command(key, rows, create, prefix),
        }
    }

    /// A click on row `index` (a hit, or the create row).
    pub fn pick(&mut self, index: usize, rows: &[Uuid], prefix: &str) -> Effect {
        self.active = index;
        self.activate(rows, index >= rows.len(), prefix, false)
    }

    fn activate(&mut self, rows: &[Uuid], create: bool, prefix: &str, copy: bool) -> Effect {
        let total = rows.len() + usize::from(create);
        if total == 0 {
            return Effect::None;
        }
        let index = self.active.min(total - 1);
        if index >= rows.len() {
            return Effect::Create(create_seed(&self.query, prefix));
        }
        let id = rows[index];
        if copy {
            self.flash = Some("Copied");
            Effect::Copy(id)
        } else {
            Effect::Insert(id)
        }
    }

    fn command(&mut self, key: &KeyEvent, rows: &[Uuid], create: bool, prefix: &str) -> Effect {
        let Some(c) = command_char(key) else {
            return Effect::None;
        };
        if c == 'e' {
            let total = rows.len() + usize::from(create);
            if rows.is_empty() || total == 0 {
                return Effect::None;
            }
            let index = self.active.min(total - 1);
            // Clamped onto the create row there is no snippet to edit.
            if index >= rows.len() {
                return Effect::None;
            }
            return Effect::Edit(rows[index]);
        }
        if c == 'n' {
            return Effect::Create(create_seed(&self.query, prefix));
        }
        if let Some(digit) = c.to_digit(10).filter(|d| (1..=9).contains(d)) {
            let index = (digit as usize) - 1;
            if index < rows.len() {
                return Effect::Insert(rows[index]);
            }
        }
        Effect::None
    }
}

fn releases_numbers(modifier: Modifier) -> bool {
    matches!(
        modifier,
        Modifier::ControlLeft | Modifier::ControlRight | Modifier::SuperLeft | Modifier::SuperRight
    )
}

/// Control+letter, whether it arrived as text (macOS Control bindings) or as a command.
fn ctrl_letter(key: &KeyEvent, letter: char) -> bool {
    key.mods.ctrl && typed(key) == Some(letter)
}

fn command_char(key: &KeyEvent) -> Option<char> {
    if !key.mods.cmd {
        return None;
    }
    typed(key)
}

fn typed(key: &KeyEvent) -> Option<char> {
    match &key.key {
        Key::Command(c) => Some(*c),
        Key::Text(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            chars.next().is_none().then_some(c.to_ascii_lowercase())
        }
        _ => None,
    }
}

fn move_by(active: usize, total: usize, delta: i64) -> usize {
    if total == 0 {
        return 0;
    }
    let current = active.min(total - 1) as i64;
    (current + delta).rem_euclid(total as i64) as usize
}

fn page(active: usize, total: usize, delta: i64) -> usize {
    if total == 0 {
        return 0;
    }
    let last = (total - 1) as i64;
    (active as i64 + delta).clamp(0, last) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    fn ids(n: usize) -> Vec<Uuid> {
        (1..=n).map(|i| Uuid::from_u128(i as u128)).collect()
    }

    fn key(k: Key) -> KeyEvent {
        KeyEvent {
            key: k,
            mods: Mods::default(),
        }
    }

    fn with(k: Key, mods: Mods) -> KeyEvent {
        KeyEvent { key: k, mods }
    }

    fn ctrl() -> Mods {
        Mods {
            ctrl: true,
            ..Mods::default()
        }
    }

    fn cmd() -> Mods {
        Mods {
            cmd: true,
            ..Mods::default()
        }
    }

    #[test]
    fn show_resets_query_scope_selection_flash_and_numbers() {
        let mut keys = PaletteKeys {
            query: "kept".into(),
            scope: Scope::Recent,
            active: 4,
            show_numbers: true,
            flash: Some("Copied"),
        };
        keys.reset();
        assert_eq!(format!("{keys:?}"), format!("{:?}", PaletteKeys::default()));
        assert!(!format!("{keys:?}").contains("kept"));
    }

    #[test]
    fn arrows_and_control_n_p_wrap_and_page_steps_seven() {
        let rows = ids(5);
        let mut keys = PaletteKeys::default();
        keys.on_key(&key(Key::Up), &rows, false, ";", false);
        assert_eq!(keys.active, 4);
        keys.on_key(&key(Key::Down), &rows, false, ";", false);
        assert_eq!(keys.active, 0);
        keys.on_key(
            &with(Key::Text("n".into()), ctrl()),
            &rows,
            false,
            ";",
            false,
        );
        assert_eq!(keys.active, 1);
        keys.on_key(
            &with(Key::Text("p".into()), ctrl()),
            &rows,
            false,
            ";",
            false,
        );
        assert_eq!(keys.active, 0);
        keys.on_key(&key(Key::PageDown), &rows, false, ";", false);
        assert_eq!(keys.active, 4);
        keys.on_key(&key(Key::PageDown), &rows, false, ";", false);
        assert_eq!(keys.active, 4);
        keys.on_key(&key(Key::PageUp), &rows, false, ";", false);
        assert_eq!(keys.active, 0);
    }

    #[test]
    fn control_n_moves_even_when_command_is_held() {
        let rows = ids(3);
        let mut keys = PaletteKeys::default();
        let both = Mods {
            ctrl: true,
            cmd: true,
            ..Mods::default()
        };
        let effect = keys.on_key(&with(Key::Command('n'), both), &rows, false, ";", false);
        assert_eq!(effect, Effect::None);
        assert_eq!(keys.active, 1);
    }

    #[test]
    fn tab_cycles_scope_and_returns_to_the_first_row() {
        let mut keys = PaletteKeys {
            active: 3,
            ..PaletteKeys::default()
        };
        keys.on_key(&key(Key::Tab), &ids(4), false, ";", false);
        assert_eq!(keys.scope, Scope::Favorites);
        assert_eq!(keys.active, 0);
        keys.on_key(
            &with(
                Key::Tab,
                Mods {
                    shift: true,
                    ..Mods::default()
                },
            ),
            &[],
            false,
            ";",
            false,
        );
        assert_eq!(keys.scope, Scope::All);
    }

    #[test]
    fn enter_inserts_command_enter_copies_and_the_create_row_creates() {
        let rows = ids(2);
        let mut keys = PaletteKeys {
            query: "brand new".into(),
            ..PaletteKeys::default()
        };
        assert_eq!(
            keys.on_key(&key(Key::Enter), &rows, true, ";", false),
            Effect::Insert(rows[0])
        );
        let copied = keys.on_key(&with(Key::Enter, cmd()), &rows, true, ";", false);
        assert_eq!(copied, Effect::Copy(rows[0]));
        assert_eq!(keys.flash, Some("Copied"));
        keys.active = 2;
        match keys.on_key(&key(Key::Enter), &rows, true, ";", false) {
            Effect::Create(seed) => assert_eq!(seed.value, "brand new"),
            other => panic!("create row, got {other:?}"),
        }
    }

    #[test]
    fn command_e_edits_command_n_creates_and_digits_insert_hits_only() {
        let rows = ids(3);
        let mut keys = PaletteKeys {
            query: ";zz".into(),
            active: 1,
            ..PaletteKeys::default()
        };
        assert_eq!(
            keys.on_key(&with(Key::Command('e'), cmd()), &rows, true, ";", false),
            Effect::Edit(rows[1])
        );
        keys.active = 3;
        assert_eq!(
            keys.on_key(&with(Key::Command('e'), cmd()), &rows, true, ";", false),
            Effect::None
        );
        match keys.on_key(&with(Key::Command('n'), cmd()), &rows, false, ";", false) {
            Effect::Create(seed) => {
                assert_eq!(seed.as_, super::super::query::CreateAs::Trigger);
                assert_eq!(seed.value, ";zz");
            }
            other => panic!("⌘N creates, got {other:?}"),
        }
        assert_eq!(
            keys.on_key(&with(Key::Command('2'), cmd()), &rows, true, ";", false),
            Effect::Insert(rows[1])
        );
        assert_eq!(
            keys.on_key(&with(Key::Command('9'), cmd()), &rows, true, ";", false),
            Effect::None
        );
    }

    #[test]
    fn escape_clears_a_query_then_dismisses_unless_main_is_focused() {
        let mut keys = PaletteKeys {
            query: "  ".into(),
            active: 2,
            ..PaletteKeys::default()
        };
        assert_eq!(
            keys.on_key(&key(Key::Escape), &[], false, ";", false),
            Effect::QueryCleared
        );
        assert!(keys.query.is_empty());
        assert_eq!(keys.active, 0);
        assert_eq!(
            keys.on_key(&key(Key::Escape), &[], false, ";", false),
            Effect::Dismiss { restore: true }
        );
        assert_eq!(
            keys.on_key(&key(Key::Escape), &[], false, ";", true),
            Effect::Dismiss { restore: false }
        );
    }

    #[test]
    fn modifiers_light_the_number_hints_and_releasing_either_clears_them() {
        let mut keys = PaletteKeys::default();
        let rows = ids(1);
        keys.on_key(&with(Key::Command('a'), cmd()), &rows, false, ";", false);
        assert!(keys.show_numbers);
        keys.on_key(
            &with(
                Key::Modifier {
                    modifier: Modifier::SuperLeft,
                    pressed: false,
                },
                Mods::default(),
            ),
            &rows,
            false,
            ";",
            false,
        );
        assert!(!keys.show_numbers);
        keys.on_key(
            &with(Key::Text("n".into()), ctrl()),
            &rows,
            false,
            ";",
            false,
        );
        assert!(keys.show_numbers);
        let still_cmd = Mods {
            cmd: true,
            ..Mods::default()
        };
        keys.on_key(
            &with(
                Key::Modifier {
                    modifier: Modifier::ControlLeft,
                    pressed: false,
                },
                still_cmd,
            ),
            &rows,
            false,
            ";",
            false,
        );
        assert!(
            !keys.show_numbers,
            "releasing Control clears the hints even if Command is held"
        );
        keys.show_numbers = true;
        keys.blur();
        assert!(!keys.show_numbers);
    }

    #[test]
    fn hover_selects_and_a_click_inserts() {
        let rows = ids(3);
        let mut keys = PaletteKeys::default();
        keys.hover(2);
        assert_eq!(keys.active, 2);
        assert_eq!(keys.pick(2, &rows, ";"), Effect::Insert(rows[2]));
    }
}
