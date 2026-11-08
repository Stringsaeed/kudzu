//! `General` and `ICloudSync` in settings.tsx: permissions, the quick-search shortcut, iCloud
//! sync and the app's appearance, login and Dock options.

use rune::{col, row, text, Cx, El};
use serde_json::json;

use super::{desc, go, group, hotkey_recorder, row_el, sentence, spinner, Hotkey, Inline};
use crate::backend::cloud::SyncStatus;
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::segmented::Segmented;
use crate::ui::kit::switch::Switch;
use crate::ui::main::insights::still;
use crate::ui::main::{MainUi, SettingsSection};
use crate::ui::theme::{text as ty, weight, WeightOf};
use crate::ui::theme_for;

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, _layer: &mut Vec<El>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings;
    let mut out = Vec::new();
    // macOS: the permission card under its own heading.
    out.push(
        col()
            .label("Permissions")
            .gap(8.0)
            .child(text("Permissions").font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground).px(4.0))
            .child(crate::ui::permissions::setup_card::<MainUi>(th, &s.status)),
    );
    out.push(group(
        th,
        "Quick search",
        None,
        vec![row_el(
            th,
            "Shortcut",
            Some(sentence(
                th,
                vec![Inline::Text("Opens the search tray over any app. ".into()), Inline::Keys("enter"), Inline::Text(" inserts, ".into()), Inline::Keys("mod+enter"), Inline::Text(" copies.".into())],
            )),
            vec![hotkey_recorder(s, th, Hotkey::Palette, "Quick search shortcut")],
        )],
    ));
    out.push(icloud(s, cx));
    let theme = match st.theme {
        crate::settings::Theme::Dark => "dark",
        crate::settings::Theme::Light => "light",
        crate::settings::Theme::System => "system",
    };
    out.push(group(
        th,
        "App",
        None,
        vec![
            row_el(
                th,
                "Appearance",
                None,
                vec![Segmented::new("settings-theme", "Appearance")
                    .option("dark", "Dark")
                    .option("light", "Light")
                    .option("system", "System")
                    .selected(theme)
                    .on_change(|s: &mut MainUi, v: &str| s.update_settings(json!({ "theme": v })))
                    .build(th, cx)],
            ),
            row_el(
                th,
                "Open at login",
                Some(desc(th, "Kudzu lives in the menu bar and starts with your Mac.")),
                vec![Switch::new(st.launch_at_login)
                    .label("Open at login")
                    .id("login")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.launch_at_login;
                        s.update_settings(json!({ "launchAtLogin": on }))
                    })
                    .build(th)],
            ),
            row_el(
                th,
                "Show in Dock while open",
                Some(desc(th, "Otherwise Kudzu only appears in the menu bar.")),
                vec![Switch::new(st.show_in_dock)
                    .label("Show in Dock while open")
                    .id("dock")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.show_in_dock;
                        s.update_settings(json!({ "showInDock": on }))
                    })
                    .build(th)],
            ),
        ],
    ));
    out
}

/// `<ICloudSync>`: the switch, a live status line and Sync now / Update.
fn icloud(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let status = &s.sync;
    let busy = matches!(status, SyncStatus::Starting | SyncStatus::Syncing);
    let (line, tone) = match status {
        SyncStatus::Off => ("Off. Your snippets stay on this Mac.".to_string(), t.muted_foreground),
        SyncStatus::Unavailable { reason } => (reason.clone(), t.muted_foreground),
        SyncStatus::NoAccount { reason } => (reason.clone(), t.warning),
        SyncStatus::Starting | SyncStatus::Syncing => ("Syncing…".to_string(), t.muted_foreground),
        SyncStatus::UpToDate { last_sync } => {
            let ms = chrono::DateTime::parse_from_rfc3339(last_sync).map_or(0, |d| d.timestamp_millis());
            let ago = kudzu_core::client::stats::relative_time(ms, chrono::Utc::now().timestamp_millis(), &chrono::Local).to_lowercase();
            (format!("Up to date · synced {ago}"), t.muted_foreground)
        }
        SyncStatus::NeedsUpdate => ("Another Mac uses a newer Kudzu. Update this one to keep syncing; nothing here is lost meanwhile.".to_string(), t.warning),
        SyncStatus::Error { message } => (message.clone(), t.destructive),
    };
    let mut d = row().center_y().gap(6.0).min_w(0.0);
    if busy {
        d = d.child(still(spinner(12.0, tone).key("icloud-spin")));
    }
    if matches!(status, SyncStatus::UpToDate { .. }) {
        d = d.child(rune::icon(&icons::CHECK, 12.0).color(t.success).key("icloud-check").transition(super::check_in()).a11y_hidden());
    }
    d = d.child(text(line).key("icloud-status-label").font(ty::XS).color(tone).wrap().shrink());
    let mut controls = Vec::new();
    match status {
        SyncStatus::NeedsUpdate => controls.push(
            Button::new("Update").size(Size::Sm).variant(Variant::Secondary).on_press(|s: &mut MainUi| go(s, SettingsSection::About)).build(th),
        ),
        SyncStatus::UpToDate { .. } | SyncStatus::Error { .. } => controls.push(
            Button::new("Sync now")
                .size(Size::Sm)
                .variant(Variant::Ghost)
                .icon(&icons::REFRESH_CW)
                .on_press(|_: &mut MainUi| crate::backend::commands::sync_now())
                .build(th),
        ),
        _ => {}
    }
    controls.push(
        Switch::new(s.settings.icloud_sync)
            .label("Sync with iCloud")
            .id("icloud")
            .on_toggle(|s: &mut MainUi| {
                let on = !s.settings.icloud_sync;
                s.update_settings(json!({ "icloudSync": on }))
            })
            .build(th),
    );
    group(
        th,
        "iCloud",
        Some("The same snippets on every Mac signed in to your iCloud. Stored in your private iCloud database; Kudzu has no servers of its own."),
        vec![row_el(th, "Sync with iCloud", Some(d.label("Sync status")), controls)],
    )
}
