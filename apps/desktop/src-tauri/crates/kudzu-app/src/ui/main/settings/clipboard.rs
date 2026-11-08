//! `ClipboardSettings` in settings.tsx: recording, the picker shortcut, retention (presets or a
//! custom number of days), plain pasting, secrets, ignored apps, and storage with Clear history.

use rune::{row, text, Cx, El, TextEdit};
use serde_json::json;

use kudzu_core::client::clipboard::{format_bytes, retention_label, RETENTION_OPTIONS};
use kudzu_core::client::js::group_thousands;

use super::expansion::{centered, centered_input};
use super::{desc, group, hotkey_recorder, page, row_el, sentence, Hotkey, Inline};
use crate::ui::kit::alert_dialog::AlertDialog;
use crate::ui::kit::app_picker::AppPicker;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::select::Select;
use crate::ui::kit::switch::Switch;
use crate::ui::kit::toast::Toast;
use crate::ui::main::insights::whole;
use crate::ui::main::MainUi;
use crate::ui::theme::text as ty;
use crate::ui::theme_for;

fn days_edit(s: &mut MainUi) -> &mut TextEdit {
    page(s).days.get_mut()
}

/// `commitCustom`: a whole number of days from 1 to 3650 saves; anything else puts the field back.
fn commit_custom(s: &mut MainUi) {
    let draft = s.settings_page.days.get_mut().text.trim().to_string();
    let days = s.settings.clipboard_retention_days;
    match draft.parse::<f64>().map(f64::round) {
        Ok(n) if (1.0..=3650.0).contains(&n) => s.update_settings(json!({ "clipboardRetentionDays": n as u32 })),
        _ => s.settings_page.days.get_mut().replace_text(if days == 0 { "7".to_string() } else { days.to_string() }),
    }
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings;
    let page_st = &s.settings_page;
    let days = st.clipboard_retention_days;
    let custom = page_st.custom.get();
    let entries = &s.clip.entries;
    let pinned = entries.iter().filter(|e| e.pinned).count();

    let edit = page_st.days.borrow().clone();
    // `onBlur={commitCustom}`: the field just lost focus.
    if page_st.days_focused.get() && !edit.focused {
        s.spawn(|| (), |s, _, _| commit_custom(s));
    }
    page_st.days_focused.set(edit.focused);

    let mut keep_controls = Vec::new();
    if custom {
        let tabular = ty::SM.features("'tnum' 1");
        let field = centered_input(&edit, "Days to keep history", tabular, days_edit, |_: &mut MainUi, _| {}).on_submit(|s: &mut MainUi, _| commit_custom(s));
        keep_controls.push(
            row()
                .center_y()
                .gap(6.0)
                .child(centered(th, field, edit.focused, 80.0))
                .child(text("days").font(ty::XS).color(t.muted_foreground)),
        );
    }
    let mut select = Select::new("keep-history", &page_st.keep, |s: &mut MainUi| &mut page(s).keep)
        .value(Some(&if custom { "custom".to_string() } else { days.to_string() }))
        .width(128.0)
        .label("Keep history for");
    for (d, label) in RETENTION_OPTIONS {
        select = select.option(d.to_string(), label);
    }
    select = select.option("custom", "Custom…").on_value_change(|s: &mut MainUi, v, _| {
        if v == "custom" {
            let days = s.settings.clipboard_retention_days;
            s.settings_page.custom.set(true);
            s.settings_page.days.get_mut().replace_text(if days == 0 { "7".to_string() } else { days.to_string() });
        } else if let Ok(d) = v.parse::<u32>() {
            s.settings_page.custom.set(false);
            s.update_settings(json!({ "clipboardRetentionDays": d }));
        }
    });
    keep_controls.push(select.build(cx, layer));
    let keep_desc = if days > 0 {
        format!("Unpinned items are deleted {}. Pinned items stay until you unpin them.", retention_label(days).replacen("for ", "after ", 1))
    } else {
        "Items stay until you delete them (up to 5,000). Pinned items always stay.".to_string()
    };

    let history = group(
        th,
        "Clipboard history",
        Some("Kudzu remembers what you copy, so you can find it and paste it again. History stays on this Mac. It is never synced or uploaded."),
        vec![
            row_el(
                th,
                "Record what I copy",
                Some(desc(th, "Text, links, images, files and colors, with the time each was copied.")),
                vec![Switch::new(st.clipboard_enabled)
                    .label("Record what I copy")
                    .id("clip-on")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.clipboard_enabled;
                        s.update_settings(json!({ "clipboardEnabled": on }))
                    })
                    .build(th)],
            ),
            row_el(
                th,
                "Shortcut",
                Some(sentence(th, vec![Inline::Text("Opens your history over any app. ".into()), Inline::Keys("enter"), Inline::Text(" pastes, ".into()), Inline::Keys("mod+enter"), Inline::Text(" only copies.".into())])),
                vec![hotkey_recorder(s, th, Hotkey::Clipboard, "Clipboard history shortcut")],
            ),
            row_el(th, "Keep history", Some(desc(th, keep_desc)), keep_controls),
            row_el(
                th,
                "Paste as plain text",
                Some(sentence(th, vec![Inline::Text("Drops fonts, colors and links from formatted text. Hold ".into()), Inline::Keys("shift"), Inline::Text(" to do the opposite.".into())])),
                vec![Switch::new(st.clipboard_paste_plain)
                    .label("Paste as plain text")
                    .id("clip-plain")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.clipboard_paste_plain;
                        s.update_settings(json!({ "clipboardPastePlain": on }))
                    })
                    .build(th)],
            ),
        ],
    );

    let ignored = AppPicker::new("clip-ignored", &st.clipboard_ignored_apps, &page_st.ignored, |s: &mut MainUi| &mut page(s).ignored)
        .add_label("Never record from an app")
        .on_change(|s: &mut MainUi, ids, _| s.update_settings(json!({ "clipboardIgnoredApps": ids })))
        .build(cx, layer);
    let privacy = group(
        th,
        "Privacy",
        Some("Password managers are excluded. Kudzu also skips anything an app marks as private or temporary, like passwords and one-time codes."),
        vec![
            row_el(
                th,
                "Show secrets",
                Some(desc(th, "Text that looks like a password, API key or token is hidden until you hover over it. Choose “Not a secret?” on an item to correct Kudzu.")),
                vec![Switch::new(st.clipboard_show_secrets)
                    .label("Show secrets")
                    .id("clip-secrets")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.clipboard_show_secrets;
                        s.update_settings(json!({ "clipboardShowSecrets": on }))
                    })
                    .build(th)],
            ),
            whole(rune::col()).py(12.0).child(ignored),
        ],
    );

    let n = entries.len();
    let size = if s.clip.disk_usage > 0 { format!(" · {}", format_bytes(s.clip.disk_usage)) } else { String::new() };
    let title = format!("{} item{}{size}", group_thousands(n as i64), if n == 1 { "" } else { "s" });
    let storage_desc = if pinned > 0 {
        format!("{pinned} pinned. Up to 5,000 unpinned items are kept; the oldest go first.")
    } else {
        "Up to 5,000 unpinned items are kept; the oldest go first.".to_string()
    };
    let storage = group(
        th,
        "Storage",
        None,
        vec![row_el(
            th,
            title,
            Some(desc(th, storage_desc)),
            vec![Button::new("Clear history…")
                .size(Size::Sm)
                .variant(Variant::Secondary)
                .disabled(n == 0)
                .on_press(|s: &mut MainUi| page(s).confirm_clear = true)
                .build(th)
                .key("clear-history")],
        )],
    );

    let what = if pinned > 0 {
        format!("Deletes everything except your {pinned} pinned item{}.", if pinned == 1 { "" } else { "s" })
    } else {
        "Deletes everything Kudzu remembered.".to_string()
    };
    AlertDialog::new("clear-clipboard")
        .open(page_st.confirm_clear)
        .on_open_change(|s: &mut MainUi, open, _| page(s).confirm_clear = open)
        .title("Clear clipboard history?")
        .description(format!("{what} This can't be undone."))
        .cancel("Cancel", |_: &mut MainUi, _| {})
        .action("Clear history", Variant::Destructive, |s: &mut MainUi, _| {
            let n = s.clip.clear(false);
            s.toasts.push(Toast::success(if n == 1 { "Deleted 1 item".to_string() } else { format!("Deleted {n} items") }));
        })
        .build(cx, layer);

    vec![history, privacy, storage]
}
