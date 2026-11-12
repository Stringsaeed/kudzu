//! The library's reference states (`docs/migration/reference/STATES.md`, `capture.mjs`), each
//! built through the list's own transitions on the S02 sample library (`lib/mock.ts`):
//!
//! - `library`: the window as it opens, on the first snippet in library order.
//! - `library-empty`: no snippets and no groups.
//! - `snippet-bulk`: click row 1, ⌘-click row 2.
//! - `snippet-deleted-toast`: click row 1, ⌘⌫.
//! - `library-5000`: 5,000 snippets, for the §1.7 scroll trace (no reference).

use kudzu_app::fixtures::SampleState;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use rune::Tx;
use uuid::Uuid;

use super::model::ClickMods;
use super::{actions, restart};
use crate::shell::MainState;

/// Replaces the library with `library` and opens the list as a fresh window does.
fn seed(s: &mut MainState, library: KudzuLibrary) {
    if let Err(error) = s.svc.library.transact(move |lib| {
        *lib = library;
        Ok(())
    }) {
        log::warn!("the scene's library could not be loaded: {error}");
    }
    restart(s);
}

/// The S02 sample library.
pub fn seed_sample(s: &mut MainState) {
    match SampleState::load() {
        Ok(sample) => seed(s, sample.library),
        Err(error) => log::warn!("the sample library could not be read: {error}"),
    }
}

pub fn library(s: &mut MainState) {
    seed_sample(s);
}

pub fn library_empty(s: &mut MainState) {
    seed(s, KudzuLibrary::default());
}

/// A press on the row at `index` of the list, with `mods`.
fn click(s: &mut MainState, index: usize, mods: ClickMods) {
    let Some(listing) = s.library.listing(s) else {
        return;
    };
    let Some(&id) = listing.ids.get(index) else {
        return;
    };
    let cursor = s.frame.selected_snippet;
    let next = s.library.selection.click(&listing.ids, cursor, id, mods);
    s.select_snippet(Some(next));
    super::sync(s);
}

pub fn snippet_bulk(s: &mut MainState) {
    seed_sample(s);
    click(s, 0, ClickMods::default());
    click(
        s,
        1,
        ClickMods {
            toggle: true,
            range: false,
        },
    );
}

pub fn snippet_deleted_toast(s: &mut MainState) {
    seed_sample(s);
    click(s, 0, ClickMods::default());
    let targets = s.library.targets(s.frame.selected_snippet);
    let mut tx = Tx::default();
    actions::delete_with_undo(s, &targets, &mut tx);
}

/// 5,000 snippets with the sample's shapes: two lines of text, a trigger, some usage.
pub fn library_5000(s: &mut MainState) {
    let Ok(sample) = SampleState::load() else {
        return;
    };
    let mut library = sample.library;
    let shapes: Vec<SnippetRecord> = library.snippets.clone();
    library.snippets = (0..5_000u128)
        .map(|n| {
            let shape = &shapes[n as usize % shapes.len()];
            SnippetRecord {
                id: Uuid::from_u128(0x5000_0000 + n),
                name: format!("{} {n:04}", shape.name),
                triggers: shape.triggers.iter().map(|t| format!("{t}{n}")).collect(),
                ..shape.clone()
            }
        })
        .collect();
    library.usage.clear();
    seed(s, library);
}
