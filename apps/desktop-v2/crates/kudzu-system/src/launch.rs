//! What happens before the app starts: the single-instance claim and the reasons a launch
//! refuses to start, and the relaunch helper's quit hook, bound once windows exist.
use crate::instance::{Instance, InstanceGuard};
use kudzu_app::{assembly::AssemblyError, error::PortError};
use std::path::Path;
use std::sync::OnceLock;

/// Why this launch exits before the UI starts. Only `AlreadyRunning` is quiet.
#[derive(Debug)]
pub enum Refusal {
    /// Another copy owns this identity and was asked to reopen.
    AlreadyRunning,
    /// An active lock whose owner could not be authenticated. Nothing is opened or unlinked.
    UnverifiedOwner,
    /// The lock could not be taken at all.
    Lock(PortError),
    /// `Kudzu::new` failed.
    Startup(AssemblyError),
}

impl Refusal {
    pub fn level(&self) -> log::Level {
        match self {
            Refusal::AlreadyRunning => log::Level::Info,
            Refusal::UnverifiedOwner => log::Level::Warn,
            Refusal::Lock(_) | Refusal::Startup(_) => log::Level::Error,
        }
    }

    pub fn message(&self, app_name: &str) -> String {
        match self {
            Refusal::AlreadyRunning => format!("{app_name} is already running"),
            Refusal::UnverifiedOwner => {
                "another process holds the Kudzu lock but couldn't be verified".into()
            }
            Refusal::Lock(error) => format!("the instance lock could not be taken: {error}"),
            Refusal::Startup(error) => format!("{app_name} could not start: {error}"),
        }
    }

    /// The process exit status.
    pub fn status(&self) -> i32 {
        match self {
            Refusal::AlreadyRunning => 0,
            _ => 1,
        }
    }

    /// Log the reason at its level and return the exit status. A fixture run installs no
    /// logger, so there the reason goes to stderr.
    pub fn report(&self, app_name: &str) -> i32 {
        let message = self.message(app_name);
        if log::log_enabled!(self.level()) {
            log::log!(self.level(), "{message}");
        } else {
            eprintln!("{message}");
        }
        self.status()
    }
}

/// Take the single-instance lock before NSApplication starts, and hold the guard until exit.
pub fn claim_instance(
    data: &Path,
    bundle_id: &str,
    bundle: Option<&Path>,
) -> Result<InstanceGuard, Refusal> {
    refuse(Instance::acquire(data, bundle_id, bundle))
}

fn refuse(acquired: Result<Instance, PortError>) -> Result<InstanceGuard, Refusal> {
    match acquired {
        Ok(Instance::Primary(guard)) => Ok(guard),
        Ok(Instance::Secondary) => Err(Refusal::AlreadyRunning),
        Ok(Instance::UnverifiedOwner) => Err(Refusal::UnverifiedOwner),
        Err(error) => Err(Refusal::Lock(error)),
    }
}

/// The relaunch helper's quit, which runs the ordered shutdown through the shell.
#[derive(Default)]
pub struct QuitHook(OnceLock<Box<dyn Fn() + Send + Sync>>);

impl QuitHook {
    /// Bound once, by composition, when the windows are built. A second bind is ignored.
    pub fn bind(&self, quit: impl Fn() + Send + Sync + 'static) {
        let _ = self.0.set(Box::new(quit));
    }

    /// Only `restart_to_update` and `request_restart` relaunch, and both are reached from a
    /// window, which exists only after `bind`. An unbound fire is a composition bug.
    pub fn fire(&self) {
        let quit = self
            .0
            .get()
            .expect("relaunch before the windows exist: only window actions may relaunch");
        quit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn refused(claim: Result<InstanceGuard, Refusal>) -> Refusal {
        match claim {
            Ok(_) => panic!("the claim succeeded"),
            Err(refusal) => refusal,
        }
    }

    #[test]
    fn only_a_secondary_exits_quietly_as_already_running() {
        let quiet = refused(refuse(Ok(Instance::Secondary)));
        assert!(matches!(quiet, Refusal::AlreadyRunning));
        assert_eq!(quiet.status(), 0);
        assert_eq!(quiet.level(), log::Level::Info);
        assert_eq!(quiet.message("Kudzu Next"), "Kudzu Next is already running");

        let unverified = refused(refuse(Ok(Instance::UnverifiedOwner)));
        assert!(matches!(unverified, Refusal::UnverifiedOwner));
        assert_eq!(unverified.status(), 1);
        assert_eq!(unverified.level(), log::Level::Warn);
        assert_eq!(
            unverified.message("Kudzu Next"),
            "another process holds the Kudzu lock but couldn't be verified"
        );

        let failed = refused(refuse(Err(PortError::PermissionDenied)));
        assert!(matches!(failed, Refusal::Lock(PortError::PermissionDenied)));
        assert_eq!(failed.status(), 1);
        assert_eq!(failed.level(), log::Level::Error);
    }

    #[test]
    fn a_startup_failure_is_an_error_with_status_one() {
        let failed = Refusal::Startup(AssemblyError::FeedTimeout);
        assert_eq!(failed.status(), 1);
        assert_eq!(failed.level(), log::Level::Error);
        assert!(failed
            .message("Kudzu Next")
            .starts_with("Kudzu Next could not start"));
    }

    #[test]
    fn a_refused_claim_leaves_the_lock_file_in_place() {
        let dir =
            std::path::PathBuf::from("/tmp").join(format!("kudzu-claim-{}", uuid::Uuid::new_v4()));
        let held = claim_instance(&dir, "sh.saeed.kudzu.next", None)
            .ok()
            .unwrap();
        let second = refused(claim_instance(&dir, "sh.saeed.kudzu.next", None));
        assert!(matches!(second, Refusal::UnverifiedOwner), "{second:?}");
        assert!(dir.join("sh.saeed.kudzu.next.lock").exists());
        drop(held);
        assert!(dir.join("sh.saeed.kudzu.next.lock").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_quit_hook_runs_what_composition_bound() {
        let hook = Arc::new(QuitHook::default());
        let calls = Arc::new(AtomicU32::new(0));
        let counted = calls.clone();
        hook.bind(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        hook.bind(|| panic!("a second bind is ignored"));
        hook.fire();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[should_panic(expected = "relaunch before the windows exist")]
    fn an_unbound_quit_hook_is_a_composition_bug() {
        QuitHook::default().fire();
    }
}
