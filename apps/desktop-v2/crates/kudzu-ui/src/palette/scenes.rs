//! Headless states for the palette. Each one freezes the sample library so `poll`
//! does not replace the list, and paints the active row's preview at the fixture clock.
use std::sync::{Arc, OnceLock};

use chrono::{TimeZone, Utc};
use kudzu_core::template::{self, RenderContext};
use kudzu_core::KudzuLibrary;

use crate::shell::windows::PaletteState;

use super::rank::{present, Scope};

type Scene = (&'static str, fn(&mut PaletteState));

pub const SCENES: &[Scene] = &[("search", search), ("no-match", no_match)];

fn search(state: &mut PaletteState) {
    show(state, "");
}

fn no_match(state: &mut PaletteState) {
    show(state, "no-such-reference-snippet");
}

/// The capture clock. Headless scenes measure "Used" and `{{date}}` against it.
pub(super) fn fixture_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap()
}

fn show(state: &mut PaletteState, query: &str) {
    let library = sample_library();
    let now = fixture_now();
    state.model.open = true;
    state.model.search.edit.replace_text(query);
    state.model.search.keys.set_query(query.to_owned());
    state.model.search.presentation = present(&library, query, Scope::All, now);
    state.model.search.frozen = Some(Arc::clone(&library));
    if let Some(rendered) = preview_of(
        &library,
        state
            .model
            .search
            .presentation
            .rows
            .first()
            .map(|row| row.id),
        now,
    ) {
        let id = rendered.0;
        state.model.search.preview.set_active(Some(id), 0.0);
        state.model.search.preview.landed(id);
        state.model.search.rendered = Some(rendered);
    }
}

fn preview_of(
    library: &KudzuLibrary,
    id: Option<uuid::Uuid>,
    now: chrono::DateTime<Utc>,
) -> Option<(uuid::Uuid, template::RenderResult)> {
    let id = id?;
    let snippet = library.snippets.iter().find(|snippet| snippet.id == id)?;
    let source = snippet.content.clone();
    let formatting = snippet.formatting.clone();
    let resolve = |sid: uuid::Uuid| {
        library
            .snippets
            .iter()
            .find(|snippet| snippet.id == sid)
            .map(|snippet| snippet.content.clone())
    };
    let ctx = RenderContext {
        now: now.fixed_offset(),
        locale: "en_US".into(),
        clipboard: None,
        form_values: std::collections::HashMap::new(),
        resolve_snippet: &resolve,
        resolve_formatting: None,
        root_snippet_id: Some(id),
        selection: None,
        clipboard_history: None,
        new_uuid: None,
        pick: None,
    };
    Some((
        id,
        template::render_formatted(&source, formatting.as_ref(), &ctx),
    ))
}

fn sample_library() -> Arc<KudzuLibrary> {
    static LIBRARY: OnceLock<Arc<KudzuLibrary>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| {
            // The release headless binary paints `--scene`, so the fixture is part of the
            // crate. A feature flag would strip it from that binary.
            let raw = include_str!("fixtures/sample-library.json");
            let library: KudzuLibrary = serde_json::from_str(raw).expect("sample library");
            Arc::new(library)
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_library_lists_rows_and_a_miss_offers_create() {
        let library = sample_library();
        let now = Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap();
        let listed = present(&library, "", Scope::All, now);
        assert!(!listed.rows.is_empty(), "the sample library has snippets");
        assert!(listed.empty.is_none());
        let preview = preview_of(&library, listed.rows.first().map(|row| row.id), now);
        assert_eq!(
            preview.as_ref().map(|(_, result)| result.text.as_str()),
            Some("Best regards,\nSaeed\nNovember 11, 2026"),
            "the open preview is the signature at the fixture clock"
        );
        let missed = present(&library, "no-such-reference-snippet", Scope::All, now);
        assert!(missed.rows.is_empty());
        assert!(missed.create);
    }
}
