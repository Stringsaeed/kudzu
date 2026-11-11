//! Composition: launch mode → identity → paths → ports (native, or fixtures in a temporary
//! folder when headless: never `Ports::macos`) → `Kudzu::new` → windows → wakers.
use crate::{cli::LaunchMode, identity::Identity};
use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::{fixtures::Fixtures, paths::Paths, settings::Variant};
use kudzu_system::launch::{claim_instance, QuitHook, Refusal};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Lives as long as the process: the services, the instance lock (native only), the relaunch
/// helper's quit hook (bound once windows exist) and a fixture run's temporary data folder.
pub struct Composed {
    pub kudzu: Arc<Kudzu>,
    _instance: Option<kudzu_system::instance::InstanceGuard>,
    quit: Arc<QuitHook>,
    pub scratch: Option<PathBuf>,
}

/// A `Refusal` exits before NSApplication starts: another copy, an unverified lock owner,
/// a lock error, or a failed `Kudzu::new`. `main` logs it and exits with its status.
pub fn start(
    mode: LaunchMode,
    dev: bool,
    id: Identity,
    dirs: &rune::dirs::AppDirs,
) -> Result<Composed, Refusal> {
    let native = mode == LaunchMode::Native;
    let scratch = (!native)
        .then(|| std::env::temp_dir().join(format!("kudzu-fixtures-{}", std::process::id())));
    let data = scratch.clone().unwrap_or_else(|| dirs.data().to_path_buf());
    let bundle = kudzu_system::bundle_path().filter(|_| native);
    let claim = || claim_instance(&data, id.bundle_id, bundle.as_deref());
    let instance = if native { Some(claim()?) } else { None };
    let version = include_str!("../version.txt").trim().to_owned();
    let executable = std::env::current_exe().unwrap_or_default();
    let quit = Arc::new(QuitHook::default());
    let host = if native {
        kudzu_system::host(kudzu_system::Native {
            bundle_id: id.bundle_id.into(),
            version: version.clone(),
            executable: executable.clone(),
            bundle: bundle.clone(),
            quit: quit.clone(),
            headless: rune::is_headless(),
        })
    } else {
        let fixtures = Fixtures::new().expect("the fixture sample is valid");
        Host::fixtures(&fixtures, data.clone())
    };
    let mut agent_args = vec!["--mcp".to_owned()];
    agent_args.extend(dev.then(|| "--dev".to_owned()));
    let config = Config {
        paths: Paths {
            data,
            logs: dirs.logs().to_path_buf(),
        },
        host,
        variant: [Variant::Dev, Variant::Production][usize::from(id.cloud_and_updates)],
        version,
        app_name: id.name.into(),
        bundle_id: id.bundle_id.into(),
        os_major: kudzu_system::os_major(),
        mcp_name: id.mcp_name.into(),
        executable,
        agent_args,
        bundled_skill_dir: bundle
            .map(|b| b.join("Contents/Resources/skills/kudzu"))
            .filter(|p| p.is_dir()),
        production: id.cloud_and_updates,
        cloud_and_updates: id.cloud_and_updates,
        debounce: Duration::from_millis(250),
        retry: Duration::from_secs(5),
    };
    let kudzu = Kudzu::new(config).map_err(Refusal::Startup)?;
    Ok(Composed {
        kudzu,
        _instance: instance,
        quit,
        scratch,
    })
}

/// The five windows (plus the gallery headless or with `--gallery`), then their wakers.
pub fn windows(
    app: &Composed,
    mode: LaunchMode,
    hidden: bool,
    id: Identity,
) -> Vec<rune::AnyWindowSpec> {
    let (app_name, dev, native) = (id.name, !id.cloud_and_updates, mode == LaunchMode::Native);
    let identity = kudzu_ui::shell::Identity { app_name, dev };
    let launch = kudzu_ui::shell::Launch {
        identity,
        native,
        hidden,
    };
    let built = kudzu_ui::shell::build(&app.kudzu, launch);
    built.wake_on_changes(&app.kudzu);
    app.quit.bind(built.quitter()); // Before any window exists, so before any relaunch.
    let mut windows = built.specs;
    let asked = cfg!(debug_assertions) && rune::extra_args().iter().any(|a| a == "--gallery");
    if mode == LaunchMode::Fixtures || asked {
        windows.push(kudzu_ui::gallery::window());
    }
    windows
}
