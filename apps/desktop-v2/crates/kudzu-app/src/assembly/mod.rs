//! Composition root. Builds every service, starts the engine, and shuts them down in order.
//! Hotkey registration stays on the main thread in the shell; this module does not touch it.
//!
//! `Task` workers are untracked. Shutdown stops every service that owns a thread, then drains
//! the writer. A `Task` still running after that (an update check, a selection capture) is not
//! joined: dropping it abandons delivery, and the worker finishes on its own. There is no
//! process-wide task list.

mod reactions;
mod router;
mod selection;

pub use reactions::LOGIN_ITEM_UNAVAILABLE;
pub use selection::SelectionCapture;

use reactions::Reactions;
use router::EngineRouter;

use crate::{
    activity::ActivityService,
    agents::{AgentConfig, AgentsService},
    apps::Apps,
    clipboard::{ClipboardPorts, ClipboardService},
    data::{DataConfig, DataService},
    error::{IoFailure, PortError},
    expansion::{ExpansionService, FeedState, HistoryReader, RenderEnvironment},
    forms::FormService,
    hotkeys::HotkeySlots,
    library::LibraryService,
    onboarding::Onboarding,
    paths::Paths,
    permissions::{Permissions, PermissionsConfig},
    persist::{LoadSource, PersistError, Writer},
    ports::{ExpansionEngine, KudzuWindows, Ports},
    schedule::{Scheduler, Timer},
    settings::{SettingsService, Variant},
    sync::{SyncConfig, SyncService},
    updates::{self, Updates},
    watch::{Publisher, Watch},
};
use chrono::{DateTime, FixedOffset};
use kudzu_core::interchange::backup::BackupTimeZone;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

#[cfg(test)]
mod tests;

/// Why composition could not finish starting.
#[derive(Debug, thiserror::Error)]
pub enum AssemblyError {
    #[error("The application folder could not be prepared.")]
    Prepare(#[from] IoFailure),
    #[error(transparent)]
    Persist(#[from] PersistError),
    #[error(transparent)]
    Settings(#[from] crate::settings::SettingsError),
    #[error(transparent)]
    Library(#[from] crate::library::LibraryError),
    #[error(transparent)]
    Clipboard(#[from] crate::clipboard::ClipboardError),
    #[error(transparent)]
    Expansion(#[from] crate::expansion::ExpansionError),
    #[error(transparent)]
    Forms(#[from] crate::forms::FormError),
    #[error(transparent)]
    Sync(#[from] crate::sync::SyncError),
    #[error(transparent)]
    Updates(#[from] crate::updates::UpdateError),
    #[error(transparent)]
    Schedule(#[from] crate::schedule::ScheduleError),
    #[error(transparent)]
    Data(#[from] crate::data::DataError),
    #[error(transparent)]
    Port(#[from] PortError),
}

impl From<std::io::Error> for AssemblyError {
    fn from(error: std::io::Error) -> Self {
        Self::Prepare(error.into())
    }
}

/// What the shell and the windows need in order to build the process.
pub struct Config {
    pub paths: Paths,
    pub ports: Ports,
    pub variant: Variant,
    pub version: String,
    pub app_name: String,
    pub bundle_id: String,
    pub os_major: u32,
    pub home: std::path::PathBuf,
    pub time_zone: BackupTimeZone,
    pub locale: String,
    /// Native uses `chrono::Local`. Headless passes a fixed offset so it ignores the host zone.
    pub local_time: fn(SystemTime) -> DateTime<FixedOffset>,
    pub mcp_name: String,
    pub executable: std::path::PathBuf,
    pub agent_args: Vec<String>,
    pub bundled_skill_dir: Option<std::path::PathBuf>,
    /// Sync may run only for the production bundle. Independent of the updates gate.
    pub production: bool,
    /// Passed to the updater identity gate. Dev and Kudzu Next stay closed.
    pub cloud_and_updates: bool,
    pub debounce: Duration,
    pub retry: Duration,
}

/// Result of [`Kudzu::shutdown`]. `ran` is false when shutdown already finished.
/// `Timeout` means the drain gave up; the quit was not durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownOutcome {
    pub ran: bool,
    pub durable: Result<(), PersistError>,
}

/// Narrow handle so the shell can shut down without holding every service.
pub trait Shutdown: Send + Sync {
    fn shutdown(&self) -> ShutdownOutcome;
}

/// The running application. Drop shuts it down.
pub struct Kudzu {
    pub settings: Arc<SettingsService>,
    pub library: Arc<LibraryService>,
    pub clipboard: Arc<ClipboardService>,
    pub expansion: Arc<ExpansionService>,
    pub forms: Arc<FormService>,
    pub activity: Arc<ActivityService>,
    pub updates: Updates,
    pub permissions: Arc<Permissions>,
    pub apps: Arc<Apps>,
    pub onboarding: Arc<Onboarding>,
    pub hotkeys: Arc<HotkeySlots>,
    pub data: DataService,
    pub sync: Arc<SyncService>,
    pub agents: Arc<AgentsService>,
    pub selection: Arc<SelectionCapture>,
    /// Which windows are Kudzu's. The shell pushes the list on every open, close or change.
    pub windows: Arc<dyn KudzuWindows>,
    /// `Some(LOGIN_ITEM_UNAVAILABLE)` after a failed enable or disable. Cleared when a later
    /// change of `launch_at_login` succeeds. A failed value is not tried again until it changes.
    pub login_notice: Watch<Option<String>>,
    first_run: bool,
    writer: Writer,
    scheduler: Arc<Scheduler>,
    engine: Arc<dyn ExpansionEngine>,
    reactions: Reactions,
    data_timer: Mutex<Option<Timer>>,
    outcome: Mutex<Option<Result<(), PersistError>>>,
}

impl Kudzu {
    /// Open stores, start producers, and block until the engine feed has caught the new engine.
    /// The block is intentional and happens before the UI loop.
    pub fn new(config: Config) -> Result<Arc<Self>, AssemblyError> {
        prepare_directory(&config.paths.data)?;
        let scheduler = Arc::new(Scheduler::new(config.ports.clock.clone())?);
        let writer = Writer::new(config.ports.clock.clone(), config.debounce, config.retry)?;
        let device_id = load_device_id(&config.paths.device_id())?;
        let settings = Arc::new(SettingsService::open(
            config.paths.settings(),
            config.variant,
            &writer,
        )?);
        let (library, source) = LibraryService::open(
            config.paths.library(),
            device_id.clone(),
            config.ports.clock.clone(),
            config.ports.ids.clone(),
            &writer,
            scheduler.clone(),
        )?;
        let library = Arc::new(library);
        let clipboard = Arc::new(ClipboardService::open(
            config.paths.clipboard(),
            settings.watch(),
            ClipboardPorts {
                board: config.ports.pasteboard.clone(),
                thumbnails: config.ports.thumbnails.clone(),
                target: config.ports.target_app.clone(),
                icons: config.ports.file_icons.clone(),
                clock: config.ports.clock.clone(),
            },
            &writer,
        )?);
        let data = DataService::new(
            DataConfig {
                backups: config.paths.backups(),
                home: config.home.clone(),
                time_zone: config.time_zone,
            },
            library.clone(),
            settings.clone(),
            config.ports.clock.clone(),
        );
        if let Err(error) = data.note_app_version(&config.version) {
            log::warn!("recording the app version failed; the next launch retries ({error})");
        }
        let data_timer = data.start(&scheduler)?;
        let history = history_reader(clipboard.clone());
        let expansion = Arc::new(ExpansionService::new(
            library.watch(),
            settings.clone(),
            config.ports.engine.clone(),
            config.ports.target_app.clone(),
            RenderEnvironment {
                clock: config.ports.clock.clone(),
                pasteboard: config.ports.pasteboard.clone(),
                locale: config.locale.clone(),
                local_time: config.local_time,
                history,
            },
        )?);
        let forms = Arc::new(FormService::open(
            config.paths.form_memory(),
            &writer,
            library.watch(),
            expansion.clone(),
            config.ports.forms.clone(),
            config.ports.target_app.clone(),
        )?);
        let activity = Arc::new(ActivityService::new(
            library.clone(),
            settings.clone(),
            expansion.status(),
            config.ports.clock.clone(),
        ));
        let router = EngineRouter::new(&expansion, &library, &forms, &activity, &clipboard);
        // Stop a previous adapter before this one starts. The adapter also stops its engine.
        config.ports.engine.stop();
        config.ports.engine.start(router)?;
        config.ports.windows.set_kudzu_windows(&[]);
        let generation = expansion.resync();
        wait_for_feed(&expansion.feed(), generation);
        clipboard.start(&scheduler)?;
        let sync = Arc::new(SyncService::open(
            SyncConfig {
                production: config.production,
                device_id,
                directory: config.paths.sync(),
            },
            library.clone(),
            settings.watch(),
            config.ports.cloud.clone(),
            config.ports.clock.clone(),
            scheduler.clone(),
        )?);
        let prefs = Arc::new(Publisher::new(prefs_of(&settings.watch().get())));
        let mut update_config = updates::Config::for_identity(
            config.bundle_id.clone(),
            config.cloud_and_updates,
            config.version.clone(),
        );
        update_config.stage_dir = config.paths.data.join("updates");
        let updates = Updates::new(
            update_config,
            prefs.watch(),
            updates::UpdatePorts {
                fetch: config.ports.fetch.clone(),
                installer: config.ports.installer.clone(),
                relauncher: config.ports.relauncher.clone(),
                clock: config.ports.clock.clone(),
                scheduler: scheduler.clone(),
            },
        )?;
        let agents = Arc::new(AgentsService::new(
            AgentConfig {
                socket: config.paths.agent_socket(),
                executable: config.executable,
                home: config.home,
                name: config.mcp_name,
                args: config.agent_args,
                version: config.version,
                locale: config.locale,
                bundled_skill_dir: config.bundled_skill_dir,
            },
            library.clone(),
            clipboard.clone(),
            settings.watch(),
            config.ports.clock.clone(),
            config.ports.ids.clone(),
        ));
        let apps = Arc::new(Apps::new(config.ports.apps.clone()));
        let selection = Arc::new(SelectionCapture::new(config.ports.target_app.clone()));
        let permissions = Arc::new(Permissions::new(
            config.ports.engine.clone(),
            config.ports.settings.clone(),
            config.ports.relauncher.clone(),
            scheduler.clone(),
            PermissionsConfig {
                app_name: config.app_name,
                os_major: config.os_major,
                marker: config.paths.permission_restart(),
            },
        ));
        let onboarding = Arc::new(Onboarding::new(
            settings.clone(),
            config.ports.engine.clone(),
        ));
        let hotkeys = Arc::new(HotkeySlots::new(settings.clone()));
        let login_notice: Arc<Publisher<Option<String>>> = Arc::new(Publisher::new(None));
        let reactions = Reactions::start(
            settings.clone(),
            agents.clone(),
            config.ports.login_item.clone(),
            updates.clone(),
            sync.status(),
            prefs,
            login_notice.clone(),
        );
        Ok(Arc::new(Self {
            settings,
            library,
            clipboard,
            expansion,
            forms,
            activity,
            updates,
            permissions,
            apps,
            onboarding,
            hotkeys,
            data,
            sync,
            agents,
            selection,
            windows: config.ports.windows.clone(),
            login_notice: login_notice.watch(),
            first_run: source == LoadSource::Missing,
            writer,
            scheduler,
            engine: config.ports.engine,
            reactions,
            data_timer: Mutex::new(Some(data_timer)),
            outcome: Mutex::new(None),
        }))
    }

    pub fn first_run(&self) -> bool {
        self.first_run
    }

    /// Idempotent. Stops producers, drains the writer, then asks the updater to install.
    /// A second call returns the same durability result and `ran: false`.
    pub fn shutdown(&self) -> ShutdownOutcome {
        let mut slot = self.outcome.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(durable) = *slot {
            return ShutdownOutcome {
                ran: false,
                durable,
            };
        }
        let durable = self.shutdown_once();
        *slot = Some(durable);
        ShutdownOutcome { ran: true, durable }
    }

    fn shutdown_once(&self) -> Result<(), PersistError> {
        // The reaction thread must not be waiting on the UI loop.
        self.reactions.stop();
        self.agents.shutdown();
        self.expansion.stop_feed();
        self.engine.stop();
        if let Err(error) = self.clipboard.shutdown() {
            log::warn!("clipboard capture did not stop cleanly: {error}");
        }
        self.forms.stop_pruning();
        drop(
            self.data_timer
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
        self.scheduler.shutdown();
        let mut durable = Ok(());
        self.sync.shutdown_with(|| {
            durable = self.writer.shutdown();
        });
        if let Err(error) = self.clipboard.finish_shutdown() {
            log::warn!("clipboard did not finish shutting down: {error}");
        }
        // The update service relaunches only from restart_to_update, and only after
        // Installed or a verified Skipped version newer than this process.
        self.updates.install_on_quit();
        durable
    }

    #[cfg(test)]
    pub(crate) fn writer(&self) -> &Writer {
        &self.writer
    }

    #[cfg(test)]
    pub(crate) fn update_checks(&self) -> u32 {
        self.reactions.update_checks()
    }
}

impl Shutdown for Kudzu {
    fn shutdown(&self) -> ShutdownOutcome {
        Kudzu::shutdown(self)
    }
}

impl Drop for Kudzu {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn prepare_directory(path: &Path) -> Result<(), AssemblyError> {
    std::fs::create_dir_all(path)?;
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

/// Read or create the stable device id. The id is never logged.
fn load_device_id(path: &Path) -> Result<String, AssemblyError> {
    if let Ok(text) = std::fs::read_to_string(path) {
        let text = text.trim();
        if uuid::Uuid::parse_str(text).is_ok() {
            return Ok(text.to_owned());
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(id.as_bytes())?;
    Ok(id)
}

fn history_reader(clipboard: Arc<ClipboardService>) -> Arc<HistoryReader> {
    Arc::new(move |index, current| clipboard.history_text(index, current))
}

fn prefs_of(settings: &crate::settings::Settings) -> updates::Prefs {
    let channel = match settings.update_channel {
        crate::settings::UpdateChannel::Stable => updates::Channel::Stable,
        crate::settings::UpdateChannel::Beta => updates::Channel::Beta,
    };
    updates::Prefs {
        auto_check: settings.auto_check_updates,
        channel,
    }
}

/// Bound the wait. A feed error or a timeout is logged and startup continues.
fn wait_for_feed(feed: &Watch<FeedState>, generation: u64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = feed.get();
        if state.generation >= generation {
            return;
        }
        if state.error.is_some() {
            log::warn!("the engine feed reported an error; opening anyway");
            return;
        }
        if Instant::now() >= deadline {
            log::warn!("the engine feed did not catch up; opening anyway");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Native clock conversion. Headless composition passes a fixed offset instead.
pub fn local_time(time: SystemTime) -> DateTime<FixedOffset> {
    let utc: DateTime<chrono::Utc> = time.into();
    utc.with_timezone(&chrono::Local).fixed_offset()
}

/// UTC offset for fixture composition. Headless runs must not read the host zone.
pub fn utc_offset() -> FixedOffset {
    FixedOffset::east_opt(0).expect("zero is a valid offset")
}

/// Fixture clock conversion. Ignores the host time zone.
pub fn utc_time(time: SystemTime) -> DateTime<FixedOffset> {
    let utc: DateTime<chrono::Utc> = time.into();
    utc.with_timezone(&utc_offset())
}
