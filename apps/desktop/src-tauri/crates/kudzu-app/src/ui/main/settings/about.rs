//! `About` and `UpdateRow` in settings.tsx: the app's icon, wordmark and version, software
//! updates (check, download progress, Restart to Update with release notes, a misplaced app),
//! the update options and the privacy note.

use rune::{col, icon, rich, row, text, Cx, El, ScrollSpec, Shadow, Span};
use serde_json::json;

use kudzu_core::client::updates::{parse_notes, RELEASES_URL};

use super::{desc, group, row_el, spinner};
use crate::backend::updater::UpdateState;
use crate::settings::UpdateChannel;
use crate::ui::icons;
use crate::ui::kit::button::{hover_fast, Button, Size, Variant};
use crate::ui::kit::switch::Switch;
use crate::ui::kit::toast::Toast;
use crate::ui::kit::wordmark::{wordmark, WordmarkVariant};
use crate::ui::main::insights::{still, whole};
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

const BRAND_TAGLINE: &str = "For things worth repeating";
const BRAND_DESCRIPTION: &str = "Clipboard history and text shortcuts for your Mac.";

/// `check`: shows Checking… and asks the feed on a worker.
fn check(s: &mut MainUi) {
    s.update = UpdateState::Checking;
    s.spawn(crate::backend::commands::check_for_updates, |s, r, _| {
        s.update = r.unwrap_or_else(|e| UpdateState::Error { message: e, needs_move: false });
    });
}

fn restart(s: &mut MainUi) {
    if let Err(e) = crate::backend::commands::restart_to_update() {
        s.toasts.push(Toast::error("Couldn't install the update").description(e));
    }
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings;
    let version = s.settings_page.version.as_deref().unwrap_or("…");

    // `drop-shadow-[0_12px_24px_oklch(0.488_0.243_264.376/0.3)]` follows the icon's squircle,
    // which fills the middle 80 % of its canvas.
    let glow = Shadow { y: 12.0, blur: 24.0, color: rune::oklch(0.488, 0.243, 264.376).alpha(0.3), ..Shadow::default() };
    let art = rune::el()
        .size(88.0, 88.0)
        .my(-8.0)
        .a11y_hidden()
        .child(rune::el().pin(Some(8.8), Some(8.8), Some(8.8), Some(8.8)).radius(16.0).box_shadow(glow))
        .child(rune::el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).pixels(crate::ui::permissions::app_icon()));
    let relaxed = ty::SM.lh(14.0 * 1.625);
    let blurb = format!("{BRAND_TAGLINE}\n{BRAND_DESCRIPTION}");
    let hero = col()
        .items_center()
        .gap(16.0)
        .pt(24.0)
        .child(art)
        .child(
            col()
                .items_center()
                // `<h2><Wordmark className="mb-2 w-36" /></h2>`: the inline mark sits on the
                // heading's baseline, so its line box runs about 5 pt below it.
                .child(wordmark(WordmarkVariant::Small, 144.0, t.foreground).mb(13.0).role("heading").label("Kudzu"))
                .child(text(format!("Version {version} · MIT licensed")).font(ty::XS).color(t.muted_foreground)),
        )
        .child(
            rich(blurb, vec![Span::new(0, BRAND_TAGLINE.len()).weight(weight::MEDIUM).color(t.foreground)])
                .font(relaxed)
                .color(t.muted_foreground)
                .align(rune::Align::Center)
                .wrap()
                .max_w(384.0),
        );

    let updates = group(
        th,
        "Updates",
        None,
        vec![
            update_row(s, cx),
            row_el(
                th,
                "Check for updates automatically",
                Some(desc(th, "Looks a few times a day and downloads quietly. Kudzu never restarts without you.")),
                vec![Switch::new(st.auto_check_updates)
                    .label("Check for updates automatically")
                    .id("auto-update")
                    .on_toggle(|s: &mut MainUi| {
                        let on = !s.settings.auto_check_updates;
                        s.update_settings(json!({ "autoCheckUpdates": on }))
                    })
                    .build(th)],
            ),
            row_el(
                th,
                "Get beta updates",
                Some(desc(th, "Try new versions early. You'll also get every stable release.")),
                vec![Switch::new(st.update_channel == UpdateChannel::Beta)
                    .label("Get beta updates")
                    .id("beta")
                    .on_toggle(|s: &mut MainUi| {
                        let beta = s.settings.update_channel != UpdateChannel::Beta;
                        s.update_settings(json!({ "updateChannel": if beta { "beta" } else { "stable" } }))
                    })
                    .build(th)],
            ),
        ],
    );

    let privacy = row()
        .items_start()
        .gap(8.0)
        .radius(radius::XL)
        .fill(t.surface)
        .px(16.0)
        .py(12.0)
        .elevate(&t.elev_card)
        .child(icon(&icons::SHIELD_CHECK, 16.0).color(t.success).mt(1.0).a11y_hidden())
        .child(
            text("No accounts, no analytics, no Kudzu servers. Your snippets live on this Mac and, if you turn on iCloud sync, in your private iCloud database, which Kudzu's developer can't see. Apart from that, Kudzu only goes online to check for updates, and that sends nothing about you.")
                .font(ty::XS.lh(12.0 * 1.625))
                .color(t.muted_foreground)
                .wrap()
                .grow(1.0)
                .min_w(0.0),
        );

    // About renders one wrapper (`flex flex-col gap-8`), so the page's stagger moves it as one.
    vec![whole(col()).gap(32.0).child(hero).child(updates).child(privacy)]
}

/// `<UpdateRow>`.
fn update_row(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let title_font = ty::BASE.weight_of(weight::MEDIUM);
    match &s.update {
        UpdateState::Ready { version, .. } | UpdateState::Restarting { version } => {
            let ready = matches!(s.update, UpdateState::Ready { .. });
            let notes = match &s.update {
                UpdateState::Ready { notes, .. } => parse_notes(notes.as_deref()),
                _ => Vec::new(),
            };
            let line = if ready { "Downloaded and verified. It installs when you quit Kudzu, or restart now. Takes a second." } else { "Installing… Kudzu will reopen by itself." };
            let action = if ready {
                Button::new("Restart to update").size(Size::Sm).icon(&icons::REFRESH_CW).on_press(restart).build(th)
            } else {
                spinner(16.0, t.muted_foreground)
            };
            let head = row()
                .center_y()
                .gap(24.0)
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .child(
                            row()
                                .center_y()
                                .gap(8.0)
                                .child(icon(&icons::SPARKLES, 14.0).color(t.iris).a11y_hidden())
                                .child(text(format!("Kudzu {version} is ready")).font(title_font).color(t.foreground)),
                        )
                        .child(desc(th, line)),
                )
                .child(action);
            let mut e = whole(col()).gap(12.0).py(14.0).child(head);
            if !notes.is_empty() {
                let relaxed = ty::XS.lh(12.0 * 1.625);
                let mut body = col().px(12.0).py(10.0).selection_group(true);
                for (i, sec) in notes.iter().enumerate() {
                    let mut block = col();
                    if i > 0 {
                        block = block.mt(8.0);
                    }
                    if let Some(h) = &sec.heading {
                        block = block.child(text(h.clone()).font(relaxed.weight_of(weight::MEDIUM)).color(Theme::tint(t.foreground, 0.9)).selectable(true).mb(2.0));
                    }
                    let mut list = col().gap(2.0);
                    for it in &sec.items {
                        list = list.child(
                            row()
                                .items_start()
                                .child(rune::el().w(16.0).h(12.0 * 1.625).child(rune::el().size(5.0, 5.0).radius(radius::FULL).fill(t.faint).absolute(4.5, (12.0 * 1.625 - 5.0) / 2.0)))
                                .child(text(it.clone()).font(relaxed).color(t.muted_foreground).selectable(true).wrap().grow(1.0).min_w(0.0)),
                        );
                    }
                    body = body.child(block.child(list));
                }
                e = e.child(
                    still(col().key("release-notes").max_h(192.0).scroll(ScrollSpec { stick_bottom: false }))
                        .radius(radius::LG)
                        .fill(Theme::tint(t.foreground, 0.025))
                        .elevate(&t.elev_well)
                        .child(body),
                );
            }
            e.child(releases_link(th))
        }
        UpdateState::Downloading { version, downloaded, total } => {
            let pct = total.filter(|&n| n > 0).map(|n| ((*downloaded as f64 / n as f64) * 100.0).round().min(100.0) as u32);
            let line = format!("In the background{}. You'll be asked before anything restarts.", pct.map_or(String::new(), |p| format!(" · {p}%")));
            let bar = rune::el()
                .h(4.0)
                .radius(radius::FULL)
                .clip_rounded(radius::FULL)
                .fill(Theme::tint(t.foreground, 0.06))
                .elevate(&t.elev_well)
                .role("progressbar")
                .label("Download progress")
                .range_value(pct.unwrap_or(0) as f64, 0.0, 100.0)
                .child(
                    rune::el()
                        .key("update-progress")
                        .h_full()
                        .w_frac(pct.unwrap_or(30) as f32 / 100.0)
                        .radius(radius::FULL)
                        .fill(t.iris)
                        .motion(rune::Spring::new(0.3, 0.0)),
                );
            whole(col())
                .gap(10.0)
                .py(14.0)
                .child(col().child(text(format!("Downloading Kudzu {version}")).font(title_font).color(t.foreground)).child(desc(th, line)))
                .child(bar)
        }
        state => {
            let checking = matches!(state, UpdateState::Checking);
            let (line, tone) = match state {
                UpdateState::Checking => ("Checking…".to_string(), t.muted_foreground),
                UpdateState::UpToDate { checked_at } => {
                    let ms = chrono::DateTime::parse_from_rfc3339(checked_at).map_or(0, |d| d.timestamp_millis());
                    let ago = kudzu_core::client::stats::relative_time(ms, chrono::Utc::now().timestamp_millis(), &chrono::Local).to_lowercase();
                    (format!("You're on the latest version · checked {ago}"), t.muted_foreground)
                }
                UpdateState::Error { message, needs_move: true } => (message.clone(), t.warning),
                UpdateState::Error { message, .. } => (format!("Couldn't check: {message}"), t.destructive),
                _ => ("New versions download in the background and install when you quit.".to_string(), t.muted_foreground),
            };
            let label = if matches!(state, UpdateState::Error { needs_move: false, .. }) { "Try again" } else { "Check now" };
            let mut b = Button::new(if checking { "" } else { label }).size(Size::Sm).variant(Variant::Secondary).disabled(checking).id("check-updates");
            if checking {
                let font = rune::TextStyle { size: 12.8, ..ty::SM.weight_of(weight::MEDIUM) };
                b = b.child(row().ml(-4.0).gap(4.0).center_y().child(spinner(14.0, t.secondary_foreground)).child(text(label).font(font).color(t.secondary_foreground)).a11y_hidden());
            } else {
                b = b.icon(&icons::REFRESH_CW);
            }
            row_el(th, "Software update", Some(text(line).font(ty::XS).color(tone).wrap()), vec![b.on_press(check).build(th).label(label)])
        }
    }
}

/// "All release notes": `self-start text-xs text-muted-foreground underline-offset-2
/// hover:text-foreground hover:underline`.
fn releases_link(th: &Theme) -> El {
    let t = &th.t;
    let rule = rune::el().pin(Some(0.0), None, Some(0.0), Some(1.0)).h(1.0).fill(t.foreground).group_opacity(0.0, 1.0).a11y_hidden();
    hover_fast(rune::el())
        .key("all-release-notes")
        .role("button")
        .label("All release notes")
        .self_start()
        .focusable(true)
        .focus_ring(2.0, 2.0, Theme::tint(t.ring, 0.5))
        .child(text("All release notes").font(ty::XS).color(t.muted_foreground).group_color(t.foreground).a11y_hidden())
        .child(rule)
        .on_press(|_: &mut MainUi| {
            let _ = crate::backend::system::open_url(RELEASES_URL);
        })
}
