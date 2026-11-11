//! Update-ready toast helper stub. Owned and expanded by S63 (`crates/kudzu-ui/src/main/update_toast.rs`).

use kudzu_app::updates::UpdateState;

use crate::main::state::MainView;
use crate::main::toasts::{ToastAction, ToastSpec, UPDATE_TOAST_SECS};
use crate::shell::{MainState, Route, SettingsPane};

/// Announces a newly downloaded update once per version when not already viewing Settings (`SHL-10`).
pub fn sync_update_toast(s: &mut MainState, now_secs: f64) -> bool {
    let update = s.svc.updates.state().get();
    let version = match &*update {
        UpdateState::Ready { version, .. } => version.clone(),
        _ => return false,
    };
    if s.frame.announced_update.as_deref() == Some(version.as_str()) {
        return false;
    }
    s.frame.announced_update = Some(version.clone());
    if matches!(s.frame.view, MainView::Settings(_)) {
        return false;
    }
    let key = format!("update-ready:{version}");
    let spec = ToastSpec::new(format!("Kudzu {version} is ready"))
        .description("It installs when you quit, or restart now.")
        .duration_secs(UPDATE_TOAST_SECS)
        .action(ToastAction::new("Restart", true, |st: &mut MainState| {
            let _ = st.svc.updates.restart_to_update();
        }))
        .cancel(ToastAction::new(
            "What's new",
            false,
            |st: &mut MainState| {
                st.shell.open_main(Route::Settings(SettingsPane::About));
            },
        ));
    s.frame.toasts.show(key, spec, now_secs);
    true
}
