//! macOS adapters for kudzu-app's ports. `Ports::macos` (S40) builds them all, once per process.
//! The shell never calls CloudKit: the sync worker does. Headless runs use fixtures instead.
pub mod apps;
pub mod cloudkit;
pub mod engine;
pub mod files;
pub mod instance;
pub mod locale;
pub mod login_item;
pub mod pasteboard;
pub mod relaunch;
pub mod settings_panes;
pub mod thumbnails;
pub mod update_install;

mod code_identity;
mod unavailable;

use kudzu_app::assembly::Host;
use kudzu_app::ports::{
    clock::SystemClock, ids::RandomIds, AppCatalog, CloudStore, EngineInstances, ExpansionEngine,
    FileIcons, FormChannel, KudzuWindows, LoginItem, Ports, Relauncher, TargetApp, UpdateInstaller,
};
use kudzu_core::interchange::backup::BackupTimeZone;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What native composition knows before any adapter exists.
pub struct Native {
    pub bundle_id: String,
    pub version: String,
    /// The running executable, for the login item.
    pub executable: PathBuf,
    /// The running `.app`, or `None` for an unbundled `cargo run`.
    pub bundle: Option<PathBuf>,
    /// Runs the ordered shutdown and quits. The relaunch helper calls it from a worker.
    pub quit: Arc<dyn Fn() + Send + Sync>,
    /// `rune::is_headless()`. A headless run must use fixtures; this refuses it.
    pub headless: bool,
}

/// `Ports::macos` lives here because the adapters do; `kudzu-app` cannot name them.
pub trait MacosPorts: Sized {
    /// Panics in a headless run: no event tap, CloudKit, socket or login item there.
    fn macos(native: Native) -> Self;
}

impl MacosPorts for Ports {
    fn macos(native: Native) -> Self {
        assert!(
            !native.headless,
            "Ports::macos is not available in a headless run"
        );
        // One allocator and one adapter for the process. The same adapter serves the engine,
        // form and window ports; a later adapter would retire this one (S35).
        let engine = Arc::new(engine::EngineAdapter::new(Arc::new(
            EngineInstances::default(),
        )));
        let apps = Arc::new(apps::SystemApps);
        let (installer, login_item, relauncher) = installation(&native);
        Ports {
            engine: engine.clone() as Arc<dyn ExpansionEngine>,
            forms: engine.clone() as Arc<dyn FormChannel>,
            windows: engine as Arc<dyn KudzuWindows>,
            pasteboard: Arc::new(pasteboard::SystemPasteboard::general()),
            thumbnails: Arc::new(thumbnails::ImageThumbnails),
            apps: apps.clone() as Arc<dyn AppCatalog>,
            target_app: apps.clone() as Arc<dyn TargetApp>,
            file_icons: apps as Arc<dyn FileIcons>,
            settings: Arc::new(settings_panes::SettingsPanes),
            login_item,
            files: Arc::new(files::WorkspaceFiles),
            // One CloudKit for the process. Construction touches nothing; `start` checks the
            // entitlement, and the sync service never starts it for Kudzu Next or Dev.
            cloud: Arc::new(cloudkit::CloudKit::new()) as Arc<dyn CloudStore>,
            installer,
            relauncher,
            fetch: Arc::new(files::fetch::HttpFetch),
            clock: Arc::new(SystemClock::default()),
            // The shell is the registrar (it owns Rune's shortcuts); nothing reads this slot.
            hotkeys: Arc::new(unavailable::Hotkeys),
            ids: Arc::new(RandomIds),
        }
    }
}

/// The native host for `Kudzu::new`: `Ports::macos`, the system locale, HOME and local time.
pub fn host(native: Native) -> Host {
    Host {
        ports: Ports::macos(native),
        locale: locale::current(),
        home: std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default(),
        time_zone: BackupTimeZone::Local,
        local_time: kudzu_app::assembly::local_time,
    }
}

/// Take the single-instance lock before NSApplication starts. `None` means exit quietly:
/// another copy owns this identity (`Secondary`, which has asked it to reopen), or the lock's
/// owner could not be verified (`UnverifiedOwner`, logged), or the lock could not be taken.
pub fn claim_instance(
    data: &Path,
    bundle_id: &str,
    bundle: Option<&Path>,
) -> Option<instance::InstanceGuard> {
    match instance::Instance::acquire(data, bundle_id, bundle) {
        Ok(instance::Instance::Primary(guard)) => Some(guard),
        Ok(instance::Instance::Secondary) => None,
        Ok(instance::Instance::UnverifiedOwner) => {
            log::warn!("another process holds the instance lock and could not be verified");
            None
        }
        Err(error) => {
            log::error!("the instance lock could not be taken: {error}");
            None
        }
    }
}

/// One installer shared with the login item. An unbundled binary or a failed constructor
/// is "unavailable": never a startup failure and never a fake that pretends to succeed.
fn installation(
    native: &Native,
) -> (
    Arc<dyn UpdateInstaller>,
    Arc<dyn LoginItem>,
    Arc<dyn Relauncher>,
) {
    let installer: Arc<dyn UpdateInstaller> = match native.bundle.as_ref().map(|bundle| {
        update_install::BundleInstaller::new(bundle.clone(), &native.bundle_id, &native.version)
    }) {
        Some(Ok(installer)) => Arc::new(installer),
        _ => Arc::new(unavailable::Installer),
    };
    let login_item: Arc<dyn LoginItem> = match native.bundle {
        Some(_) => match login_item::LaunchAgent::new(
            &native.bundle_id,
            &native.executable,
            installer.clone(),
        ) {
            Ok(agent) => Arc::new(agent),
            Err(_) => Arc::new(unavailable::Login),
        },
        None => Arc::new(unavailable::Login),
    };
    let quit = native.quit.clone();
    let relauncher: Arc<dyn Relauncher> = match native.bundle.clone().map(|bundle| {
        relaunch::BundleRelauncher::new(bundle, native.bundle_id.clone(), move || quit())
    }) {
        Some(Ok(relauncher)) => Arc::new(relauncher),
        _ => Arc::new(unavailable::Relaunch),
    };
    (installer, login_item, relauncher)
}

/// The `.app` around the running executable (`X.app/Contents/MacOS/exe`), if any.
pub fn bundle_path() -> Option<PathBuf> {
    bundle_of(&std::env::current_exe().ok()?)
}

fn bundle_of(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let shaped = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension().is_some_and(|ext| ext == "app");
    shaped.then(|| bundle.to_path_buf())
}

/// The macOS major version, for the permission copy (System Settings names changed in 13).
pub fn os_major() -> u32 {
    let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
    u32::try_from(version.majorVersion).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{bundle_of, os_major};
    use std::path::{Path, PathBuf};

    #[test]
    fn only_the_bundle_layout_counts_as_a_bundle() {
        let inside = Path::new("/Applications/Kudzu Next.app/Contents/MacOS/Kudzu");
        assert_eq!(
            bundle_of(inside),
            Some(PathBuf::from("/Applications/Kudzu Next.app"))
        );
        assert_eq!(bundle_of(Path::new("/repo/target/debug/kudzu")), None);
        assert_eq!(
            bundle_of(Path::new("/x/Kudzu.app/Contents/Resources/k")),
            None
        );
        assert_eq!(bundle_of(Path::new("/x/Kudzu/Contents/MacOS/k")), None);
    }

    #[test]
    fn this_mac_reports_a_supported_major_version() {
        assert!(os_major() >= 14);
    }
}
