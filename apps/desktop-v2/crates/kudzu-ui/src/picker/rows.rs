//! The picker's rows: pulling the history into the list, the active clip's preview, and the
//! thumbnails and app icons the visible rows want.

use std::sync::Arc;

use kudzu_core::clipboard::query::ClipQuery;
use kudzu_core::clipboard::sections::RowOptions;
use rune::Tx;
use uuid::Uuid;

use crate::clip::content::loader::{open_wants, row_want, Media};
use crate::clip::list::Hooks;
use crate::clip::model::RowKind;
use crate::shell::windows::PickerState;

use super::drive::paste;

/// Reads the settings the picker shows. True when one of them changed.
pub(super) fn refresh_prefs(s: &mut PickerState) -> bool {
    let settings = s.svc.settings.watch().get();
    let prefs = super::model::Prefs {
        recording: settings.clipboard_enabled,
        show_secrets: settings.clipboard_show_secrets,
        paste_plain: settings.clipboard_paste_plain,
        retention_days: settings.clipboard_retention_days,
    };
    let p = &mut s.model.picker;
    p.clips.look.show_secrets = prefs.show_secrets;
    std::mem::replace(&mut p.prefs, prefs) != prefs
}

/// Rebuilds the rows when the history, the filter, the query or the day changed, and keeps
/// the preview pointed at the active clip. True when something visible changed.
pub fn pull(s: &mut PickerState) -> bool {
    let mut changed = refresh_prefs(s);
    let service = s.svc.clipboard.clone();
    let revision = service.watch().revision();
    let local = s.now();
    let day = i64::from(chrono::Datelike::num_days_from_ce(&local.date_naive()));
    let p = &mut s.model.picker;
    let text = p.edit.text.clone();
    let stale = p.seen.history != Some(revision)
        || p.seen.filter != Some(p.filter)
        || p.seen.query != text
        || p.seen.day != day;
    if stale {
        let query = ClipQuery::parse(&text);
        let results = service.query(p.filter, &query);
        let moved = p.seen.filter != Some(p.filter) || p.seen.query != text;
        p.total = results.history.entries.len();
        p.clips.look.terms = query.terms.clone();
        p.clips.look.now = local;
        p.clips.list.set(
            results.history.clone(),
            &results.indices,
            RowOptions::picker(&text),
            local,
        );
        if moved {
            // A new query or filter starts at the top, as `setActiveId(null)` does.
            if let Some(first) = p.clips.list.order().first().copied() {
                p.clips.list.activate(first);
            }
        }
        p.seen.history = Some(revision);
        p.seen.filter = Some(p.filter);
        p.seen.query = text;
        p.seen.day = day;
        want_app_icons(s);
        changed = true;
    }
    changed | track_active(s)
}

pub(super) fn media_mut(s: &mut PickerState) -> &mut Media {
    if s.model.picker.clips.media.is_none() {
        let handle = s.shell.handles().picker.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || handle.wake());
        s.model.picker.clips.media = Some(Media::new(wake));
    }
    s.model
        .picker
        .clips
        .media
        .get_or_insert_with(|| unreachable!("created above"))
}

pub(super) fn want_app_icons(s: &mut PickerState) {
    let mut bundles: Vec<String> = Vec::new();
    for e in &s.model.picker.clips.list.history().entries {
        if let Some(src) = &e.source {
            if !bundles.contains(&src.bundle_id) {
                bundles.push(src.bundle_id.clone());
            }
        }
        if bundles.len() >= 48 {
            break;
        }
    }
    let apps = s.svc.apps.clone();
    let media = media_mut(s);
    for bundle in bundles {
        media.want_app(&apps, &bundle);
    }
}

/// Loads the full text and pictures of the active clip. Cheap when nothing changed.
pub(crate) fn track_active(s: &mut PickerState) -> bool {
    let Some(entry) = s.model.picker.active().cloned() else {
        let had = s.model.picker.seen.active.take().is_some();
        s.model.picker.full = Default::default();
        return had;
    };
    let key = (entry.id, entry.hash.clone());
    let clipboard = s.svc.clipboard.clone();
    let wants = open_wants(&entry);
    let media = media_mut(s);
    media.need_open(wants.iter().copied());
    for (id, want) in &wants {
        media.want(&clipboard, *id, *want);
    }
    if s.model.picker.seen.active.as_ref() == Some(&key) {
        return false;
    }
    s.model.picker.seen.active = Some(key);
    s.model.picker.cover_hover = false;
    let handle = s.shell.handles().picker.clone();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || handle.wake());
    s.model.picker.full.show(&clipboard, &wake, &entry);
    if let Some(src) = &entry.source {
        let apps = s.svc.apps.clone();
        media_mut(s).want_app(&apps, &src.bundle_id);
    }
    true
}

/// The rows in view start their thumbnails and file icons.
pub(super) fn want_rows(s: &mut PickerState, range: std::ops::Range<usize>) {
    let list = &s.model.picker.clips.list;
    let wanted: Vec<(Uuid, crate::clip::content::loader::Want)> = list
        .rows()
        .get(range)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Entry { id, index, .. } => list
                .history()
                .entries
                .get(*index)
                .and_then(row_want)
                .map(|w| (*id, w)),
            RowKind::Header { .. } => None,
        })
        .collect();
    let clipboard = s.svc.clipboard.clone();
    let media = media_mut(s);
    media.need_rows(wanted.iter().copied());
    for (id, want) in wanted {
        media.want(&clipboard, id, want);
    }
}

/// How the shared list reaches the picker. A press only makes the row active: the search
/// field keeps keyboard focus.
pub fn list_hooks() -> Hooks<PickerState> {
    Hooks {
        state: |s: &PickerState| &s.model.picker.clips,
        visible: want_rows,
        reveal: |s: &mut PickerState, id: Uuid, on: bool| {
            let clips = &mut s.model.picker.clips;
            if on {
                clips.revealed = Some(id);
            } else if clips.revealed == Some(id) {
                clips.revealed = None;
            }
        },
        press: |s: &mut PickerState, id: Uuid, _click, _tx: &mut Tx| {
            s.model.picker.clips.list.activate(id);
            track_active(s);
        },
        pick: |s: &mut PickerState, id: Uuid, tx: &mut Tx| {
            s.model.clock_secs = tx.clock();
            s.model.picker.clips.list.activate(id);
            track_active(s);
            paste(s, id, false);
        },
    }
}

pub fn place(state: &PickerState) {
    use super::place::{display_local_origin, HEIGHT, WIDTH};
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
    state.shell.handles().picker.place(
        rune::DisplayTarget::UnderPointer,
        rune::OverlayPlacement::Rect {
            x,
            y,
            width: WIDTH,
            height: HEIGHT,
        },
    );
}
