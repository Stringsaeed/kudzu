//! `Data` in settings.tsx (with `data-actions.ts`): import from Espanso (detected or chosen),
//! Raycast, TextExpander and Kudzu JSON; export JSON and Espanso; the backups list, Back up now
//! and the data folder.

use std::path::PathBuf;

use rune::{col, icon, row, text, Cx, El, FileDialogOptions, Tx};

use kudzu_core::interchange::backup::BackupReason;
use kudzu_core::interchange::coordinator::ImportSummary;

use super::{desc, group, page, row_el, XS2_BODY};
use crate::backend::commands;
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::toast::Toast;
use crate::ui::main::insights::whole;
use crate::ui::main::MainUi;
use crate::ui::theme::text as ty;
use crate::ui::theme_for;

/// `IMPORT_HINTS`, shared with onboarding.
pub const RAYCAST_HINT: &str = "In Raycast, run Export Snippets, then choose the .json file.";
pub const TEXTEXPANDER_HINT: &str = "In TextExpander, export a group as .textexpander or CSV.";

/// The backups (and, on mount, Espanso folders on this Mac).
pub(super) fn reload(s: &MainUi, espanso: bool) {
    s.spawn(|| commands::list_backups().unwrap_or_default(), |s, list, _| page(s).backups = list);
    if espanso {
        s.spawn(commands::detect_espanso, |s, found, _| page(s).espanso = found);
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// `reportImport`.
fn report(s: &mut MainUi, r: &ImportSummary) {
    let mut what = plural(r.imported_snippets, "snippet");
    if r.imported_groups > 0 {
        what = format!("{what} in {}", plural(r.imported_groups, "group"));
    }
    let description = match r.warnings.first() {
        Some(w) => format!("{} imported with changes — e.g. {}", plural(r.warnings.len(), "item"), w.message),
        None => "A backup was saved first, just in case.".to_string(),
    };
    s.toasts.push(Toast::success(format!("Imported {what}")).description(description));
}

fn import_failed(s: &mut MainUi, e: impl ToString) {
    s.toasts.push(Toast::error("Import failed").description(e.to_string()));
}

#[derive(Clone, Copy)]
enum ImportKind {
    File,
    Raycast,
    TextExpander,
    EspansoFolder,
}

/// `runImport`: asks for a file (or an Espanso folder) and imports it on a worker.
fn run_import(kind: ImportKind, tx: &mut Tx) {
    let land = |s: &mut MainUi, r: Result<ImportSummary, String>, _: &mut Tx| match r {
        Ok(r) => report(s, &r),
        Err(e) => import_failed(s, e),
    };
    let options = |title: &str| FileDialogOptions { title: title.into(), ..Default::default() };
    let answer = move |s: &mut MainUi, result: rune::FileDialogResult, _: &mut Tx| match result {
        Ok(Some(path)) => match kind {
            ImportKind::EspansoFolder => s.spawn(move || commands::import_espanso(path), land),
            _ => s.spawn(move || commands::import_file(path), land),
        },
        Ok(None) => {}
        Err(e) => import_failed(s, e),
    };
    match kind {
        ImportKind::EspansoFolder => tx.open_folder(options("Choose your Espanso config folder"), answer),
        ImportKind::File => tx.open_file(options("Import snippets").filter("Snippet files", &["json", "yml", "yaml", "textexpander", "csv"]), answer),
        ImportKind::Raycast => tx.open_file(options("Choose your Raycast snippets export").filter("Raycast snippets", &["json"]), answer),
        ImportKind::TextExpander => tx.open_file(options("Choose your TextExpander export").filter("TextExpander", &["textexpander", "csv"]), answer),
    }
}

fn export_failed(s: &mut MainUi, e: impl ToString) {
    s.toasts.push(Toast::error("Export failed").description(e.to_string()));
}

/// `runExport("json")`.
fn export_json(tx: &mut Tx) {
    let stamp = chrono::Utc::now().format("%Y-%m-%d");
    let options = FileDialogOptions { title: "Export library".into(), file_name: Some(format!("kudzu-{stamp}.json")), ..Default::default() }.filter("Kudzu library", &["json"]);
    tx.save_file(options, |s: &mut MainUi, result, _| match result {
        Ok(Some(path)) => {
            let shown = path.to_string_lossy().into_owned();
            s.spawn(move || commands::export_json(path), move |s, r, _| match r {
                Ok(()) => s.toasts.push(Toast::success("Library exported").description(shown.clone())),
                Err(e) => export_failed(s, e),
            });
        }
        Ok(None) => {}
        Err(e) => export_failed(s, e),
    });
}

/// `runExport("espanso")`.
fn export_espanso(tx: &mut Tx) {
    let options = FileDialogOptions { title: "Choose a folder for the Espanso match files".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut MainUi, result, _| match result {
        Ok(Some(dir)) => {
            let shown = dir.to_string_lossy().into_owned();
            s.spawn(move || commands::export_espanso(dir), move |s, r, _| match r {
                Ok(n) => s.toasts.push(
                    Toast::success(format!("Exported {n} Espanso file{}", if n == 1 { "" } else { "s" }))
                        .description(format!("{shown} · Formatting and images export as plain text. Use Kudzu JSON to keep them.")),
                ),
                Err(e) => export_failed(s, e),
            });
        }
        Ok(None) => {}
        Err(e) => export_failed(s, e),
    });
}

fn reason_label(r: BackupReason) -> &'static str {
    match r {
        BackupReason::Daily => "Daily",
        BackupReason::PreImport => "Before import",
        BackupReason::PreRestore => "Before restore",
        BackupReason::Manual => "Manual",
        BackupReason::PreUpdate => "Before update",
    }
}

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> Vec<El> {
    let th = theme_for(cx);
    let t = &th.t;
    let st = &s.settings_page;

    let mut import_rows = Vec::new();
    for d in &st.espanso {
        let path = d.path.clone();
        let shown = d.path.to_string_lossy().into_owned();
        import_rows.push(row_el(
            th,
            "Espanso found on this Mac",
            Some(desc(th, format!("{} snippets in {} files · {shown}", d.snippet_count, d.group_count))),
            vec![Button::new("Import")
                .size(Size::Sm)
                .icon(&icons::DOWNLOAD)
                .id(format!("espanso-import-{shown}"))
                .on_press(move |s: &mut MainUi| {
                    let path: PathBuf = path.clone();
                    s.spawn(move || commands::import_espanso(path), |s, r, _| match r {
                        Ok(r) => s.toasts.push(Toast::success(format!("Imported {} snippets", r.imported_snippets))),
                        Err(e) => import_failed(s, e),
                    });
                })
                .build(th)
                .label(format!("Import Espanso snippets from {shown}"))],
        ));
    }
    let choose = |name: &str, kind: ImportKind| {
        Button::new("Choose file…")
            .size(Size::Sm)
            .variant(Variant::Secondary)
            .id(format!("choose-{name}"))
            .on_press_tx(move |_: &mut MainUi, tx| run_import(kind, tx))
            .build(th)
            .label(format!("Choose file… for {name}"))
    };
    import_rows.push(row_el(th, "Raycast", Some(desc(th, RAYCAST_HINT)), vec![choose("Raycast", ImportKind::Raycast)]));
    import_rows.push(row_el(th, "TextExpander", Some(desc(th, TEXTEXPANDER_HINT)), vec![choose("TextExpander", ImportKind::TextExpander)]));
    import_rows.push(row_el(
        th,
        "Kudzu or Espanso",
        Some(desc(th, "A Kudzu .json library, or Espanso .yml files and folders.")),
        vec![
            choose("Kudzu or Espanso", ImportKind::File),
            Button::new("Espanso folder…").size(Size::Sm).variant(Variant::Ghost).on_press_tx(|_: &mut MainUi, tx| run_import(ImportKind::EspansoFolder, tx)).build(th),
        ],
    ));
    let import = group(th, "Import", Some("Bring snippets from Raycast, TextExpander, Espanso or another Kudzu library. A backup is saved first."), import_rows);

    let export = group(
        th,
        "Export",
        Some("Your snippets are always yours to take elsewhere."),
        vec![
            row_el(
                th,
                "Kudzu library",
                Some(desc(th, "Everything, lossless — snippets, groups, rules and usage.")),
                vec![Button::new("Export JSON").size(Size::Sm).variant(Variant::Secondary).icon(&icons::UPLOAD).on_press_tx(|_: &mut MainUi, tx| export_json(tx)).build(th)],
            ),
            row_el(
                th,
                "Espanso",
                Some(desc(th, "Best-effort YAML so you can switch back any time.")),
                vec![Button::new("Export YAML").size(Size::Sm).variant(Variant::Secondary).icon(&icons::UPLOAD).on_press_tx(|_: &mut MainUi, tx| export_espanso(tx)).build(th)],
            ),
        ],
    );

    let mut backup_rows = Vec::new();
    if st.backups.is_empty() {
        backup_rows.push(whole(rune::el()).py(16.0).child(text("No backups yet.").font(ty::XS).color(t.muted_foreground)));
    }
    let now = chrono::Utc::now().timestamp_millis();
    for b in st.backups.iter().take(14) {
        let local = b.created_at.with_timezone(&chrono::Local);
        let when = format!("{}, {}", kudzu_core::client::js::date_medium(&local), kudzu_core::client::js::time_short(&local));
        let ago = kudzu_core::client::stats::relative_time(b.created_at.timestamp_millis(), now, &chrono::Local);
        let path = b.path.clone();
        backup_rows.push(
            whole(row())
                .key(format!("backup-{}", b.filename))
                .center_y()
                .gap(12.0)
                .py(10.0)
                .child(icon(&icons::ROTATE_CCW_CLOCK, 16.0).color(t.faint).a11y_hidden())
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .child(text(when).font(ty::BASE).color(t.foreground))
                        .child(text(format!("{} · {ago}", reason_label(b.reason))).font(XS2_BODY).color(t.faint)),
                )
                .child(
                    Button::new("Restore")
                        .size(Size::Sm)
                        .variant(Variant::Ghost)
                        .icon(&icons::ROTATE_CCW)
                        .id(format!("restore-{}", b.filename))
                        .on_press(move |s: &mut MainUi| {
                            let path = path.clone();
                            s.spawn(move || commands::restore_backup(path), |s, r, _| match r {
                                Ok(()) => {
                                    s.toasts.push(Toast::success("Library restored").description("Your previous library was backed up first."));
                                    reload(s, false);
                                }
                                Err(e) => s.toasts.push(Toast::error("Restore failed").description(e)),
                            });
                        })
                        .build(th),
                ),
        );
    }
    backup_rows.push(
        whole(row())
            .gap(8.0)
            .py(12.0)
            .child(
                Button::new("Back up now")
                    .size(Size::Sm)
                    .variant(Variant::Secondary)
                    .on_press(|s: &mut MainUi| {
                        s.spawn(commands::backup_now, |s, r, _| {
                            if r.is_ok() {
                                s.toasts.push(Toast::success("Backup saved"));
                            }
                            reload(s, false);
                        })
                    })
                    .build(th),
            )
            .child(
                Button::new("Show data folder")
                    .size(Size::Sm)
                    .variant(Variant::Ghost)
                    .icon(&icons::FOLDER_OPEN)
                    .on_press(|_: &mut MainUi| commands::reveal_data_folder())
                    .build(th),
            ),
    );
    let backups = group(th, "Backups", Some("Kudzu keeps a snapshot every day and before every import — the last 14."), backup_rows);
    vec![import, export, backups]
}
