//! Rankings and tile copy for Insights. Domain math stays in `kudzu_core::library::usage`.

use std::cmp::Reverse;
use std::time::{SystemTime, UNIX_EPOCH};

use kudzu_core::fmt::{format_count, format_duration, relative_time};
use kudzu_core::library::usage::{self, CHARACTERS_PER_WORD};
use kudzu_core::{KudzuLibrary, SnippetRecord};
use uuid::Uuid;

use crate::shell::MainState;

pub const TOP_N: usize = 10;
pub const RECENT_N: usize = 6;
pub const UNUSED_N: usize = 18;
/// Fixture clock is UTC; calendar fallback past 30 days uses this offset.
pub const OFFSET_MINUTES: i32 = 0;

pub struct Tile {
    pub label: &'static str,
    pub value: String,
    pub sub: String,
}

#[derive(Clone)]
pub struct Ranked {
    pub id: Uuid,
    pub name: String,
    pub trigger: Option<String>,
    pub count: i64,
    pub last_ms: i64,
    pub relative: String,
}

pub struct Unused {
    pub id: Uuid,
    pub label: String,
}

pub struct Data {
    pub tiles: [Tile; 4],
    pub top: Vec<Ranked>,
    pub recent: Vec<Ranked>,
    pub unused: Vec<Unused>,
    pub max: i64,
    pub wpm: u32,
}

pub fn from_state(s: &MainState) -> Data {
    let wpm = s.svc.settings.watch().get().typing_speed_wpm;
    collect(
        &s.model.library,
        wpm,
        unix_ms(s.svc.clipboard.clock().now()),
    )
}

pub fn collect(library: &KudzuLibrary, wpm: u32, now_ms: i64) -> Data {
    let usage = usage::usage_by_id(&library.usage);
    let totals = usage::totals(&library.snippets, &usage, wpm);
    let words = js_round(totals.chars as f64 / CHARACTERS_PER_WORD);
    let tiles = [
        Tile {
            label: "Expansions",
            value: format_count(totals.expansions),
            sub: format!("{} snippets used", totals.used_snippets),
        },
        Tile {
            label: "Characters saved",
            value: format_count(totals.chars),
            sub: format!("≈ {} words", format_count(words)),
        },
        Tile {
            label: "Time saved",
            value: format_duration(totals.minutes_saved),
            sub: format!("at {wpm} words per minute"),
        },
        Tile {
            label: "Active snippets",
            value: totals.active_snippets.to_string(),
            sub: format!(
                "{} turned off",
                library.snippets.len() - totals.active_snippets
            ),
        },
    ];

    let mut ranked: Vec<Ranked> = library
        .snippets
        .iter()
        .filter_map(|snippet| {
            let row = usage.get(&snippet.id)?;
            (row.count > 0).then(|| ranked_row(snippet, row.count, row.last_ms, now_ms))
        })
        .collect();
    ranked.sort_by_key(|a| Reverse(a.count));
    let top: Vec<Ranked> = ranked.iter().take(TOP_N).cloned().collect();
    let max = top.first().map(|row| row.count).unwrap_or(1);
    let mut recent = ranked;
    recent.sort_by_key(|a| Reverse(a.last_ms));
    recent.truncate(RECENT_N);

    let unused = library
        .snippets
        .iter()
        .filter(|snippet| {
            snippet.is_enabled
                && usage.get(&snippet.id).is_none_or(|row| row.count == 0)
                && !snippet.triggers.is_empty()
        })
        .take(UNUSED_N)
        .map(|snippet| Unused {
            id: snippet.id,
            label: unused_label(snippet),
        })
        .collect();

    Data {
        tiles,
        top,
        recent,
        unused,
        max,
        wpm,
    }
}

fn ranked_row(snippet: &SnippetRecord, count: i64, last_ms: i64, now_ms: i64) -> Ranked {
    Ranked {
        id: snippet.id,
        name: display_name(snippet),
        trigger: snippet.triggers.first().cloned(),
        count,
        last_ms,
        relative: relative_time(last_ms, now_ms, OFFSET_MINUTES),
    }
}

fn display_name(snippet: &SnippetRecord) -> String {
    if snippet.name.is_empty() {
        "Untitled".into()
    } else {
        snippet.name.clone()
    }
}

fn unused_label(snippet: &SnippetRecord) -> String {
    if snippet.name.is_empty() {
        snippet.triggers.first().cloned().unwrap_or_default()
    } else {
        snippet.name.clone()
    }
}

fn unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn js_round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}
