//! The menu-bar item's model: rows, labels, the status line, the glyph and the tooltip.
//! Pure, so the Tauri tray's order and labels are pinned by unit tests. `live` maps it onto
//! `kudzu_appkit::status_item` (RUNE-GAP(#141)).
mod live;

pub(crate) use live::Tray;

use kudzu_app::expansion::tray_summary;
use kudzu_app::updates::UpdateState;
use kudzu_appkit::status_item::{Glyph, GlyphKind, Shortcut};
use kudzu_core::system::{EngineStatus, TapMode};

/// Stable row ids; the live menu keeps one `NSMenuItem` per id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Row {
    Status,
    QuickSearch,
    Clipboard,
    FromSelection,
    Pause,
    Open,
    Settings,
    Updates,
    Quit,
}

impl Row {
    pub fn id(self) -> u32 {
        self as u32 + 1
    }

    pub fn from_id(id: u32) -> Option<Row> {
        ROWS.iter().copied().find(|row| row.id() == id)
    }
}

const ROWS: [Row; 9] = [
    Row::Status,
    Row::QuickSearch,
    Row::Clipboard,
    Row::FromSelection,
    Row::Pause,
    Row::Open,
    Row::Settings,
    Row::Updates,
    Row::Quit,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub row: Row,
    pub title: String,
    pub enabled: bool,
    pub shortcut: Shortcut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Item(Item),
    Separator,
}

/// Everything the status item shows, recomputed from snapshots on each change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// "Dev" beside the glyph for a Dev identity (Kudzu Next until cut-over).
    pub title: String,
    pub tooltip: String,
    pub glyph: Glyph,
    pub lines: Vec<Line>,
}

pub struct Input<'a> {
    pub app_name: &'a str,
    pub dev: bool,
    pub status: Option<&'a EngineStatus>,
    pub clipboard_enabled: bool,
    pub palette_hotkey: &'a str,
    pub clipboard_hotkey: &'a str,
    pub update: &'a UpdateState,
}

pub fn model(input: &Input<'_>) -> Model {
    let summary = input
        .status
        .map_or_else(|| "Starting…".to_owned(), tray_summary);
    let paused = input.status.is_some_and(|status| status.paused);
    let tooltip = match input.status {
        Some(_) => format!("{} — {summary}", input.app_name),
        None => input.app_name.to_owned(),
    };
    let none = Shortcut::default;
    Model {
        title: if input.dev {
            "Dev".into()
        } else {
            String::new()
        },
        tooltip,
        glyph: glyph(input.status, input.update),
        lines: vec![
            item(Row::Status, summary, false, none()),
            Line::Separator,
            item(
                Row::QuickSearch,
                format!("Quick Search      {}", hotkey_label(input.palette_hotkey)),
                true,
                none(),
            ),
            item(
                Row::Clipboard,
                format!(
                    "Clipboard History      {}",
                    hotkey_label(input.clipboard_hotkey)
                ),
                input.clipboard_enabled,
                none(),
            ),
            item(
                Row::FromSelection,
                "New Snippet from Selection".into(),
                true,
                none(),
            ),
            item(
                Row::Pause,
                if paused {
                    "Resume Expanding"
                } else {
                    "Pause Expanding"
                }
                .into(),
                true,
                Shortcut {
                    shift: true,
                    ..Shortcut::command("p")
                },
            ),
            Line::Separator,
            item(
                Row::Open,
                format!("Open {}…", input.app_name),
                true,
                Shortcut::command("o"),
            ),
            item(
                Row::Settings,
                "Settings…".into(),
                true,
                Shortcut::command(","),
            ),
            item(Row::Updates, update_label(input.update), true, none()),
            Line::Separator,
            item(Row::Quit, format!("Quit {}", input.app_name), true, none()),
        ],
    }
}

fn item(row: Row, title: String, enabled: bool, shortcut: Shortcut) -> Line {
    Line::Item(Item {
        row,
        title,
        enabled,
        shortcut,
    })
}

/// Pause, then Secure Input, then missing permission; the dot while an update waits.
pub fn glyph(status: Option<&EngineStatus>, update: &UpdateState) -> Glyph {
    let kind = match status {
        Some(status) if status.paused => GlyphKind::Paused,
        Some(status) if status.secure_input => GlyphKind::SecureInput,
        Some(status) if status.tap == TapMode::Off || !status.permissions.accessibility => {
            GlyphKind::Permissions
        }
        _ => GlyphKind::Normal,
    };
    Glyph {
        kind,
        update: matches!(update, UpdateState::Ready { .. }),
    }
}

pub fn update_label(state: &UpdateState) -> String {
    match state {
        UpdateState::Ready { version, .. } => format!("Restart to Update to {version}"),
        UpdateState::Downloading { version, .. } => format!("Downloading {version}…"),
        UpdateState::Restarting { .. } => "Restarting…".into(),
        UpdateState::Checking => "Checking for Updates…".into(),
        _ => "Check for Updates…".into(),
    }
}

/// `Control+Alt+Shift+Space` → `⌃⌥⇧SPACE`, as the Tauri tray wrote it.
pub fn hotkey_label(accelerator: &str) -> String {
    accelerator
        .split('+')
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => "⌃".to_owned(),
            "alt" | "option" => "⌥".to_owned(),
            "shift" => "⇧".to_owned(),
            "command" | "cmd" | "super" | "meta" | "cmdorctrl" | "commandorcontrol" => {
                "⌘".to_owned()
            }
            other => other.trim_start_matches("key").to_uppercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
