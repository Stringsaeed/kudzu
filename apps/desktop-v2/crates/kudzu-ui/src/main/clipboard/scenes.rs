//! Headless scenes for the clipboard screen, named as in `reference/STATES.md`. The sample
//! history is the Tauri preview's (`mock-state.json`); `clips-5000` adds the preview's 5,000
//! generated plain copies (`?clips=5000`), and `clipboard-empty` is `?emptyclips`.
//! The bytes on disk are the preview's 38,400,000, since fixtures have no blobs to measure.

use chrono::Duration;
use kudzu_app::fixtures::SampleState;
use kudzu_core::clipboard::query::ClipFilter;
use kudzu_core::clipboard::{ClipEntry, ClipKind, ClipSource, History};
use uuid::Uuid;

use super::sync;
use crate::main::state::MainView;
use crate::shell::MainState;

const PREVIEW_DISK_USAGE: u64 = 38_400_000;
const WORDS: [&str; 16] = [
    "alpha", "beta", "gamma", "delta", "invoice", "meeting", "design", "review", "contract",
    "notes", "kudzu", "snippet", "paste", "copy", "deploy", "release",
];

fn open(s: &mut MainState, history: History) {
    if let Err(error) = s.svc.clipboard.replace_history(history) {
        log::warn!("the scene history was not applied: {error}");
    }
    s.clipboard.usage = Some(PREVIEW_DISK_USAGE);
    s.clipboard.usage_fixed = true;
    s.navigate_to(MainView::Clipboard(ClipFilter::All));
    sync(s);
}

/// The Tauri preview's sample history (`mock-state.json`), shared with the picker's scenes.
pub(crate) fn sample() -> History {
    SampleState::load()
        .map(|sample| sample.clipboard)
        .unwrap_or_default()
}

pub fn history(s: &mut MainState) {
    open(s, sample());
}

pub fn empty(s: &mut MainState) {
    open(s, History::new());
}

pub fn no_match(s: &mut MainState) {
    open(s, sample());
    s.clipboard.search.replace_text("no-such-reference-item");
    sync(s);
}

pub fn options_menu(s: &mut MainState) {
    open(s, sample());
    s.clipboard.options_open = true;
}

/// The first row clicked, the second ⌘-clicked.
pub fn bulk(s: &mut MainState) {
    use crate::clip::model::Click;
    open(s, sample());
    let ids = s.clipboard.clips.list.order().to_vec();
    if let [first, second, ..] = ids[..] {
        s.clipboard.clips.list.click(first, Click::default());
        s.clipboard.clips.list.click(
            second,
            Click {
                command: true,
                shift: false,
            },
        );
    }
}

pub fn clips_5000(s: &mut MainState) {
    open(s, history_5000());
}

/// The sample plus the preview's 5,000 generated copies (`?clips=5000`), newest first.
pub(crate) fn history_5000() -> History {
    let mut history = sample();
    let now = history
        .entries
        .first()
        .map_or_else(chrono::Utc::now, |e| e.copied_at);
    let now = now.max(chrono::DateTime::UNIX_EPOCH) + Duration::minutes(1);
    history.entries.extend(generated(5000, now));
    history
        .entries
        .sort_by_key(|e| std::cmp::Reverse(e.copied_at));
    history
}

/// `mock.ts`: text built from the word list, every ninth a code clip, spread over 160 hours.
fn generated(count: usize, now: chrono::DateTime<chrono::Utc>) -> Vec<ClipEntry> {
    let sources = [
        ("com.tinyspeck.slackmacgap", "Slack"),
        ("com.apple.Notes", "Notes"),
        ("com.apple.mail", "Mail"),
        ("com.microsoft.VSCode", "Visual Studio Code"),
    ];
    (0..count)
        .map(|i| {
            let words = 6 + i % 40;
            let mut text = (0..words)
                .map(|j| WORDS[(i * 7 + j * 3) % WORDS.len()])
                .collect::<Vec<_>>()
                .join(" ");
            text.push_str(&format!(" #{i}"));
            let hours = 0.5 + (i as f64 / count as f64) * 160.0;
            let at = now - Duration::seconds((hours * 3600.0) as i64);
            let (bundle, name) = sources[i % sources.len()];
            ClipEntry {
                id: Uuid::from_u128(0x5000_0000 + i as u128),
                kind: if i % 9 == 0 {
                    ClipKind::Code
                } else {
                    ClipKind::Text
                },
                text_len: text.chars().count(),
                line_count: 1,
                files: vec![],
                image: None,
                color: None,
                rich: false,
                types: vec!["public.utf8-plain-text".into()],
                size: text.len() as u64,
                source: Some(ClipSource {
                    bundle_id: bundle.into(),
                    name: name.into(),
                    path: None,
                }),
                hash: format!("gen-{i}"),
                first_copied_at: at,
                copied_at: at,
                copy_count: 1,
                timings: vec![at],
                last_used_at: None,
                use_count: 0,
                pinned: false,
                secret: None,
                secret_override: None,
                text: Some(text),
            }
        })
        .collect()
}

/// A kind folder opened on the sample history (the reference clicks the sidebar's folder).
fn folder(s: &mut MainState, filter: ClipFilter) {
    open(s, sample());
    s.navigate_to(MainView::Clipboard(filter));
    sync(s);
}

pub fn code(s: &mut MainState) {
    folder(s, ClipFilter::Code);
}

pub fn color(s: &mut MainState) {
    folder(s, ClipFilter::Color);
}

pub fn file(s: &mut MainState) {
    folder(s, ClipFilter::File);
}

pub fn image(s: &mut MainState) {
    folder(s, ClipFilter::Image);
}

pub fn link(s: &mut MainState) {
    folder(s, ClipFilter::Link);
}
