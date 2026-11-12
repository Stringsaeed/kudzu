//! What the clipboard screen does to clips. Calls that read or write files (copy, restore,
//! the full text, open, usage) run on a worker and land in [`finish_jobs`]; pin, secret and
//! delete only change the in-memory history, so they run in place.
//!
//! Toasts name a clip by its first 40 characters, except a secret, which is never named.

use kudzu_app::clipboard::{ClipboardError, DeletedClips, Flavor};
use kudzu_app::tasks::Task;
use kudzu_app::watch::{Revision, Subscription};
use kudzu_core::clipboard::facts::{concealed, describe, has_text};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::Tx;
use uuid::Uuid;

use super::{ClearDialog, COPIED_SECS, LIST_KEY};
use crate::main::nav;
use crate::main::toasts::{ToastAction, ToastSpec};
use crate::shell::{MainState, NewSnippet, Route};

pub type Failure = ClipboardError;
pub type Batch = DeletedClips;

const COPY_TOAST_SECS: f64 = 1.6;
const UNDO_TOAST_SECS: f64 = 6.0;

pub(crate) enum JobKind {
    Copy {
        id: Uuid,
        plain: bool,
        task: Task<Result<(), Failure>>,
    },
    Restore {
        deleted: Batch,
        task: Task<Result<(), Failure>>,
    },
    Text {
        task: Task<Result<Option<String>, Failure>>,
    },
    Open {
        task: Task<Result<(), Failure>>,
    },
    Usage {
        task: Task<u64>,
    },
}

/// A worker's task plus the subscription that wakes `main` when it finishes.
pub(crate) struct Job {
    kind: JobKind,
    _wake: Subscription,
}

fn start<T: Send + 'static>(
    s: &mut MainState,
    name: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<(Task<T>, Subscription)> {
    let task = Task::spawn(name, work).ok()?;
    let handle = s.shell.handles().main.clone();
    let wake = task.ready().subscribe(move || handle.wake());
    Some((task, wake))
}

fn push(s: &mut MainState, kind: JobKind, wake: Subscription) {
    s.clipboard.jobs.push(Job { kind, _wake: wake });
}

fn say(s: &mut MainState, text: impl Into<String>) {
    s.clipboard.announcement = text.into();
}

fn toast(s: &mut MainState, key: &str, spec: ToastSpec<MainState>) {
    let now = s.frame.clock_secs;
    s.frame.toasts.show(key, spec, now);
}

fn failure_toast(s: &mut MainState, title: &str, error: &Failure) {
    toast(
        s,
        "clip-failed",
        ToastSpec::new(title).description(error.to_string()),
    );
}

/// `↩` or `⌘C` (and `⇧↩`, which flips plain and formatted). Copying our own write is not
/// re-captured: the service's own-change ring skips it.
pub fn copy_active(s: &mut MainState, flip: bool, _tx: &mut Tx) {
    let Some(e) = s.clipboard.clips.list.active_entry() else {
        return;
    };
    let id = e.id;
    let plain = e.rich && (s.clipboard.prefs.paste_plain != flip);
    let service = s.svc.clipboard.clone();
    let flavor = if plain {
        Flavor::Plain
    } else {
        Flavor::Original
    };
    match start(s, "kudzu-clip-copy", move || service.copy(id, flavor)) {
        Some((task, wake)) => push(s, JobKind::Copy { id, plain, task }, wake),
        None => failure_toast(s, "Couldn't copy", &ClipboardError::WorkerStopped),
    }
}

/// `⌘P`: pins the selection, or unpins it when every clip in it is pinned.
pub fn toggle_pin(s: &mut MainState) {
    let ids = s.clipboard.clips.list.selection();
    let entries: Vec<&ClipEntry> = s.clipboard.clips.list.entries(&ids).collect();
    if entries.is_empty() {
        return;
    }
    let pin = !entries.iter().all(|e| e.pinned);
    for id in &ids {
        if let Err(error) = s.svc.clipboard.pin(*id, pin) {
            log::warn!("pinning a clip failed: {error}");
        }
    }
    let said = match (pin, ids.len()) {
        (true, 1) => "Pinned. It won't expire.".to_owned(),
        (true, n) => format!("Pinned {n} items"),
        (false, _) => "Unpinned".to_owned(),
    };
    say(s, said);
}

/// The delete keys and the Delete buttons: removes the selection, posts the 6-second Undo
/// toast and puts keyboard focus back on the list.
pub fn delete_selection(s: &mut MainState, tx: &mut Tx) {
    let ids = s.clipboard.clips.list.selection();
    if ids.is_empty() {
        return;
    }
    s.clipboard.clips.list.clear_multi();
    let deleted = match s.svc.clipboard.delete(&ids) {
        Ok(deleted) if !deleted.is_empty() => deleted,
        Ok(_) => return,
        Err(error) => return failure_toast(s, "Couldn't delete", &error),
    };
    let show = s.clipboard.prefs.show_secrets;
    let entries = deleted.entries();
    let title = match entries {
        [one] if concealed(one, show) => "Deleted a secret".to_owned(),
        [one] => format!(
            "Deleted “{}”",
            describe(one).title.chars().take(40).collect::<String>()
        ),
        many => format!("Deleted {} items", many.len()),
    };
    say(s, title.clone());
    offer_undo(s, title, deleted);
    nav::focus_list_pane(s, LIST_KEY, tx);
}

fn offer_undo(s: &mut MainState, title: String, deleted: Batch) {
    let spec = ToastSpec::new(title)
        .duration_secs(UNDO_TOAST_SECS)
        .action(ToastAction::new("Undo", true, move |st: &mut MainState| {
            restore(st, deleted.clone());
        }));
    toast(s, "clip-undo", spec);
}

fn restore(s: &mut MainState, deleted: Batch) {
    let service = s.svc.clipboard.clone();
    let again = deleted.clone();
    match start(s, "kudzu-clip-restore", move || service.restore(deleted)) {
        Some((task, wake)) => push(
            s,
            JobKind::Restore {
                deleted: again,
                task,
            },
            wake,
        ),
        None => failure_toast(s, "Couldn't restore", &ClipboardError::WorkerStopped),
    }
}

/// `⌘S`: the full plain text becomes a new snippet in the library.
pub fn save_as_snippet(s: &mut MainState) {
    let Some(e) = s
        .clipboard
        .clips
        .list
        .active_entry()
        .filter(|e| has_text(e))
    else {
        return;
    };
    let id = e.id;
    let service = s.svc.clipboard.clone();
    if let Some((task, wake)) = start(s, "kudzu-clip-text", move || service.text(id)) {
        push(s, JobKind::Text { task }, wake);
    }
}

/// `⌘O`: a link opens in the browser, a file in its app.
pub fn open_active(s: &mut MainState, tx: &mut Tx) {
    let Some(e) = s.clipboard.clips.list.active_entry() else {
        return;
    };
    match e.kind {
        ClipKind::Link => {
            let url = e.text.clone().unwrap_or_default();
            if tx.open_url(url.trim()).is_err() {
                toast(s, "clip-failed", ToastSpec::new("Couldn't open it"));
            }
        }
        ClipKind::File => file_job(s, false),
        _ => {}
    }
}

/// `⌘R`: shows the first copied file in Finder.
pub fn reveal_active(s: &mut MainState) {
    if s.clipboard
        .clips
        .list
        .active_entry()
        .is_some_and(|e| e.kind == ClipKind::File)
    {
        file_job(s, true);
    }
}

fn file_job(s: &mut MainState, reveal: bool) {
    let Some(path) = s
        .clipboard
        .clips
        .list
        .active_entry()
        .and_then(|e| e.files.first().cloned())
    else {
        return;
    };
    let files = s.svc.files.clone();
    let work = move || {
        let path = std::path::Path::new(&path);
        let result = if reveal {
            files.reveal(path)
        } else {
            files.open(path)
        };
        result.map_err(ClipboardError::from)
    };
    if let Some((task, wake)) = start(s, "kudzu-clip-open", work) {
        push(s, JobKind::Open { task }, wake);
    }
}

/// The footer's "Hide as a secret" and "Not a secret — show it".
pub fn toggle_secret(s: &mut MainState) {
    let Some(e) = s
        .clipboard
        .clips
        .list
        .active_entry()
        .filter(|e| has_text(e))
    else {
        return;
    };
    let (id, hide) = (e.id, !e.is_secret());
    if let Err(error) = s.svc.clipboard.set_secret(id, Some(hide)) {
        failure_toast(s, "Couldn't change that", &error);
    }
}

pub fn toggle_recording(s: &mut MainState) {
    let on = !s.clipboard.prefs.recording;
    if let Err(error) = s.svc.settings.update(|st| st.clipboard_enabled = on) {
        log::warn!("recording could not be switched: {error}");
    }
}

pub fn set_retention(s: &mut MainState, days: u32) {
    if let Err(error) = s.svc.settings.set_clipboard_retention_days(days) {
        log::warn!("retention could not be changed: {error}");
    }
}

pub fn open_clear(s: &mut MainState) {
    if !s.clipboard.clips.list.history().entries.is_empty() {
        s.clipboard.clear = Some(ClearDialog::default());
    }
}

pub fn confirm_clear(s: &mut MainState) {
    let Some(dialog) = s.clipboard.clear.take() else {
        return;
    };
    match s.svc.clipboard.clear(dialog.include_pinned) {
        Ok(n) => {
            let title = if n == 1 {
                "Deleted 1 item".to_owned()
            } else {
                format!("Deleted {n} items")
            };
            say(s, format!("Cleared {n} items"));
            toast(
                s,
                "clip-cleared",
                ToastSpec::new(title).duration_secs(COPY_TOAST_SECS),
            );
        }
        Err(error) => failure_toast(s, "Couldn't clear history", &error),
    }
}

/// "Show everything": drops the query and opens the whole history.
pub fn show_everything(s: &mut MainState, tx: &mut Tx) {
    s.clipboard.search.replace_text("");
    s.clipboard.search.focused = false;
    nav::view(
        s,
        crate::main::state::MainView::Clipboard(crate::main::state::ClipFilter::All),
        tx,
    );
}

/// Starts the size check when the history changed. `usage` walks the store, so it runs on a
/// worker; a scene that fixed the number keeps it.
pub fn refresh_usage(s: &mut MainState, revision: Revision) {
    if s.clipboard.usage_fixed || s.clipboard.seen.usage_for == Some(revision) {
        return;
    }
    if s.clipboard
        .jobs
        .iter()
        .any(|j| matches!(j.kind, JobKind::Usage { .. }))
    {
        return;
    }
    s.clipboard.seen.usage_for = Some(revision);
    let service = s.svc.clipboard.clone();
    if let Some((task, wake)) = start(s, "kudzu-clip-usage", move || service.usage()) {
        push(s, JobKind::Usage { task }, wake);
    }
}

/// Takes the answers of finished jobs. True when one changed what the screen shows.
pub fn finish_jobs(s: &mut MainState) -> bool {
    let mut changed = false;
    let now = s.frame.clock_secs;
    let jobs = std::mem::take(&mut s.clipboard.jobs);
    for job in jobs {
        let Job { kind, _wake } = job;
        let done = match kind {
            JobKind::Copy { id, plain, task } => match task.take() {
                None => Err(JobKind::Copy { id, plain, task }),
                Some(result) => {
                    match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        Ok(()) => {
                            let title = if plain {
                                "Copied as plain text"
                            } else {
                                "Copied"
                            };
                            toast(
                                s,
                                "clip-copied",
                                ToastSpec::new(title).duration_secs(COPY_TOAST_SECS),
                            );
                            say(s, "Copied to clipboard");
                            s.clipboard.clips.look.copied = Some(id);
                            s.clipboard.copied_until = Some(now + COPIED_SECS);
                        }
                        Err(error) => failure_toast(s, "Couldn't copy", &error),
                    }
                    Ok(())
                }
            },
            JobKind::Restore { deleted, task } => match task.take() {
                None => Err(JobKind::Restore { deleted, task }),
                Some(result) => {
                    if let Err(error) = result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        let spec = ToastSpec::new("Couldn't restore")
                            .description(error.to_string())
                            .action(ToastAction::new("Undo", true, move |st: &mut MainState| {
                                restore(st, deleted.clone());
                            }));
                        toast(s, "clip-failed", spec);
                    }
                    Ok(())
                }
            },
            JobKind::Text { task } => match task.take() {
                None => Err(JobKind::Text { task }),
                Some(result) => {
                    match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        Ok(Some(content)) => {
                            s.open_route(Route::NewSnippet(NewSnippet {
                                content,
                                ..NewSnippet::default()
                            }));
                        }
                        Ok(None) => {}
                        Err(error) => failure_toast(s, "Couldn't save as a snippet", &error),
                    }
                    Ok(())
                }
            },
            JobKind::Open { task } => match task.take() {
                None => Err(JobKind::Open { task }),
                Some(result) => {
                    if let Err(error) = result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        failure_toast(s, "Couldn't open it", &error);
                    }
                    Ok(())
                }
            },
            JobKind::Usage { task } => match task.take() {
                None => Err(JobKind::Usage { task }),
                Some(result) => {
                    if let Ok(bytes) = result {
                        s.clipboard.usage = Some(bytes);
                    }
                    Ok(())
                }
            },
        };
        match done {
            Ok(()) => changed = true,
            Err(kind) => s.clipboard.jobs.push(Job { kind, _wake }),
        }
    }
    changed
}
