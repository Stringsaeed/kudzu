//! Consumer-specific boundaries implemented by system adapters and deterministic fakes.
//! S40: assembly owns an EngineRouter implementing EngineEvents. It forwards to expansion,
//! library, forms, activity and clipboard, so expansion never depends on those four services.
pub mod apps;
pub mod clock;
pub mod cloudkit;
pub mod engine;
pub mod fetch;
pub mod files;
pub mod hotkey_registrar;
pub mod login_item;
pub mod pasteboard;
pub mod relaunch;
pub mod settings_panes;
pub mod thumbnails;
pub mod update_install;

pub mod ids;
pub use apps::{AppCatalog, FileIcons, TargetApp};
pub use clock::Clock;
pub use cloudkit::{CloudAvailability, CloudBatch, CloudEvents, CloudStore};
pub use engine::{
    EngineEvents, EngineLibrary, Expansion, ExpansionEngine, FormChannel, FormRequest, TriggerLost,
};
pub use fetch::{Cancel, Fetch, FetchRequest};
pub use files::FileOpener;
pub use hotkey_registrar::{HotkeyRegistrar, ShortcutSlot};
pub use ids::IdSource;
pub use login_item::LoginItem;
pub use pasteboard::{PasteSnapshot, Pasteboard};
pub use relaunch::Relauncher;
pub use settings_panes::{PermissionKind, SettingsPane, SystemSettings};
pub use thumbnails::{Thumbnail, Thumbnailer};
pub use update_install::{InstallProbe, UpdateInstaller, UpdatePackage};

/// Composition-only bundle. Services take their individual ports; windows take services.
/// Inbound EngineEvents/CloudEvents are supplied by services when starting the adapter.
#[derive(Clone)]
pub struct Ports {
    pub engine: std::sync::Arc<dyn ExpansionEngine>,
    pub forms: std::sync::Arc<dyn FormChannel>,
    pub pasteboard: std::sync::Arc<dyn Pasteboard>,
    pub thumbnails: std::sync::Arc<dyn Thumbnailer>,
    pub apps: std::sync::Arc<dyn AppCatalog>,
    pub target_app: std::sync::Arc<dyn TargetApp>,
    pub file_icons: std::sync::Arc<dyn FileIcons>,
    pub settings: std::sync::Arc<dyn SystemSettings>,
    pub login_item: std::sync::Arc<dyn LoginItem>,
    pub files: std::sync::Arc<dyn FileOpener>,
    pub cloud: std::sync::Arc<dyn CloudStore>,
    pub installer: std::sync::Arc<dyn UpdateInstaller>,
    pub relauncher: std::sync::Arc<dyn Relauncher>,
    pub fetch: std::sync::Arc<dyn Fetch>,
    pub clock: std::sync::Arc<dyn Clock>,
    pub hotkeys: std::sync::Arc<dyn HotkeyRegistrar>,
    pub ids: std::sync::Arc<dyn IdSource>,
}
