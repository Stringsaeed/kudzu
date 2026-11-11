//! Turn a key into an effect, and advance search, preview, copy and outcomes.
//!
//! The main thread never renders, copies or inserts. Those are tasks. Timers use the
//! frame clock so a headless script and the window agree.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rune::{Key, KeyEvent, Mods, Tx, Waker};
use uuid::Uuid;

use super::keys::Effect;
use super::model::{log_start, utc_from, CopyJob, HeldList, RenderInflight};
use super::notice::{classify, NoticeKind};
use super::place::{display_local_origin, HEIGHT, WIDTH};
use super::preview::COPY_HIDE_AFTER;
use super::query::CreateAs;
use crate::shell::routes::{NewSnippet, Route};
use crate::shell::windows::PaletteState;
use crate::widgets::key_hints::hint_for;

pub fn setup(state: &mut PaletteState, waker: Waker) {
    state.model.search.search.set_waker(waker);
}

/// How long an insert keeps the palette from opening again.
/// `expansion::hold::OUTCOME_DEADLINE` is the same ten seconds and is crate-private.
const INSERT_HOLD: Duration = Duration::from_secs(10);

pub fn poll(state: &mut PaletteState, now: f64, tx: &mut Tx) -> bool {
    let mut changed = state.sync();
    changed |= commit_pending(state);
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
    state.model.search.held.press(hint_for(key));
    if hold_until_the_list_matches(state, key) {
        return;
    }
    let prefix = trigger_prefix(state);
    let rows = hit_ids(state);
    let create = state.model.search.presentation.create;
    let main_focused = state.shell.main_focused();
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
        Effect::QueryCleared => {
            state.model.search.held_list = None;
            state.model.search.edit.replace_text("");
        }
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
        let revision = revision.0;
        let scope = state.model.search.keys.scope;
        let now = utc_from(state.svc.clock.as_ref());
        let library = Arc::clone(&library);
        let cache = Arc::clone(&state.model.search.cache);
        state
            .model
            .search
            .search
            .request_if_changed(&(revision, text, scope), move |(_, query, scope)| {
                cache.answer(revision, &library, &query, scope, now)
            });
    }
    if let Some(next) = state.model.search.search.take() {
        if state.model.search.frozen.is_none() {
            state.model.search.presentation = next;
            changed = true;
            flush_held(state);
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
                state.model.search.insert_until = None;
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
            state.model.search.insert_until = None;
            state.model.search.baseline = notice.sequence;
            true
        }
        NoticeKind::Settled(_) => {
            state.model.search.pending = None;
            state.model.search.insert_until = None;
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
    // The capture task stays put. Dropping it here loses the session, and the pick with it.
    state.model.search.pending_pick = Some(id);
    commit_pending(state);
}

/// Run a queued pick once the session exists. Hiding can blur back into `dismiss`, which
/// must not clear `pending`: the guard has already moved into the insert task.
fn commit_pending(state: &mut PaletteState) -> bool {
    let Some(id) = state.model.search.pending_pick else {
        return false;
    };
    let Some(session) = state.model.session.take() else {
        return false;
    };
    state.model.search.pending_pick = None;
    let baseline = match *state.svc.expansion.outcomes().get() {
        Some(notice) => notice.sequence,
        None => 0,
    };
    state.model.search.baseline = baseline;
    state.model.search.pending = Some(id);
    state.model.search.insert_until = Some(state.svc.clock.now() + INSERT_HOLD);
    state.model.open = false;
    state.shell.handles().palette.hide();
    match session.insert(id) {
        Ok(task) => state.model.search.insert_task = Some(task),
        Err(error) => {
            log_start("quick search insert", error);
            state.model.search.pending = None;
            state.model.search.insert_until = None;
        }
    }
    true
}

/// Enter and ⌘1–9 name a row. They wait until `presentation` was built for this field.
fn hold_until_the_list_matches(state: &mut PaletteState, key: &KeyEvent) -> bool {
    let Some(action) = list_action(key) else {
        return false;
    };
    if list_matches(state) {
        state.model.search.held_list = None;
        return false;
    }
    if key.mods.cmd {
        state.model.search.keys.show_numbers = true;
    }
    state.model.search.held_list = Some(action);
    true
}

fn flush_held(state: &mut PaletteState) {
    if !list_matches(state) {
        return;
    }
    let Some(action) = state.model.search.held_list.take() else {
        return;
    };
    let key = action.event();
    let prefix = trigger_prefix(state);
    let rows = hit_ids(state);
    let create = state.model.search.presentation.create;
    let main_focused = state.shell.main_focused();
    let effect = state
        .model
        .search
        .keys
        .on_key(&key, &rows, create, &prefix, main_focused);
    apply(state, effect);
}

fn list_matches(state: &PaletteState) -> bool {
    let search = &state.model.search;
    search.presentation.for_query == search.edit.text
        && search.presentation.for_scope == search.keys.scope
}

fn list_action(key: &KeyEvent) -> Option<HeldList> {
    if matches!(key.key, Key::Enter) {
        return Some(HeldList::Enter { copy: key.mods.cmd });
    }
    if key.mods.cmd && !key.mods.ctrl {
        if let Key::Command(c) = key.key {
            if let Some(digit) = c.to_digit(10).filter(|d| (1..=9).contains(d)) {
                return Some(HeldList::Digit(digit as u8));
            }
        }
    }
    None
}

impl HeldList {
    fn event(self) -> KeyEvent {
        match self {
            HeldList::Enter { copy } => KeyEvent {
                key: Key::Enter,
                mods: Mods {
                    cmd: copy,
                    ..Mods::default()
                },
            },
            HeldList::Digit(n) => KeyEvent {
                key: Key::Command(char::from(b'0' + n)),
                mods: Mods {
                    cmd: true,
                    ..Mods::default()
                },
            },
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
