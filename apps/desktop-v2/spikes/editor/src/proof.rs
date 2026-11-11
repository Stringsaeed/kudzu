//! Headless Runtime evidence for the S13 questions. No window, no system settings.

use std::time::Instant;

use rune::edit::{Ime, Mark};
use rune::*;

use crate::doc::from_template;
use crate::field::{self, field_style, load_edit, selected_pill, slash_query, State};

pub struct Report {
    pub single_clicks: usize,
    pub single_click_selected_pill: usize,
    pub aimed_caret: (usize, usize),
    pub double_click_selection: (usize, usize),
    pub pill: (usize, usize),
    pub enter_opens: bool,
    pub space_opens: bool,
    pub chip_near_caret: bool,
    pub field_caret: Option<[f32; 4]>,
    pub chip_rect: Option<[f32; 4]>,
    pub slash_row: bool,
    pub slash_inserted: bool,
    pub slash_mid_word: bool,
    pub ime_text_during_preedit: String,
    pub ime_committed_outside: bool,
    pub bold_chord: bool,
    pub link_mark: bool,
    pub strike_ignored: bool,
    pub blocks_stay_paragraphs: bool,
    pub mono_width: f32,
    pub sans_width: f32,
    pub spell_menu: Vec<String>,
    pub overlay_hit: String,
    pub keystroke_median_us: u128,
    pub keystroke_runs_us: Vec<u128>,
}

pub fn exercise() -> Report {
    let doc = from_template("Hello {{date}} there", None);
    let pill = doc.pills[0].range.clone();
    let mut rt = runtime(State {
        edit: load_edit(&doc),
        ..State::default()
    });
    focus_field(&mut rt);
    let aimed = aim(&rt, &doc.text[..pill.start], &doc.text[pill.clone()]);
    let mut selected = 0usize;
    let mut samples = 0usize;
    let mut aimed_caret = (0usize, 0usize);
    let y = line_y(&rt);
    let rect = rect_of(&rt, "field");
    let mut x = rect[0] + 4.0;
    while x < rect[0] + rect[2] - 4.0 {
        settle(&mut rt);
        click(&mut rt, x, y);
        samples += 1;
        let (a, b) = (rt.state.edit.anchor, rt.state.edit.focus);
        if a == pill.start && b == pill.end && a != b {
            selected += 1;
        }
        if (x - aimed.0).abs() < 2.0 {
            aimed_caret = (a, b);
        }
        x += 3.0;
    }
    settle(&mut rt);
    click(&mut rt, aimed.0, aimed.1);
    click(&mut rt, aimed.0, aimed.1);
    let double_click_selection = (rt.state.edit.anchor, rt.state.edit.focus);

    let enter_opens = open_with(&doc, &pill, Key::Enter);
    let space_opens = open_with(&doc, &pill, Key::Text(" ".into()));
    let (chip_near_caret, field_caret, chip_rect) = chip_sits_on_caret(&doc, &pill);

    let (slash_row, slash_inserted, slash_mid_word) = slash(&mut rt);
    let (ime_text_during_preedit, ime_committed_outside) = ime();
    let (bold_chord, link_mark, strike_ignored, blocks_stay_paragraphs) = marks();
    let (mono_width, sans_width) = fonts(&mut rt);
    let spell_menu = spell();
    let overlay_hit = overlay();
    let (keystroke_median_us, keystroke_runs_us) = typing_cost();

    Report {
        single_clicks: samples,
        single_click_selected_pill: selected,
        aimed_caret,
        double_click_selection,
        pill: (pill.start, pill.end),
        enter_opens,
        space_opens,
        chip_near_caret,
        field_caret,
        chip_rect,
        slash_row,
        slash_inserted,
        slash_mid_word,
        ime_text_during_preedit,
        ime_committed_outside,
        bold_chord,
        link_mark,
        strike_ignored,
        blocks_stay_paragraphs,
        mono_width,
        sans_width,
        spell_menu,
        overlay_hit,
        keystroke_median_us,
        keystroke_runs_us,
    }
}

pub(crate) fn runtime(state: State) -> Runtime<State> {
    let mut rt = Runtime::new(state, field::view, (720.0, 280.0), Env::new(true, 1.0));
    rt.set_font_families("System Font, Figtree", "Paper Mono, ui-monospace");
    for _ in 0..4 {
        rt.frame(1.0 / 120.0);
    }
    rt
}

pub(crate) fn settle(rt: &mut Runtime<State>) {
    rt.frame(rt.double_click_interval + 0.05);
}

pub(crate) fn click(rt: &mut Runtime<State>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

pub(crate) fn focus_field(rt: &mut Runtime<State>) {
    let rect = rect_of(rt, "field");
    click(rt, rect[0] + 12.0, line_y(rt));
    settle(rt);
}

pub(crate) fn line_y(rt: &Runtime<State>) -> f32 {
    let rect = rect_of(rt, "field");
    rect[1] + 8.0 + 9.0
}

pub(crate) fn rect_of(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .map(|e| e.rect)
        .unwrap_or([0.0, 0.0, 0.0, 0.0])
}

fn aim(rt: &Runtime<State>, prefix: &str, label: &str) -> (f32, f32) {
    let style = field_style(false);
    let (prefix_w, _) = rune::text::measure(prefix, &[], style, None);
    let (label_w, _) = rune::text::measure(label, &[], style, None);
    let rect = rect_of(rt, "field");
    (rect[0] + 8.0 + prefix_w + label_w * 0.35, line_y(rt))
}

fn open_with(doc: &crate::doc::EditorDoc, pill: &std::ops::Range<usize>, key: Key) -> bool {
    let mut rt = runtime(State {
        edit: load_edit(doc),
        ..State::default()
    });
    focus_field(&mut rt);
    rt.state.edit.set_selection(pill.start, pill.end);
    rt.frame(1.0 / 60.0);
    rt.key(key.into());
    rt.frame(1.0 / 60.0);
    rt.state.chip_open && selected_pill(&rt.state.edit).is_some()
}

fn chip_sits_on_caret(
    doc: &crate::doc::EditorDoc,
    pill: &std::ops::Range<usize>,
) -> (bool, Option<[f32; 4]>, Option<[f32; 4]>) {
    let mut rt = runtime(State {
        edit: load_edit(doc),
        ..State::default()
    });
    focus_field(&mut rt);
    rt.state.edit.set_selection(pill.start, pill.end);
    // A write from outside a handler does not rebuild the view. Settle so this frame lays the
    // selection out and publishes `Cx::caret` for the view that opens the popover.
    rt.settle_all();
    rt.frame(1.0 / 60.0);
    rt.key(Key::Enter.into());
    rt.frame(1.0 / 60.0);
    let field_caret = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("field"))
        .and_then(|e| e.caret);
    let anchor = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("caret-anchor"))
        .map(|e| e.rect);
    let chip_rect = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("chip-config"))
        .map(|e| e.rect);
    let style = field_style(false);
    let (prefix_w, _) = rune::text::measure(&doc.text[..pill.start], &[], style, None);
    let (label_w, _) = rune::text::measure(&doc.text[pill.clone()], &[], style, None);
    let field_rect = rect_of(&rt, "field");
    // Glyph width of the prefix plus the label. Pill chrome sits a little past that.
    let focus_x = field_rect[0] + 8.0 + prefix_w + label_w;
    let near = match (field_caret, anchor, chip_rect) {
        (Some(caret), Some(anchor), Some(rect)) => {
            let on_field = (rect[0] - caret[0]).abs() < 8.0 && rect[1] >= caret[1] - 2.0;
            let on_anchor = (rect[0] - anchor[0]).abs() < 8.0
                && (rect[1] - (anchor[1] + anchor[3] + 4.0)).abs() < 8.0;
            let at_pill_end = caret[0] > focus_x - 4.0 && caret[0] < focus_x + 48.0;
            on_field && on_anchor && at_pill_end && rt.state.chip_open
        }
        _ => false,
    };
    (near, field_caret, chip_rect)
}

fn slash(rt: &mut Runtime<State>) -> (bool, bool, bool) {
    rt.state.edit = TextEdit::new("Say ").rich().allow_marks([Mark::Bold]);
    rt.state.edit.set_caret(rt.state.edit.text.len());
    rt.state.chosen = None;
    focus_field(rt);
    rt.state.edit.set_caret(rt.state.edit.text.len());
    rt.key(Key::Text("/".into()).into());
    for _ in 0..6 {
        rt.frame(1.0 / 60.0);
    }
    let row = rt
        .elements()
        .into_iter()
        .any(|e| e.key.as_deref() == Some("slash-option-date"));
    if let Some(row) = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("slash-option-date"))
    {
        let rect = row.rect;
        click(rt, rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5);
        rt.frame(1.0 / 60.0);
    }
    let inserted = rt.state.chosen.as_deref() == Some("date")
        && rt.state.edit.pills().any(|(_, id)| id == "time:{{date}}");
    rt.state.edit = TextEdit::new("hello/x").rich();
    rt.state.edit.set_caret(rt.state.edit.text.len());
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
    }
    let mid = slash_query(&rt.state.edit).is_none()
        && rt
            .elements()
            .into_iter()
            .all(|e| e.exiting || e.key.as_deref() != Some("slash-option-date"));
    (row, inserted, mid)
}

fn ime() -> (String, bool) {
    let doc = from_template("Hello {{date}} there", None);
    let pill = doc.pills[0].range.clone();
    let mut rt = runtime(State {
        edit: load_edit(&doc),
        ..State::default()
    });
    focus_field(&mut rt);
    rt.state.edit.set_caret(pill.end);
    let before = rt.state.edit.text.clone();
    rt.ime(Ime::Preedit("かん".into(), Some((0, 6))));
    rt.frame(1.0 / 60.0);
    let during = rt.state.edit.text.clone();
    let preedit = rt.state.edit.preedit().map(|(t, _)| t.to_string());
    rt.ime(Ime::Commit("感".into()));
    rt.frame(1.0 / 60.0);
    let (range, _) = rt.state.edit.pills().next().unwrap();
    let outside = during == before
        && preedit.as_deref() == Some("かん")
        && range == pill
        && rt.state.edit.text[range.end..].starts_with("感");
    (during, outside)
}

fn marks() -> (bool, bool, bool, bool) {
    let mut rt = runtime(State::default());
    focus_field(&mut rt);
    let hello = "Hello".len();
    rt.state.edit.set_selection(0, hello);
    rt.key(KeyEvent {
        key: Key::Command('b'),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    });
    rt.frame(1.0 / 60.0);
    let bold = rt
        .state
        .edit
        .marks()
        .iter()
        .any(|m| m.mark == Mark::Bold && m.start == 0 && m.end == hello);
    rt.state.edit.set_selection(0, hello);
    rt.state
        .edit
        .set_mark(Mark::Link("https://example.com".into()));
    let link = rt
        .state
        .edit
        .marks()
        .iter()
        .any(|m| matches!(&m.mark, Mark::Link(url) if url == "https://example.com"));
    rt.state.edit.set_mark(Mark::Strike);
    let strike = rt.state.edit.marks().iter().all(|m| m.mark != Mark::Strike);
    rt.state.edit.set_selection(0, rt.state.edit.text.len());
    rt.state.edit.set_block(rune::edit::Block::Bullet(0));
    let blocks = rt
        .state
        .edit
        .blocks()
        .iter()
        .all(|b| *b == rune::edit::Block::Paragraph);
    (bold, link, strike, blocks)
}

fn fonts(rt: &mut Runtime<State>) -> (f32, f32) {
    let sample = "Hello 0123";
    let (mono, _) = rune::text::measure(sample, &[], field_style(false), None);
    rt.state.formatted = true;
    rt.frame(1.0 / 60.0);
    let (sans, _) = rune::text::measure(sample, &[], field_style(true), None);
    rt.state.formatted = false;
    (mono, sans)
}

fn spell() -> Vec<String> {
    let source = "This is mispeled today";
    let word = "This is ".len().."This is mispeled".len();
    let mut edit = TextEdit::new(source).rich();
    edit.set_selection(word.start, word.end);
    let mut rt = runtime(State {
        edit,
        spellcheck: true,
        ..State::default()
    });
    focus_field(&mut rt);
    rt.state.edit.set_selection(word.start, word.end);
    rt.settle_all();
    rt.frame(1.0 / 60.0);
    rt.key(Key::ContextMenu.into());
    rt.frame(1.0 / 60.0);
    let mut found = spell_menu(&rt);
    if !found
        .iter()
        .any(|row| row.contains("learn") || row.contains("suggestion") || row.contains("guess"))
    {
        let style = field_style(false);
        let (prefix, _) = rune::text::measure("This is ", &[], style, None);
        let (word_w, _) = rune::text::measure("mispeled", &[], style, None);
        let rect = rect_of(&rt, "field");
        let x = rect[0] + 8.0 + prefix + word_w * 0.5;
        rt.secondary_down(x, line_y(&rt));
        rt.secondary_up(x, line_y(&rt));
        rt.frame(1.0 / 60.0);
        let more = spell_menu(&rt);
        if !more.is_empty() {
            found = more;
        }
    }
    found
}

/// Ids from the field's own menu. A native menu is not in `elements()`.
fn spell_menu(rt: &Runtime<State>) -> Vec<String> {
    let Some(menu) = rt.popup_menu() else {
        return Vec::new();
    };
    [
        "no-guesses",
        "learn-spelling",
        "suggestion-0",
        "suggestion-1",
        "suggestion-2",
        "cut",
        "copy",
        "paste",
        "spellcheck",
    ]
    .into_iter()
    .filter_map(|id| menu.find(id).map(|item| format!("{id} {}", item.title())))
    .collect()
}

fn overlay() -> String {
    let doc = from_template("Hello {{date}} there", None);
    let mut rt = runtime(State {
        edit: load_edit(&doc),
        overlay: true,
        overlay_at: [40.0, 40.0],
        ..State::default()
    });
    rt.frame(1.0 / 60.0);
    let rect = rect_of(&rt, "bitmap");
    let hit = rt
        .element_at(rect[0] + 8.0, rect[1] + 8.0)
        .and_then(|p| p.key().map(str::to_string))
        .unwrap_or_default();
    if hit.is_empty() {
        if rect[2] > 0.0 {
            "bitmap-drawn".into()
        } else {
            "missing".into()
        }
    } else {
        hit
    }
}

fn typing_cost() -> (u128, Vec<u128>) {
    let body = "alpha beta gamma delta epsilon ".repeat(70);
    assert!(body.chars().count() >= 2000, "{}", body.chars().count());
    let source = format!("{body}{{{{date}}}} and more");
    let doc = from_template(&source, None);
    assert!(doc.text.chars().count() >= 2000);
    let mut runs = Vec::new();
    for _ in 0..5 {
        let mut rt = runtime(State {
            edit: load_edit(&doc),
            formatted: true,
            ..State::default()
        });
        focus_field(&mut rt);
        rt.state.edit.set_caret(rt.state.edit.text.len());
        let mut samples = Vec::new();
        for _ in 0..20 {
            let started = Instant::now();
            rt.key(Key::Text("a".into()).into());
            rt.frame(1.0 / 120.0);
            samples.push(started.elapsed().as_micros());
        }
        samples.sort_unstable();
        runs.push(samples[samples.len() / 2]);
    }
    let mut sorted = runs.clone();
    sorted.sort_unstable();
    (sorted[sorted.len() / 2], runs)
}

pub fn to_json(report: &Report) -> serde_json::Value {
    serde_json::json!({
        "singleClicks": report.single_clicks,
        "singleClickSelectedPill": report.single_click_selected_pill,
        "aimedCaret": [report.aimed_caret.0, report.aimed_caret.1],
        "doubleClickSelection": [report.double_click_selection.0, report.double_click_selection.1],
        "pill": [report.pill.0, report.pill.1],
        "enterOpens": report.enter_opens,
        "spaceOpens": report.space_opens,
        "chipNearCaret": report.chip_near_caret,
        "fieldCaret": report.field_caret,
        "chipRect": report.chip_rect,
        "slashRow": report.slash_row,
        "slashInserted": report.slash_inserted,
        "slashMidWord": report.slash_mid_word,
        "imeTextDuringPreedit": report.ime_text_during_preedit,
        "imeCommittedOutside": report.ime_committed_outside,
        "boldChord": report.bold_chord,
        "linkMark": report.link_mark,
        "strikeIgnored": report.strike_ignored,
        "blocksStayParagraphs": report.blocks_stay_paragraphs,
        "monoWidth": report.mono_width,
        "sansWidth": report.sans_width,
        "spellMenu": report.spell_menu,
        "overlayHit": report.overlay_hit,
        "keystrokeMedianUs": report.keystroke_median_us,
        "keystrokeRunsUs": report.keystroke_runs_us,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_click_does_not_select_a_pill_and_the_rest_of_the_field_holds() {
        let report = exercise();
        let body = serde_json::to_string_pretty(&to_json(&report)).unwrap();
        eprintln!("{body}");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/s13-evidence.json");
        let _ = std::fs::write(&path, &body);
        assert!(report.single_clicks > 10, "sweep did not cross the field");
        assert_eq!(
            report.single_click_selected_pill, 0,
            "a single click selected the pill"
        );
        assert_eq!(
            report.double_click_selection, report.pill,
            "double-click did not select the pill; aimed {:?}",
            report.aimed_caret
        );
        assert!(
            report.enter_opens,
            "Enter on a selected pill did not open configuration"
        );
        assert!(
            report.space_opens,
            "Space on a selected pill did not open configuration"
        );
        assert!(
            report.chip_near_caret,
            "chip popover was not anchored on the caret; caret {:?} chip {:?}",
            report.field_caret, report.chip_rect
        );
        assert!(report.slash_row, "slash menu did not open");
        assert!(
            report.slash_inserted,
            "choosing a slash row did not insert a pill"
        );
        assert!(
            report.slash_mid_word,
            "slash opened in the middle of a word"
        );
        assert!(
            report.ime_committed_outside,
            "commit landed inside a pill; text during preedit {}",
            report.ime_text_during_preedit
        );
        assert!(
            report.bold_chord
                && report.link_mark
                && report.strike_ignored
                && report.blocks_stay_paragraphs
        );
        assert!(
            report.overlay_hit == "bitmap" || report.overlay_hit == "bitmap-drawn",
            "{}",
            report.overlay_hit
        );
        assert!(report.keystroke_median_us > 0);
    }
}
