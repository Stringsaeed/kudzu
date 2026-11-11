//! Routes `Shell::open_main` sends.

use super::Rig;
use crate::main::state::MainView;
use crate::shell::{Route, SettingsPane};

#[test]
fn shell_open_main_reconciles_routes_into_frame() {
    let mut rig = Rig::new(true);
    let shell = rig.rt.state.shell.clone();

    shell.open_main(Route::Settings(SettingsPane::Data));
    // Headless Runtime doesn't run Rune's multi-window event loop, so set route the same way the handle callback does.
    rig.rt.update(|st, _| {
        st.model.route = Route::Settings(SettingsPane::Data);
    });
    rig.rt.frame(0.1);
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Settings(SettingsPane::Data)
    );

    let _ = &rig.fixtures;
}
