//! Moves that reach rows the virtual list never built (S14's caveats for S50: #547, #549): End
//! and Home on 5,000 rows, ⇧End, and a route that opens a snippet far down the list. The
//! reveal is scoped to the list's scroll viewport (`Tx::scroll_into_view_in`).

use rune::Key;
use uuid::Uuid;

use super::library_list::{focus_list, key};
use super::Rig;
use crate::main::library::{self, scenes};
use crate::shell::Route;

fn rig_5000() -> Rig {
    let mut rig = Rig::new(true);
    rig.rt.update(|st, _| scenes::library_5000(st));
    rig.settle();
    rig
}

fn ids(rig: &Rig) -> Vec<Uuid> {
    rig.rt
        .state
        .library
        .listing(&rig.rt.state)
        .expect("the library is on screen")
        .ids
        .clone()
}

/// The row is built and wholly inside the list's viewport.
fn shown(rig: &Rig, id: Uuid) -> bool {
    let (Some(row), Some(port)) = (
        rig.rect(&library::row_key(id)),
        rig.rect(library::SCROLL_KEY),
    ) else {
        return false;
    };
    row[1] >= port[1] - 0.5 && row[1] + row[3] <= port[1] + port[3] + 0.5
}

#[test]
fn end_home_and_shift_end_reach_rows_never_built() {
    let mut rig = rig_5000();
    let list = ids(&rig);
    assert_eq!(list.len(), 5_000);
    let (first, last) = (list[0], list[4_999]);
    assert!(!shown(&rig, last), "the last row starts unbuilt");
    focus_list(&mut rig);

    rig.press(key(Key::End, false));
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(last));
    assert!(shown(&rig, last), "End reveals row 4,999");

    rig.press(key(Key::Home, false));
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(first));
    assert!(shown(&rig, first), "Home comes back to the first row");

    rig.press(key(Key::End, true));
    assert_eq!(rig.rt.state.library.selection.bulk().len(), 5_000);
    assert!(shown(&rig, last));
}

#[test]
fn a_route_to_an_unbuilt_snippet_reveals_it() {
    let mut rig = rig_5000();
    let target = ids(&rig)[2_500];
    assert!(!shown(&rig, target));
    rig.open_route(Route::Snippet(target));
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(target));
    assert!(shown(&rig, target), "the poll's reveal scrolls to it");
}
