//! Vocabulary shared by the engine, the application layer and the UI: plain value types with
//! no behaviour, so a status or an app can cross a layer without a mapping.
//!
//! The engine defines the same shapes today; S34 switches it to these types and deletes its
//! copies. Nothing here is persisted, so nothing here derives serde.

/// A screen rectangle in global top-left-origin points (logical pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A running or installed application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub bundle_id: String,
    pub name: String,
    /// Process id when running.
    pub pid: Option<i32>,
    /// Bundle path on disk, when known.
    pub path: Option<String>,
}

/// Privacy permissions the engine needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PermissionStatus {
    /// Accessibility (post events, read caret position, active tap).
    pub accessibility: bool,
    /// Input Monitoring (listen-only tap).
    pub input_monitoring: bool,
}

/// How the keyboard tap is currently attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapMode {
    /// Not listening (missing permissions or stopped).
    Off,
    /// Can observe but not swallow keystrokes.
    ListenOnly,
    /// Can observe and swallow keystrokes (best reliability).
    Active,
}

/// Engine health for the tray and the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineStatus {
    /// Permission to insert synthetic keystrokes, independent of pause and Secure Input.
    pub can_post_events: bool,
    /// A live listener can insert, and neither pause nor Secure Input blocks it.
    pub can_expand: bool,
    pub paused: bool,
    pub tap: TapMode,
    pub permissions: PermissionStatus,
    pub secure_input: bool,
    /// App holding Secure Input, when known.
    pub secure_input_owner: Option<String>,
    /// One-line human summary ("Ready", "Paused", "Secure Input on — expansion blocked"…).
    pub summary: String,
}
