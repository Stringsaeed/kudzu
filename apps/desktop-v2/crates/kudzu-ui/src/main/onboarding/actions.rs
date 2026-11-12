use crate::shell::MainState;
use kudzu_app::onboarding::{DetectedFolder, ImportSource, ImportTally, Step};
use kudzu_app::tasks::Task;
use rune::{FileDialogOptions, Tx};

pub fn next(s: &mut MainState) {
    detect(s);
    let _ = s.svc.onboarding.advance();
    focus_practice(s);
}
pub fn skip(s: &mut MainState) {
    let _ = s.svc.onboarding.skip();
    focus_practice(s);
}
pub fn back(s: &mut MainState) {
    let was_practice = s.svc.onboarding.step() == Step::TryIt;
    if s.svc.onboarding.back().is_ok() && was_practice {
        s.onboarding.practice = rune::TextEdit::default();
    }
    s.onboarding.blur_inputs();
}
fn focus_practice(s: &mut MainState) {
    s.onboarding.practice.focused = s.svc.onboarding.step() == Step::TryIt;
}
pub fn finish(s: &mut MainState) {
    if s.onboarding.opening() || s.svc.onboarding.snapshot().busy() {
        return;
    }
    s.onboarding
        .opening
        .store(true, std::sync::atomic::Ordering::Release);
    let opening = s.onboarding.opening.clone();
    s.onboarding.scene_finish_error = false;
    let m = s.svc.onboarding.clone();
    let handle = s.shell.handles().main.clone();
    s.onboarding.task = Task::spawn("onboarding finish", move || {
        let _ = m.finish();
        opening.store(false, std::sync::atomic::Ordering::Release);
        handle.wake();
    })
    .ok();
    super::track_task(s);
    if s.onboarding.task.is_none() {
        s.onboarding.scene_finish_error = true;
        s.onboarding
            .opening
            .store(false, std::sync::atomic::Ordering::Release);
    }
}
pub fn detect(s: &mut MainState) {
    if s.onboarding.detecting {
        return;
    }
    let (m, data) = (s.svc.onboarding.clone(), s.svc.data.clone());
    s.onboarding.detecting = true;
    let handle = s.shell.handles().main.clone();
    s.onboarding.task = Task::spawn("onboarding detection", move || {
        match data.detect_espanso() {
            Ok(task) => m.record_detection(
                task.wait()
                    .map(|folders| {
                        folders
                            .into_iter()
                            .map(|f| DetectedFolder {
                                path: f.path,
                                snippet_count: f.snippet_count,
                            })
                            .collect()
                    })
                    .map_err(|_| ()),
            ),
            Err(_) => m.record_detection(Err(())),
        }
        handle.wake();
    })
    .ok();
    super::track_task(s);
    if s.onboarding.task.is_none() {
        s.svc.onboarding.record_detection(Err(()));
    }
}
pub fn pick(s: &mut MainState, tx: &mut Tx, source: ImportSource) {
    let m = &s.svc.onboarding;
    if m.begin_pick().is_err() {
        return;
    }
    let options = match source {
        ImportSource::Espanso => FileDialogOptions {
            title: "Choose your Espanso config folder".into(),
            ..Default::default()
        },
        ImportSource::Raycast => FileDialogOptions {
            title: "Choose your Raycast snippets export".into(),
            ..Default::default()
        }
        .filter("Raycast", &["json"]),
        ImportSource::TextExpander => FileDialogOptions {
            title: "Choose your TextExpander export".into(),
            ..Default::default()
        }
        .filter("TextExpander", &["textexpander", "csv"]),
    };
    let callback = move |s: &mut MainState, result: rune::FileDialogResult, tx: &mut Tx| {
        s.svc.onboarding.end_pick();
        match result {
            Ok(Some(path)) => import(s, source, vec![path], false),
            Ok(None) => {}
            Err(error) => s.svc.onboarding.fail_import(source, error.to_string()),
        }
        tx.focus(source_key(source), true);
    };
    if source == ImportSource::Espanso {
        tx.open_folder(options, callback);
    } else {
        tx.open_file(options, callback);
    }
}
pub fn source_key(source: ImportSource) -> &'static str {
    match source {
        ImportSource::Espanso => "onboarding-espanso",
        ImportSource::Raycast => "onboarding-raycast",
        ImportSource::TextExpander => "onboarding-textexpander",
    }
}
pub fn detected(s: &mut MainState) {
    let m = &s.svc.onboarding;
    let paths = m
        .pending_espanso_folders()
        .into_iter()
        .map(|f| f.path)
        .collect();
    import(s, ImportSource::Espanso, paths, true);
}
fn import(
    s: &mut MainState,
    source: ImportSource,
    paths: Vec<std::path::PathBuf>,
    accumulate: bool,
) {
    let (m, data) = (s.svc.onboarding.clone(), s.svc.data.clone());
    if m.snapshot().busy() || m.begin_import(source).is_err() {
        return;
    }
    let handle = s.shell.handles().main.clone();
    let task = Task::spawn("onboarding import", move || {
        for path in paths {
            let result = data
                .import(path.clone())
                .and_then(|task| task.wait().map_err(kudzu_app::data::DataError::from)?);
            let summary = match result {
                Ok(summary) => summary,
                Err(error) => {
                    let mut message = error.to_string();
                    if accumulate {
                        if let kudzu_app::onboarding::Detection::Found { folders } =
                            &m.snapshot().detection
                        {
                            let total = folders.len();
                            let completed = total - m.pending_espanso_folders().len();
                            if completed > 0 {
                                message = format!(
                                    concat!(
                                        "Imported {} of {} folders; ",
                                        "retry brings in the rest. {}"
                                    ),
                                    completed, total, message
                                );
                            }
                        }
                    }
                    m.fail_import(source, message);
                    handle.update(move |_, tx| tx.focus(source_key(source), true));
                    handle.wake();
                    return;
                }
            };
            m.record_import(
                source,
                ImportTally {
                    imported_snippets: summary.imported_snippets,
                    merged_snippets: summary
                        .imported_snippets
                        .saturating_sub(summary.added_snippets),
                    warnings: summary.warnings.len(),
                    first_warning: summary.warnings.first().map(|w| w.message.clone()),
                    path: Some(path),
                },
                accumulate,
            );
        }
        m.end_import(source);
        handle.update(move |s, tx| {
            s.refresh_library_snapshot();
            tx.focus(
                if source == ImportSource::Espanso {
                    "onboarding-continue"
                } else {
                    source_key(source)
                },
                true,
            );
        });
        handle.wake();
    });
    match task {
        Ok(task) => s.onboarding.task = Some(task),
        Err(error) => s.svc.onboarding.fail_import(source, error.to_string()),
    }
    super::track_task(s);
}

pub fn open_permissions(s: &mut MainState) {
    use std::sync::atomic::Ordering;
    s.onboarding.permission_opening = true;
    s.onboarding.permission_error = None;
    s.onboarding.permission_ack.store(false, Ordering::Release);
    let ack = s.onboarding.permission_ack.clone();
    let main = s.shell.handles().main.clone();
    s.shell.open_permissions();
    s.shell.handles().permissions.update(move |_, _| {
        ack.store(true, Ordering::Release);
        main.wake();
    });
}
