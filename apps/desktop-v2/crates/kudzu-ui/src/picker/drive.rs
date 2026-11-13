//! Applies the picker's effects to the window: services, workers, polls and timers.
//!
//! The main thread reads no file, writes no pasteboard and activates no app: a copy, a paste,
//! a restore, a snippet's text and a file open are tasks, and `poll` takes their answers. A
//! paste is two of them: the copy runs with the picker still showing, so a clip that cannot
//! be copied says so in the footer; the picker then hides, and `paste_copied` (up to about
//! 850 ms) runs on a worker.
//!
//! Timers are deadlines on the UI clock (`Picker::flash`, `Picker::hide`); `poll` ends them.
//! Clip text never reaches a log: only error sentences do.

use kudzu_app::clipboard::{ClipboardError, DeletedClips, Flavor, PasteOutcome, PasteTicket};
use kudzu_app::tasks::Task;
use kudzu_app::watch::Subscription;
use kudzu_core::clipboard::facts::has_text;
use kudzu_core::clipboard::ClipKind;
use rune::{Key, KeyEvent, Tx};
use uuid::Uuid;

use super::keys::{self, Ctx, Effect};
use super::model::{
    plain_for, COPY_HIDE_AFTER, ERROR_FLASH_SECS, FALLBACK_FLASH_SECS, FALLBACK_HIDE_AFTER,
    FALLBACK_NOTICE, FLASH_SECS, UNDO_FLASH_SECS,
};
use crate::clip::content::loader::Media;
use crate::shell::windows::PickerState;
use crate::shell::{NewSnippet, Route, SettingsPane};
use crate::widgets::key_hints::hint_for;

pub const FIELD_KEY: &str = "picker-field";

/// What a copy is for once it lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum After {
    /// Hide, then paste what was copied.
    Paste,
    /// ⌘↩: "Copied", then hide.
    Only,
    /// No Accessibility permission: say so, then hide.
    Fallback,
}

type Wake = Subscription;

pub(crate) enum Job {
    Copy {
        then: After,
        task: Task<Result<(), ClipboardError>>,
        _wake: Wake,
    },
    /// A copy that a paste follows: its ticket keeps every other copy away until spent.
    CopyForPaste {
        task: Task<Result<PasteTicket, ClipboardError>>,
        _wake: Wake,
    },
    Restore {
        backup: DeletedClips,
        task: Task<Result<(), ClipboardError>>,
        _wake: Wake,
    },
    Text {
        task: Task<Result<Option<String>, ClipboardError>>,
        _wake: Wake,
    },
    /// Open or show a file; `close` ends the opening once it worked.
    File {
        close: bool,
        task: Task<Result<(), ClipboardError>>,
        _wake: Wake,
    },
}

/// A paste that has left the panel. It outlives the opening that started it: the answer is
/// taken whether or not the panel is showing, and only the opening that asked may show it.
pub(crate) struct PasteJob {
    opening: u64,
    task: Task<Result<PasteOutcome, ClipboardError>>,
    _wake: Wake,
}

fn start<T: Send + 'static>(
    s: &PickerState,
    name: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<(Task<T>, Wake)> {
    let task = Task::spawn(name, work).ok()?;
    let handle = s.shell.handles().picker.clone();
    let wake = task.ready().subscribe(move || handle.wake());
    Some((task, wake))
}

// MARK: Keys

pub fn on_key(s: &mut PickerState, key: &KeyEvent, tx: &mut Tx) -> bool {
    // An input method owns every key while it composes, ⌘Q included.
    if s.model.picker.is_composing() {
        return false;
    }
    if crate::shell::closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    let took = dispatch(s, key, tx);
    if matches!(key.key, Key::Modifier { .. }) {
        return false;
    }
    took
}

/// The field sees keys before the editor: take the ones the picker owns, leave the rest to it.
pub fn on_input_key(s: &mut PickerState, key: &KeyEvent, tx: &mut Tx) -> bool {
    if s.model.picker.is_composing() {
        return false;
    }
    if matches!(key.key, Key::Modifier { .. }) {
        dispatch(s, key, tx);
        return false;
    }
    dispatch(s, key, tx)
}

/// True when the key was the picker's.
fn dispatch(s: &mut PickerState, key: &KeyEvent, tx: &mut Tx) -> bool {
    s.model.clock_secs = tx.clock();
    if let Key::Modifier { modifier, pressed } = key.key {
        let command = matches!(
            modifier,
            rune::Modifier::SuperLeft | rune::Modifier::SuperRight
        );
        if pressed && key.mods.cmd {
            s.model.picker.clips.look.show_numbers = true;
        } else if !pressed && command {
            s.model.picker.clips.look.show_numbers = false;
        }
        return false;
    }
    s.model.picker.held.press(hint_for(key));
    if key.mods.cmd {
        s.model.picker.clips.look.show_numbers = true;
    }
    let p = &s.model.picker;
    let ctx = Ctx {
        query_empty: p.query().is_empty(),
        can_undo: p.undo.is_some(),
    };
    let effect = keys::effect(key, ctx);
    let took = effect != Effect::None;
    apply(s, effect, tx);
    took
}

pub fn apply(s: &mut PickerState, effect: Effect, tx: &mut Tx) {
    s.model.clock_secs = tx.clock();
    match effect {
        Effect::None => {}
        Effect::Move(delta) => {
            if let Some(id) = s.model.picker.clips.list.move_by(delta, false) {
                if let Some(key) = s.model.picker.clips.list.scroll_key(id) {
                    tx.scroll_into_view(key.to_owned());
                }
                track_active(s);
            }
        }
        Effect::Paste { nth, flip } => {
            let id = match nth {
                Some(n) => s.model.picker.clips.list.order().get(n - 1).copied(),
                None => s.model.picker.clips.list.active(),
            };
            if let Some(id) = id {
                paste(s, id, flip);
            }
        }
        Effect::CopyOnly { flip } => {
            if let Some(id) = s.model.picker.clips.list.active() {
                copy(s, id, flip, After::Only);
            }
        }
        Effect::Pin => pin(s),
        Effect::Delete => delete(s),
        Effect::Undo => undo(s),
        Effect::Snippet => snippet(s),
        Effect::Open => open(s, tx),
        Effect::Settings => {
            s.go_main(Route::Settings(SettingsPane::Clipboard));
            s.dismiss();
        }
        Effect::Filter { backwards } => {
            let p = &mut s.model.picker;
            p.filter = crate::clip::filters::cycle(p.filter, backwards);
            pull(s);
        }
        Effect::ClearQuery => {
            s.model.picker.edit.replace_text("");
            pull(s);
        }
        Effect::Dismiss => {
            let restore = !s.shell.main_focused();
            s.dismiss_with(restore);
        }
    }
}

// MARK: Actions

fn flavor(s: &PickerState, id: Uuid, flip: bool) -> Option<Flavor> {
    let p = &s.model.picker;
    let entry = p.clips.list.entry(id)?;
    Some(if plain_for(entry, p.prefs.paste_plain, flip) {
        Flavor::Plain
    } else {
        Flavor::Original
    })
}

pub(super) fn paste(s: &mut PickerState, id: Uuid, flip: bool) {
    // One paste at a time: while a copy for a paste is on its way, another Return is the same
    // request again.
    if s.model
        .jobs
        .iter()
        .any(|job| matches!(job, Job::CopyForPaste { .. }))
    {
        return;
    }
    // The session lands a moment after the panel opens: a paste asked for before then waits.
    if s.model.session.is_none() && s.model.begin.is_some() {
        s.model.picker.pending_paste = Some((id, flip));
        return;
    }
    let then = if s.model.picker.can_paste && s.model.session.is_some() {
        After::Paste
    } else {
        After::Fallback
    };
    copy(s, id, flip, then);
}

fn copy(s: &mut PickerState, id: Uuid, flip: bool, then: After) {
    let Some(flavor) = flavor(s, id, flip) else {
        return;
    };
    let service = s.svc.clipboard.clone();
    let started = if then == After::Paste {
        start(s, "kudzu-picker-copy", move || {
            service.copy_for_paste(id, flavor)
        })
        .map(|(task, _wake)| Job::CopyForPaste { task, _wake })
    } else {
        start(s, "kudzu-picker-copy", move || service.copy(id, flavor))
            .map(|(task, _wake)| Job::Copy { then, task, _wake })
    };
    match started {
        Some(job) => s.model.jobs.push(job),
        None => say_error(s, &ClipboardError::WorkerStopped),
    }
}

fn say_error(s: &mut PickerState, error: &ClipboardError) {
    let clock = s.model.clock_secs;
    s.model
        .picker
        .say(error.to_string(), ERROR_FLASH_SECS, clock);
}

fn pin(s: &mut PickerState) {
    let clock = s.model.clock_secs;
    let Some((id, pinned)) = s.model.picker.active().map(|e| (e.id, e.pinned)) else {
        return;
    };
    match s.svc.clipboard.pin(id, !pinned) {
        Ok(()) => s.model.picker.say(
            if pinned {
                "Unpinned"
            } else {
                "Pinned. It won't expire."
            },
            FLASH_SECS,
            clock,
        ),
        Err(error) => say_error(s, &error),
    }
}

/// Deletes the active clip and moves to its neighbour (the next, else the previous).
fn delete(s: &mut PickerState) {
    let clock = s.model.clock_secs;
    let list = &s.model.picker.clips.list;
    let Some(id) = list.active() else { return };
    let order = list.order();
    let at = order.iter().position(|o| *o == id).unwrap_or(0);
    let neighbour = order
        .get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|p| order.get(p)));
    if let Some(next) = neighbour.copied() {
        s.model.picker.clips.list.activate(next);
    }
    match s.svc.clipboard.delete(&[id]) {
        Ok(deleted) => {
            s.model.picker.undo = Some(deleted);
            s.model
                .picker
                .say("Deleted. ⌘Z to undo", UNDO_FLASH_SECS, clock);
        }
        Err(error) => say_error(s, &error),
    }
}

fn undo(s: &mut PickerState) {
    let Some(backup) = s.model.picker.undo.take() else {
        return;
    };
    let service = s.svc.clipboard.clone();
    let again = backup.clone();
    match start(s, "kudzu-picker-restore-clips", move || {
        service.restore(backup)
    }) {
        Some((task, _wake)) => s.model.jobs.push(Job::Restore {
            backup: again,
            task,
            _wake,
        }),
        None => say_error(s, &ClipboardError::WorkerStopped),
    }
}

fn snippet(s: &mut PickerState) {
    let Some(id) = s
        .model
        .picker
        .active()
        .filter(|e| has_text(e))
        .map(|e| e.id)
    else {
        return;
    };
    let service = s.svc.clipboard.clone();
    if let Some((task, _wake)) = start(s, "kudzu-picker-text", move || service.text(id)) {
        s.model.jobs.push(Job::Text { task, _wake });
    }
}

/// ⌘O: a link opens in the browser, a file in its app; either closes the picker.
fn open(s: &mut PickerState, tx: &mut Tx) {
    let clock = s.model.clock_secs;
    let Some(entry) = s.model.picker.active().cloned() else {
        return;
    };
    match entry.kind {
        ClipKind::Link => {
            let target =
                crate::clip::content::link::open_target(entry.text.as_deref().unwrap_or(""));
            match tx.open_url(&target) {
                Ok(()) => s.dismiss(),
                Err(rune::OpenUrlError::UnsupportedScheme(scheme)) => s.model.picker.say(
                    format!("Kudzu can't open {scheme}: links."),
                    ERROR_FLASH_SECS,
                    clock,
                ),
                Err(_) => s
                    .model
                    .picker
                    .say("Couldn't open that link.", ERROR_FLASH_SECS, clock),
            }
        }
        ClipKind::File => file_job(s, 0, false, true),
        _ => {}
    }
}

/// Opens (or shows in Finder) the file at `index` of the active clip, through the file port,
/// on a worker. A failure goes to the footer; `close` ends the opening when it worked.
pub(super) fn file_job(s: &mut PickerState, index: usize, reveal: bool, close: bool) {
    let Some(path) = s
        .model
        .picker
        .active()
        .and_then(|e| e.files.get(index).cloned())
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
    match start(s, "kudzu-picker-file", work) {
        Some((task, _wake)) => s.model.jobs.push(Job::File { close, task, _wake }),
        None => say_error(s, &ClipboardError::WorkerStopped),
    }
}

/// A paste asked for before the session landed runs now.
pub(crate) fn session_landed(s: &mut PickerState) {
    if let Some((id, flip)) = s.model.picker.pending_paste.take() {
        paste(s, id, flip);
    }
}

// MARK: Poll

pub fn poll(s: &mut PickerState, _now: f64, tx: &mut Tx) -> bool {
    s.model.clock_secs = tx.clock();
    let clock = s.model.clock_secs;
    let mut changed = s.sync();
    changed |= refresh_prefs(s);
    // A paste's answer is taken whether or not the panel is showing.
    changed |= finish_pastes(s);
    if s.model.open {
        if std::mem::take(&mut s.model.focus_search) {
            tx.focus(FIELD_KEY, true);
            changed = true;
        }
        changed |= pull(s);
        changed |= finish_jobs(s);
        changed |= s.model.picker.note_focus(tx.focused(), tx.focus_visible());
        let p = &mut s.model.picker;
        changed |= p.full.poll();
        changed |= p.clips.media.as_mut().is_some_and(Media::poll);
        changed |= p.expire(clock);
        if let Some(hide) = s.model.picker.take_hide(clock) {
            s.dismiss_with(hide.restore);
            changed = true;
        }
        if let Some(due) = s.model.picker.next_deadline() {
            tx.wake_at(due);
        }
    }
    changed
}

/// The panel already hid for a paste that then failed or fell back to a copy: the sentence has
/// to be seen, so the panel comes back for `hide_after` seconds.
fn reshow(s: &mut PickerState, sentence: String, flash_secs: f64, hide_after: f64) {
    let clock = s.model.clock_secs;
    s.model.open = true;
    s.model.picker.say(sentence, flash_secs, clock);
    s.model.picker.hide_after(hide_after, false, clock);
    let picker = s.shell.handles().picker.clone();
    s.shell.handles().picker.update(move |st, _| {
        if st.model.open {
            picker.show();
        }
    });
}

/// Takes the answers of pastes that have left the panel. Only the opening that asked may show
/// one: a newer opening's footer and query are never touched by an older paste.
fn finish_pastes(s: &mut PickerState) -> bool {
    let jobs = std::mem::take(&mut s.model.pastes);
    let mut changed = false;
    for job in jobs {
        let Some(result) = job.task.take() else {
            s.model.pastes.push(job);
            continue;
        };
        let current = job.opening == s.model.opening;
        match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
            Ok(PasteOutcome::Pasted) => {}
            Ok(PasteOutcome::Copied) if current => {
                // Permission went away after the panel opened: the clip is on the pasteboard.
                reshow(
                    s,
                    FALLBACK_NOTICE.into(),
                    FALLBACK_FLASH_SECS,
                    FALLBACK_HIDE_AFTER,
                );
                changed = true;
            }
            Ok(PasteOutcome::Copied) => log::warn!("a paste fell back to a copy"),
            Err(error) if current => {
                log::warn!("the paste failed: {error}");
                reshow(s, error.to_string(), ERROR_FLASH_SECS, ERROR_FLASH_SECS);
                changed = true;
            }
            Err(error) => log::warn!("a paste for an earlier opening failed: {error}"),
        }
    }
    changed
}

fn finish_jobs(s: &mut PickerState) -> bool {
    let jobs = std::mem::take(&mut s.model.jobs);
    let mut changed = false;
    for job in jobs {
        match job {
            Job::Copy { then, task, _wake } => match task.take() {
                None => s.model.jobs.push(Job::Copy { then, task, _wake }),
                Some(result) => {
                    changed = true;
                    after_copy(
                        s,
                        then,
                        result.unwrap_or(Err(ClipboardError::WorkerStopped)),
                    );
                }
            },
            Job::CopyForPaste { task, _wake } => match task.take() {
                None => s.model.jobs.push(Job::CopyForPaste { task, _wake }),
                Some(result) => {
                    changed = true;
                    match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        Ok(ticket) => start_paste(s, ticket),
                        Err(error) => say_error(s, &error),
                    }
                }
            },
            Job::Restore {
                backup,
                task,
                _wake,
            } => match task.take() {
                None => s.model.jobs.push(Job::Restore {
                    backup,
                    task,
                    _wake,
                }),
                Some(result) => {
                    changed = true;
                    let clock = s.model.clock_secs;
                    match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        Ok(()) => s.model.picker.say("Restored", FLASH_SECS, clock),
                        Err(error) => {
                            s.model.picker.undo = Some(backup);
                            say_error(s, &error);
                        }
                    }
                }
            },
            Job::Text { task, _wake } => match task.take() {
                None => s.model.jobs.push(Job::Text { task, _wake }),
                Some(result) => {
                    changed = true;
                    match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        Ok(Some(content)) => {
                            s.go_main(Route::NewSnippet(NewSnippet {
                                content,
                                ..NewSnippet::default()
                            }));
                            s.dismiss();
                        }
                        Ok(None) => {}
                        Err(error) => say_error(s, &error),
                    }
                }
            },
            Job::File { close, task, _wake } => match task.take() {
                None => s.model.jobs.push(Job::File { close, task, _wake }),
                Some(result) => {
                    changed = true;
                    match result.unwrap_or(Err(ClipboardError::WorkerStopped)) {
                        Ok(()) if close => s.dismiss(),
                        Ok(()) => {}
                        Err(error) => say_error(s, &error),
                    }
                }
            },
        }
    }
    changed
}

fn after_copy(s: &mut PickerState, then: After, result: Result<(), ClipboardError>) {
    let clock = s.model.clock_secs;
    if let Err(error) = result {
        say_error(s, &error);
        return;
    }
    match then {
        After::Only => {
            s.model.picker.say("Copied", FLASH_SECS, clock);
            s.model.picker.hide_after(COPY_HIDE_AFTER, true, clock);
        }
        After::Fallback => {
            s.model
                .picker
                .say(FALLBACK_NOTICE, FALLBACK_FLASH_SECS, clock);
            s.model.picker.hide_after(FALLBACK_HIDE_AFTER, true, clock);
        }
        // The paste follows a copy that holds a ticket: `start_paste`.
        After::Paste => {}
    }
}

/// Hides the panel, then pastes on a worker. The session moves into the task, so the blur the
/// hide causes cannot drop it. Without one (the opening ended) the ticket is dropped and
/// the clip stays on the pasteboard.
fn start_paste(s: &mut PickerState, ticket: PasteTicket) {
    let Some(session) = s.model.session.take() else {
        return;
    };
    s.dismiss();
    let opening = s.model.opening;
    if let Some((task, _wake)) = start(s, "kudzu-picker-paste", move || {
        session.paste_copied(ticket)
    }) {
        s.model.pastes.push(PasteJob {
            opening,
            task,
            _wake,
        });
    }
}

pub use super::rows::{list_hooks, place, pull};
use super::rows::{refresh_prefs, track_active};
