//! Turn a key into an effect, and advance search, preview, copy and outcomes.
//!
//! The main thread never renders, copies or inserts. Those are tasks. Timers use the
//! frame clock so a headless script and the window agree.

use std::collections::HashMap;

use chrono::Utc;
use kudzu_app::expansion::InsertionOutcome;
use rune::{Key, KeyEvent, Tx, Waker};
use uuid::Uuid;

use super::keys::Effect;
use super::model::{log_start, CopyJob, RenderInflight};
use super::notice::{classify, NoticeKind};
use super::place::{display_local_origin, HEIGHT, WIDTH};
use super::preview::COPY_HIDE_AFTER;
use super::query::CreateAs;
use super::rank::present;
use crate::shell::routes::{NewSnippet, Route};
use crate::shell::windows::PaletteState;
use crate::widgets::key_hints::hint_for;

pub fn setup(state: &mut PaletteState, waker: Waker) {
    state.model.search.search.set_waker(waker);
}

pub fn poll(state: &mut PaletteState, now: f64, tx: &mut Tx) -> bool {
    let mut changed = state.sync();
    changed |= pull_search(state);
    changed |= advance_preview(state, now, tx);
    changed |= advance_copy(state, now, tx);
    changed |= advance_insert(state);
    changed |= advance_outcomes(state);
    if let Some(due) = state.model.search.preview.next_wake(now) {
        tx.wake_at(due);
    }
    changed
}

pub fn on_key(state: &mut PaletteState, key: &KeyEvent, tx: &mut Tx) -> bool {
    if crate::shell::closes_windows(key) {
        state.shell.close_windows();
        return true;
    }
    dispatch(state, key, tx);
    // Modifiers also have to reach the window, so a release still clears the footer.
    if matches!(key.key, Key::Modifier { .. }) {
        return false;
    }
    palette_key(key)
}

/// The field sees keys before the editor. Consume the ones the palette owns.
pub fn on_input_key(state: &mut PaletteState, key: &KeyEvent, tx: &mut Tx) -> bool {
    if matches!(key.key, Key::Modifier { .. }) {
        dispatch(state, key, tx);
        return false;
    }
    if !palette_key(key) {
        return false;
    }
    dispatch(state, key, tx);
    true
}

pub fn place(state: &PaletteState) {
    if !state.shell.native() {
        return;
    }
    let Some(marker) = objc2_foundation::MainThreadMarker::new() else {
        return;
    };
    let Some(screen) = kudzu_appkit::screens::under_pointer(marker) else {
        return;
    };
    let (x, y) = display_local_origin(screen.frame.width as f32, screen.frame.height as f32);
    state.shell.handles().palette.place(
        rune::DisplayTarget::UnderPointer,
        rune::OverlayPlacement::Rect {
            x,
            y,
            width: WIDTH,
            height: HEIGHT,
        },
    );
}

fn dispatch(state: &mut PaletteState, key: &KeyEvent, tx: &mut Tx) {
    let prefix = trigger_prefix(state);
    let rows = hit_ids(state);
    let create = state.model.search.presentation.create;
    let main_focused = state.shell.main_focused();
    state.model.search.held.press(hint_for(key));
    let effect = state
        .model
        .search
        .keys
        .on_key(key, &rows, create, &prefix, main_focused);
    if matches!(
        key.key,
        Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Tab
    ) || (key.mods.ctrl && matches!(typed(key), Some('n' | 'p')))
    {
        let index = clamped(state);
        tx.scroll_into_view(format!("pal-row-{index}"));
    }
    apply(state, effect);
}

pub fn apply(state: &mut PaletteState, effect: Effect) {
    match effect {
        Effect::None => {}
        Effect::QueryCleared => state.model.search.edit.replace_text(""),
        Effect::Insert(id) => insert(state, id),
        Effect::Copy(id) => start_copy(state, id),
        Effect::Edit(id) => {
            state.dismiss(false);
            state.shell.open_main(Route::Snippet(id));
        }
        Effect::Create(seed) => {
            state.dismiss(false);
            let mut draft = NewSnippet::default();
            match seed.as_ {
                CreateAs::Trigger => draft.trigger = seed.value,
                CreateAs::Name => draft.name = seed.value,
            }
            state.shell.open_main(Route::NewSnippet(draft));
        }
        Effect::Dismiss { restore } => state.dismiss(restore),
    }
}

pub fn pick(state: &mut PaletteState, index: usize, tx: &mut Tx) {
    let prefix = trigger_prefix(state);
    let rows = hit_ids(state);
    let effect = state.model.search.keys.pick(index, &rows, &prefix);
    tx.scroll_into_view(format!("pal-row-{index}"));
    apply(state, effect);
}

fn pull_search(state: &mut PaletteState) -> bool {
    let mut changed = false;
    let text = state.model.search.edit.text.clone();
    if state.model.search.keys.query != text {
        state.model.search.keys.set_query(text.clone());
        changed = true;
    }
    if state.model.search.frozen.is_none() {
        let (revision, library) = state.svc.library.watch().snapshot();
        let scope = state.model.search.keys.scope;
        let now = Utc::now();
        let library = library.clone();
        state
            .model
            .search
            .search
            .request_if_changed(&(revision.0, text, scope), move |(_, query, scope)| {
                present(&library, &query, scope, now)
            });
    }
    if let Some(next) = state.model.search.search.take() {
        if state.model.search.frozen.is_none() {
            state.model.search.presentation = next;
            changed = true;
        }
    }
    changed
}

fn advance_preview(state: &mut PaletteState, now: f64, _tx: &mut Tx) -> bool {
    let search = &mut state.model.search;
    if search.restart_preview {
        search.restart_preview = false;
        search.preview.kick(now);
    }
    let active = active_snippet(state);
    state.model.search.preview.set_active(active, now);
    let tick = state.model.search.preview.tick(now);
    let mut changed = false;
    if tick.dim != state.model.search.dim || tick.shimmer != state.model.search.shimmer {
        state.model.search.dim = tick.dim;
        state.model.search.shimmer = tick.shimmer;
        changed = true;
    }
    if tick.start {
        if let Some(id) = tick.want {
            state.model.search.preview.started();
            begin_preview(state, id);
        }
    }
    let landed = state
        .model
        .search
        .preview_task
        .as_ref()
        .and_then(|job| job.task.take().map(|result| (job.id, result)));
    if let Some((id, result)) = landed {
        state.model.search.preview_task = None;
        match result {
            Ok(Ok(rendered)) if state.model.search.preview.want() == Some(id) => {
                state.model.search.preview.landed(id);
                state.model.search.rendered = Some((id, rendered));
                changed = true;
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => log::warn!("quick search preview failed"),
        }
    }
    changed
}

fn advance_copy(state: &mut PaletteState, now: f64, tx: &mut Tx) -> bool {
    let finished_render = match &state.model.search.copy {
        Some(CopyJob::Render(job)) => job.task.take().map(|result| (job.id, result)),
        _ => None,
    };
    if let Some((id, result)) = finished_render {
        state.model.search.copy = None;
        match result {
            Ok(Ok(rendered)) => match state.svc.expansion.copy_rendered(rendered) {
                Ok(task) => state.model.search.copy = Some(CopyJob::Write(task)),
                Err(error) => {
                    log_start("quick search copy", error);
                    state.model.search.keys.flash = None;
                }
            },
            Ok(Err(_)) | Err(_) => {
                log::warn!("quick search copy failed");
                state.model.search.keys.flash = None;
                let _ = id;
            }
        }
    }
    let finished_write = match &state.model.search.copy {
        Some(CopyJob::Write(task)) => task.take(),
        _ => None,
    };
    if let Some(result) = finished_write {
        state.model.search.copy = None;
        match result {
            Ok(Ok(())) => {
                let due = now + COPY_HIDE_AFTER;
                state.model.search.copy_due = Some(due);
                tx.wake_at(due);
            }
            Ok(Err(_)) | Err(_) => {
                log::warn!("quick search copy failed");
                state.model.search.keys.flash = None;
            }
        }
    }
    if let Some(due) = state.model.search.copy_due {
        if now >= due {
            state.model.search.copy_due = None;
            state.dismiss(true);
            return true;
        }
        tx.wake_at(due);
    }
    false
}

fn advance_insert(state: &mut PaletteState) -> bool {
    let Some(task) = state.model.search.insert_task.as_ref() else {
        return false;
    };
    match task.take() {
        None => false,
        Some(result) => {
            state.model.search.insert_task = None;
            if !matches!(result, Ok(Ok(()))) {
                log::warn!("quick search insert was not admitted");
                state.model.search.pending = None;
            }
            true
        }
    }
}

fn advance_outcomes(state: &mut PaletteState) -> bool {
    let Some(notice) = *state.svc.expansion.outcomes().get() else {
        return false;
    };
    let kind = classify(
        &notice,
        state.model.search.baseline,
        state.model.search.pending,
    );
    match kind {
        NoticeKind::Ignore => false,
        NoticeKind::EngineStopped => {
            log::warn!("quick search stopped because the engine stopped");
            state.model.search.pending = None;
            state.model.search.baseline = notice.sequence;
            true
        }
        NoticeKind::Settled(outcome) => {
            if outcome != InsertionOutcome::Expanded {
                super::model::log_outcome(outcome);
            }
            state.model.search.pending = None;
            state.model.search.baseline = notice.sequence;
            true
        }
    }
}

fn begin_preview(state: &mut PaletteState, id: Uuid) {
    let selection = state
        .model
        .session
        .as_ref()
        .and_then(|s| s.selection().map(str::to_owned));
    match state
        .svc
        .expansion
        .preview_snippet(id, selection, HashMap::new())
    {
        Ok(task) => {
            state.model.search.preview_task = Some(RenderInflight { id, task });
        }
        Err(error) => log_start("quick search preview", error),
    }
}

fn start_copy(state: &mut PaletteState, id: Uuid) {
    let selection = state
        .model
        .session
        .as_ref()
        .and_then(|s| s.selection().map(str::to_owned));
    match state
        .svc
        .expansion
        .preview_snippet(id, selection, HashMap::new())
    {
        Ok(task) => state.model.search.copy = Some(CopyJob::Render(RenderInflight { id, task })),
        Err(error) => {
            log_start("quick search copy", error);
            state.model.search.keys.flash = None;
        }
    }
}

fn insert(state: &mut PaletteState, id: Uuid) {
    let baseline = match *state.svc.expansion.outcomes().get() {
        Some(notice) => notice.sequence,
        None => 0,
    };
    state.model.search.baseline = baseline;
    state.model.search.pending = Some(id);
    state.model.capture = None;
    // Take the session before hide. A blur from hiding calls `dismiss`, which would
    // otherwise drop the guard before `insert` moves it into the task.
    let Some(session) = state.model.session.take() else {
        log::warn!("quick search insert had no captured target");
        state.model.search.pending = None;
        return;
    };
    state.model.open = false;
    state.shell.handles().palette.hide();
    match session.insert(id) {
        Ok(task) => state.model.search.insert_task = Some(task),
        Err(error) => {
            log_start("quick search insert", error);
            state.model.search.pending = None;
        }
    }
}

fn trigger_prefix(state: &PaletteState) -> String {
    state
        .svc
        .settings
        .watch()
        .get()
        .default_trigger_prefix
        .clone()
}

fn hit_ids(state: &PaletteState) -> Vec<Uuid> {
    state
        .model
        .search
        .presentation
        .rows
        .iter()
        .map(|row| row.id)
        .collect()
}

pub fn clamped(state: &PaletteState) -> usize {
    let rows = state.model.search.presentation.rows.len();
    let total = rows + usize::from(state.model.search.presentation.create);
    if total == 0 {
        0
    } else {
        state.model.search.keys.active.min(total - 1)
    }
}

fn active_snippet(state: &PaletteState) -> Option<Uuid> {
    let index = clamped(state);
    state
        .model
        .search
        .presentation
        .rows
        .get(index)
        .map(|row| row.id)
}

fn palette_key(key: &KeyEvent) -> bool {
    if key.mods.ctrl && matches!(typed(key), Some('n' | 'p')) {
        return true;
    }
    match &key.key {
        Key::Escape | Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Tab | Key::Enter => {
            true
        }
        Key::Command(c) if key.mods.cmd && !key.mods.ctrl && matches!(c, 'e' | 'n' | '1'..='9') => {
            true
        }
        _ => false,
    }
}

fn typed(key: &KeyEvent) -> Option<char> {
    match &key.key {
        Key::Command(c) => Some(*c),
        Key::Text(text) => {
            let mut chars = text.chars();
            let c = chars.next()?.to_ascii_lowercase();
            chars.next().is_none().then_some(c)
        }
        _ => None,
    }
}
