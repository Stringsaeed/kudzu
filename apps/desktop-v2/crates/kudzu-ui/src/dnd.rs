//! dnd: in-app drag and drop (S42 owns this module). S50 added only the payload its rows carry;
//! S42's version wins when the two merge.

use uuid::Uuid;

/// What an in-app drag carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragPayload {
    Snippets(Vec<uuid::Uuid>),
    Group(uuid::Uuid),
}

pub fn drop_payload(
    s: &mut crate::shell::MainState,
    payload: DragPayload,
    target: Option<Uuid>,
) -> bool {
    let library = s.svc.library.watch().get();
    if target.is_some_and(|id| !library.groups.iter().any(|group| group.id == id)) {
        crate::main::sidebar::actions::show_error(
            s,
            kudzu_app::library::LibraryError::MissingGroup,
        );
        return false;
    }
    let result = match payload {
        DragPayload::Snippets(ids) => s.svc.library.move_snippets(&ids, target),
        DragPayload::Group(id) => s.svc.library.move_group(id, target, i64::MAX),
    };
    match result {
        Ok(_) => {
            s.refresh_library_snapshot();
            true
        }
        Err(error) => {
            crate::main::sidebar::actions::show_error(s, error);
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Root,
    Group(Uuid),
}

fn target(path: &str) -> Option<DropTarget> {
    if path.split('/').any(|part| part == "sidebar-groups-root") {
        return Some(DropTarget::Root);
    }
    let id = path.split('/').find_map(|part| {
        part.strip_prefix("sidebar-group-")
            .and_then(|id| Uuid::parse_str(id).ok())
    })?;
    Some(DropTarget::Group(id))
}

fn hover(s: &mut crate::shell::MainState, payload: &DragPayload, picked: Option<rune::Picked>) {
    if !s
        .sidebar
        .drag
        .as_ref()
        .is_some_and(|(active, _, _)| active == payload)
    {
        return;
    }
    let found = picked.as_ref().and_then(|p| target(p.path()));
    s.sidebar.drop_target = match (payload, found) {
        (DragPayload::Group(id), Some(DropTarget::Group(target)))
            if kudzu_core::library::groups::descendant_ids(
                &s.svc.library.watch().get().groups,
                *id,
            )
            .contains(&target) =>
        {
            None
        }
        (_, target) => target,
    };
}

fn drop_at(s: &mut crate::shell::MainState, payload: DragPayload, picked: Option<rune::Picked>) {
    match picked.as_ref().and_then(|p| target(p.path())) {
        Some(DropTarget::Root) => {
            drop_payload(s, payload, None);
        }
        Some(DropTarget::Group(id)) => {
            drop_payload(s, payload, Some(id));
        }
        None => {}
    }
}

pub fn update_hover(
    s: &mut crate::shell::MainState,
    payload: DragPayload,
    x: f32,
    y: f32,
    tx: &mut rune::Tx,
) {
    move_drag(s, payload.clone(), x, y);
    tx.element_at(x, y, move |s: &mut crate::shell::MainState, picked, _| {
        hover(s, &payload, picked)
    });
}

pub fn finish_drag(
    s: &mut crate::shell::MainState,
    payload: DragPayload,
    x: f32,
    y: f32,
    tx: &mut rune::Tx,
) {
    s.sidebar.drag = None;
    s.sidebar.drop_target = None;
    tx.element_at(x, y, move |s: &mut crate::shell::MainState, picked, _| {
        drop_at(s, payload, picked)
    });
}

pub fn move_drag(s: &mut crate::shell::MainState, payload: DragPayload, x: f32, y: f32) {
    s.sidebar.drag = Some((payload, x, y));
}

/// Connects the library row's active drag to the sidebar without changing its selection.
pub fn source_move(
    sidebar: &mut crate::main::sidebar::State,
    payload: Option<DragPayload>,
    drag: &rune::Drag,
    tx: &mut rune::Tx,
) {
    let Some(payload) = payload else {
        return;
    };
    let [x, y] = drag.window_point();
    sidebar.drag = Some((payload.clone(), x, y));
    tx.element_at(x, y, move |s: &mut crate::shell::MainState, picked, _| {
        hover(s, &payload, picked)
    });
}

/// Finishes the library row's drag before S50 clears its snapshot.
pub fn source_release(
    sidebar: &mut crate::main::sidebar::State,
    payload: Option<DragPayload>,
    drag: &rune::Drag,
    tx: &mut rune::Tx,
) {
    sidebar.drag = None;
    sidebar.drop_target = None;
    let Some(payload) = payload else {
        return;
    };
    let [x, y] = drag.window_point();
    let cancelled = drag.cancelled();
    tx.element_at(x, y, move |s: &mut crate::shell::MainState, picked, _| {
        if !cancelled {
            drop_at(s, payload, picked);
        }
    });
}
