//! S13b: headless proof for chip gestures, chip copy and paste, a clean load history, and the
//! slash menu by keyboard. No window and no real pasteboard: a `MemoryClipboard` stands in.

use std::ops::Range;

use rune::clipboard::{self, MemoryClipboard};
use rune::edit::Mark;
use rune::*;
use serde_json::{json, Value};

use crate::chips;
use crate::doc::from_template;
use crate::field::{load_edit, selected_pill, OpenVia, State};
use crate::proof::{focus_field, line_y, rect_of, runtime, settle};

/// Writes one measurement under `target/s13b/` for `evidence-s13b.json`.
fn record(name: &str, value: &Value) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/s13b");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_string_pretty(value).unwrap(),
    );
}

fn cmd(c: char) -> KeyEvent {
    KeyEvent {
        key: Key::Command(c),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

fn shift(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    }
}

fn state_for(source: &str, width: Option<f32>) -> State {
    State {
        edit: load_edit(&from_template(source, None)),
        field_width: width,
        ..State::default()
    }
}

// -- mouse -----------------------------------------------------------------------------------

fn move_to(rt: &mut Runtime<State>, x: f32, y: f32) {
    rt.pointer_move(x, y);
    rt.frame(1.0 / 120.0);
}

fn press(rt: &mut Runtime<State>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

/// A mouse that arrives, clicks twice inside the double-click interval, and rests.
fn double_click(rt: &mut Runtime<State>, x: f32, y: f32) {
    move_to(rt, x, y);
    press(rt, x, y);
    press(rt, x, y);
}

fn single_click(rt: &mut Runtime<State>, x: f32, y: f32) {
    move_to(rt, x, y);
    press(rt, x, y);
}

fn drag(rt: &mut Runtime<State>, from: (f32, f32), to: (f32, f32), step: f32) {
    move_to(rt, from.0, from.1);
    rt.pointer_down(from.0, from.1);
    rt.frame(1.0 / 120.0);
    let steps = ((to.0 - from.0).abs() / step).ceil().max(1.0) as usize;
    for i in 1..=steps {
        let x = from.0 + (to.0 - from.0) * i as f32 / steps as f32;
        rt.pointer_move(x, to.1);
        rt.frame(1.0 / 120.0);
    }
    rt.pointer_up(to.0, to.1);
    rt.frame(1.0 / 60.0);
}

/// What one gesture left behind: the selection, and whether the app opened the chip.
#[derive(Debug, Clone, Copy)]
struct Trial {
    selection: (usize, usize),
    opened: Option<OpenVia>,
}

fn reset(rt: &mut Runtime<State>) {
    settle(rt);
    rt.state.chip_open = false;
    rt.state.opened_via = None;
    rt.state.edit.set_caret(0);
    rt.settle_all();
    rt.frame(1.0 / 60.0);
}

fn trial(rt: &mut Runtime<State>, act: impl FnOnce(&mut Runtime<State>)) -> Trial {
    reset(rt);
    act(rt);
    Trial {
        selection: rt.state.edit.range(),
        opened: rt.state.opened_via.filter(|_| rt.state.chip_open),
    }
}

fn pill_of(rt: &Runtime<State>) -> Range<usize> {
    rt.state.edit.pills().next().unwrap().0
}

fn on_pill(t: &Trial, pill: &Range<usize>) -> bool {
    t.selection == (pill.start, pill.end)
}

/// Rune has no API for a pill's rect. The chip's clickable extent on one line is measured from
/// outside instead: the x values whose double-click leaves the selection exactly on the pill.
fn zone(rt: &mut Runtime<State>, y: f32, x0: f32, x1: f32) -> Option<(f32, f32)> {
    let pill = pill_of(rt);
    let mut hits = Vec::new();
    let mut x = x0;
    while x < x1 {
        if on_pill(&trial(rt, |rt| double_click(rt, x, y)), &pill) {
            hits.push(x);
        }
        x += 1.0;
    }
    Some((*hits.first()?, *hits.last()?))
}

fn field_runtime(source: &str, width: Option<f32>) -> Runtime<State> {
    let mut rt = runtime(state_for(source, width));
    focus_field(&mut rt);
    rt
}

// -- Q1: gestures ----------------------------------------------------------------------------

#[test]
fn a_single_click_never_opens_a_chip() {
    let mut rt = field_runtime("Hello {{date}} there", None);
    let field = rect_of(&rt, "field");
    let y = line_y(&rt);
    let mut opened = 0;
    let mut x = field[0] + 4.0;
    while x < field[0] + 330.0 {
        if trial(&mut rt, |rt| single_click(rt, x, y)).opened.is_some() {
            opened += 1;
        }
        x += 2.0;
    }
    record("single_click", &json!({ "opened": opened }));
    assert_eq!(opened, 0);
}

#[test]
fn double_click_opens_a_chip_at_both_edges_and_not_beside_it() {
    let mut rt = field_runtime("Hello {{date}} there", None);
    let pill = pill_of(&rt);
    let field = rect_of(&rt, "field");
    let y = line_y(&rt);
    let (lo, hi) = zone(&mut rt, y, field[0] + 20.0, field[0] + 300.0).expect("a clickable chip");
    assert!(hi - lo > 100.0, "chip zone {lo}..{hi}");

    let left = trial(&mut rt, |rt| double_click(rt, lo, y));
    let right = trial(&mut rt, |rt| double_click(rt, hi, y));
    let middle = trial(&mut rt, |rt| double_click(rt, (lo + hi) / 2.0, y));
    assert!(on_pill(&left, &pill) && left.opened == Some(OpenVia::DoubleClick));
    assert!(on_pill(&right, &pill) && right.opened == Some(OpenVia::DoubleClick));
    assert!(on_pill(&middle, &pill) && middle.opened == Some(OpenVia::DoubleClick));
    // `chip-config` is in the tree after the double-click, under the caret at the chip's end.
    rt.frame(1.0 / 60.0);
    let config = rect_of(&rt, "chip-config");
    let caret = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("field"))
        .and_then(|e| e.caret)
        .expect("a caret");
    assert!(
        config[2] > 0.0 && config[1] >= caret[1],
        "config {config:?} caret {caret:?}"
    );

    // One point outside each edge is the space beside the chip: a word or a space, not the chip.
    let before = trial(&mut rt, |rt| double_click(rt, lo - 4.0, y));
    let after = trial(&mut rt, |rt| double_click(rt, hi + 4.0, y));
    let word_before = trial(&mut rt, |rt| double_click(rt, lo - 20.0, y));
    let word_after = trial(&mut rt, |rt| double_click(rt, hi + 20.0, y));
    for (name, t) in [
        ("before", before),
        ("after", after),
        ("word_before", word_before),
        ("word_after", word_after),
    ] {
        assert!(t.opened.is_none(), "{name}: {t:?}");
        assert!(!on_pill(&t, &pill), "{name}: {t:?}");
    }

    record(
        "double_click_zone",
        &json!({
            "pill": [pill.start, pill.end],
            "zoneX": [lo, hi],
            "left": { "selection": [left.selection.0, left.selection.1], "opened": left.opened.is_some() },
            "right": { "selection": [right.selection.0, right.selection.1], "opened": right.opened.is_some() },
            "beforeEdge": { "selection": [before.selection.0, before.selection.1], "opened": before.opened.is_some() },
            "afterEdge": { "selection": [after.selection.0, after.selection.1], "opened": after.opened.is_some() },
            "wordBefore": { "selection": [word_before.selection.0, word_before.selection.1], "opened": word_before.opened.is_some() },
            "wordAfter": { "selection": [word_after.selection.0, word_after.selection.1], "opened": word_after.opened.is_some() },
        }),
    );
}

#[test]
fn a_word_touching_the_chip_selects_with_it_and_does_not_open_it() {
    // "Hello" and "there" touch the chip, so a word double-click reaches over it.
    let mut rt = field_runtime("Hello{{date}}there", None);
    let pill = pill_of(&rt);
    let field = rect_of(&rt, "field");
    let y = line_y(&rt);
    let word = trial(&mut rt, |rt| double_click(rt, field[0] + 8.0 + 12.0, y));
    assert!(
        word.selection.1 >= pill.end || word.selection.0 == 0,
        "{word:?}"
    );
    assert!(!on_pill(&word, &pill), "{word:?}");
    assert!(word.opened.is_none(), "{word:?}");
    let (lo, hi) = zone(&mut rt, y, field[0] + 20.0, field[0] + 300.0).expect("a clickable chip");
    let chip = trial(&mut rt, |rt| double_click(rt, (lo + hi) / 2.0, y));
    assert_eq!(chip.opened, Some(OpenVia::DoubleClick), "{chip:?}");
    record(
        "touching_word",
        &json!({
            "pill": [pill.start, pill.end],
            "wordSelection": [word.selection.0, word.selection.1],
            "wordOpened": word.opened.is_some(),
            "chipOpened": chip.opened.is_some(),
        }),
    );
}

#[test]
fn drag_and_keyboard_selections_of_a_chip_do_not_open_it() {
    let mut rt = field_runtime("Hello {{date}} there", None);
    let pill = pill_of(&rt);
    let field = rect_of(&rt, "field");
    let y = line_y(&rt);
    let (lo, hi) = zone(&mut rt, y, field[0] + 20.0, field[0] + 300.0).expect("a clickable chip");

    // Across the chip and its neighbours.
    let across = trial(&mut rt, |rt| drag(rt, (lo - 30.0, y), (hi + 30.0, y), 3.0));
    assert!(
        across.selection.0 < pill.start && across.selection.1 > pill.end,
        "{across:?}"
    );
    assert!(across.opened.is_none(), "{across:?}");

    // From one chip edge to the other: the selection really is exactly the chip.
    let exact = trial(&mut rt, |rt| drag(rt, (lo + 1.0, y), (hi - 1.0, y), 3.0));
    assert!(on_pill(&exact, &pill), "{exact:?}");
    assert!(exact.opened.is_none(), "{exact:?}");

    // A slow drag in 1 pt steps ends on the chip too.
    let slow = trial(&mut rt, |rt| drag(rt, (lo + 1.0, y), (hi - 1.0, y), 1.0));
    assert!(on_pill(&slow, &pill), "{slow:?}");
    assert!(slow.opened.is_none(), "{slow:?}");

    // A 2 pt drag across the point where the caret flips from one chip edge to the other lands
    // on exactly the chip with almost no travel. Look for it, then check it does not open.
    let mid = (lo + hi) / 2.0;
    let mut tiny_exact = Vec::new();
    let mut x = lo + 2.0;
    while x < hi - 2.0 {
        let t = trial(&mut rt, |rt| drag(rt, (x - 1.0, y), (x + 1.0, y), 2.0));
        assert!(
            t.opened.is_none(),
            "a 2 pt drag at {x} opened the chip: {t:?}"
        );
        if on_pill(&t, &pill) {
            tiny_exact.push(x);
        }
        x += 1.0;
    }

    // Shift+Right from the chip's left edge selects the chip, and nothing opens by itself,
    // even when a stray pointer event arrives afterwards at a point the mouse rests on.
    reset(&mut rt);
    move_to(&mut rt, mid, y);
    press(&mut rt, mid, y);
    move_to(&mut rt, mid, y);
    rt.state.edit.set_caret(pill.start);
    rt.settle_all();
    rt.key(shift(Key::Right));
    rt.frame(1.0 / 60.0);
    assert_eq!(
        selected_pill(&rt.state.edit).map(|(r, _)| r),
        Some(pill.clone())
    );
    assert!(!rt.state.chip_open);
    rt.pointer_up(mid, y);
    move_to(&mut rt, mid, y);
    move_to(&mut rt, mid, y);
    assert!(
        !rt.state.chip_open,
        "a pointer event after a keyboard selection opened the chip"
    );
    let keyboard_open = rt.state.chip_open;

    // The keyboard still opens it, on request.
    rt.key(Key::Enter.into());
    rt.frame(1.0 / 60.0);
    assert!(rt.state.chip_open && rt.state.opened_via == Some(OpenVia::Return));

    record(
        "drag_and_keyboard",
        &json!({
            "across": [across.selection.0, across.selection.1],
            "acrossOpened": across.opened.is_some(),
            "exactDragSelection": [exact.selection.0, exact.selection.1],
            "exactDragOpened": exact.opened.is_some(),
            "slowDragSelection": [slow.selection.0, slow.selection.1],
            "slowDragOpened": slow.opened.is_some(),
            "twoPointDragsThatSelectExactlyTheChip": tiny_exact,
            "twoPointDragsThatOpenedIt": 0,
            "shiftArrowOpenedByItself": keyboard_open,
            "returnAfterShiftArrow": true,
        }),
    );
}

/// Known limit: the detector wants the two releases at exactly one point. A mouse that nudges
/// one point between the presses is still a double-click to Rune (inside its 4 pt slop), which
/// selects the chip, but the app sees a moved pointer and does not open it.
#[test]
fn known_limit_a_double_click_with_a_nudge_selects_the_chip_but_is_missed() {
    let mut rt = field_runtime("Hello {{date}} there", None);
    let pill = pill_of(&rt);
    let y = line_y(&rt);
    let x = 120.0;
    let t = trial(&mut rt, |rt| {
        move_to(rt, x, y);
        press(rt, x, y);
        move_to(rt, x + 1.0, y);
        press(rt, x + 1.0, y);
    });
    record(
        "known_limit_nudge",
        &json!({ "selection": [t.selection.0, t.selection.1], "opened": t.opened.is_some() }),
    );
    assert!(
        on_pill(&t, &pill),
        "Rune did not treat the nudged click as a double-click: {t:?}"
    );
    assert!(t.opened.is_none(), "{t:?}");
}

/// A drag's selection change is queued by the move that caused it, so the app sees the change on
/// the next event, which is the second at that point however many the host repeats. A
/// double-click needs the change on the third. Repeated move events do not turn a drag into one.
#[test]
fn repeated_move_events_do_not_make_a_drag_onto_a_chip_look_like_a_double_click() {
    let mut rt = field_runtime("Hello {{date}} there", None);
    let pill = pill_of(&rt);
    let field = rect_of(&rt, "field");
    let y = line_y(&rt);
    let (lo, hi) = zone(&mut rt, y, field[0] + 20.0, field[0] + 300.0).unwrap();
    let mut results = Vec::new();
    for repeats in [0usize, 1, 2, 5] {
        let t = trial(&mut rt, |rt| {
            move_to(rt, lo + 1.0, y);
            rt.pointer_down(lo + 1.0, y);
            rt.frame(1.0 / 120.0);
            rt.pointer_move(hi - 1.0, y);
            rt.frame(1.0 / 120.0);
            for _ in 0..repeats {
                rt.pointer_move(hi - 1.0, y);
                rt.frame(1.0 / 120.0);
            }
            rt.pointer_up(hi - 1.0, y);
            rt.frame(1.0 / 60.0);
        });
        assert!(on_pill(&t, &pill), "{repeats} repeats: {t:?}");
        results.push((repeats, t.opened.is_some()));
    }
    record(
        "repeated_move_events",
        &json!({ "repeatsThenOpened": results.iter().map(|r| json!([r.0, r.1])).collect::<Vec<_>>() }),
    );
    assert_eq!(results, [(0, false), (1, false), (2, false), (5, false)]);
}

thread_local! {
    static VIEWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn counting_view(s: &State, cx: &mut Cx<State>) -> El {
    VIEWS.with(|n| n.set(n.get() + 1));
    crate::field::view(s, cx)
}

/// The pointer handler runs on every move over the field. It must not rebuild the view each time.
#[test]
fn the_pointer_handler_does_not_rebuild_the_view_on_each_move() {
    let mut rt = Runtime::new(
        state_for("Hello {{date}} there", None),
        counting_view,
        (720.0, 280.0),
        Env::new(true, 1.0),
    );
    rt.set_font_families("System Font, Figtree", "Paper Mono, ui-monospace");
    for _ in 0..4 {
        rt.frame(1.0 / 120.0);
    }
    focus_field(&mut rt);
    settle(&mut rt);
    let y = line_y(&rt);
    let before = VIEWS.with(|n| n.get());
    for i in 0..200 {
        rt.pointer_move(20.0 + (i % 100) as f32, y);
        rt.frame(1.0 / 120.0);
    }
    let rebuilt = VIEWS.with(|n| n.get()) - before;
    record(
        "pointer_handler_cost",
        &json!({ "moves": 200, "viewRuns": rebuilt }),
    );
    assert!(rebuilt <= 20, "{rebuilt} view runs for 200 moves");
}

#[test]
fn space_and_return_open_a_selected_chip_and_only_a_selected_chip() {
    for (key, via) in [
        (Key::Enter, OpenVia::Return),
        (Key::Text(" ".into()), OpenVia::Space),
    ] {
        let mut rt = field_runtime("Hello {{date}} there", None);
        let pill = pill_of(&rt);
        rt.state.edit.set_caret(pill.end);
        rt.key(key.clone().into());
        rt.frame(1.0 / 60.0);
        assert!(
            !rt.state.chip_open,
            "{key:?} opened a chip with only a caret beside it"
        );
        rt.state.edit.set_selection(pill.start, pill.end);
        rt.key(key.into());
        rt.frame(1.0 / 60.0);
        assert!(rt.state.chip_open && rt.state.opened_via == Some(via));
    }
}

#[test]
fn a_chip_that_wraps_across_lines_opens_from_either_line() {
    let mut rt = field_runtime("Hello {{date}} there", Some(130.0));
    let pill = pill_of(&rt);
    let field = rect_of(&rt, "field");
    let top = field[1] + 8.0;
    // Which (x, y) points double-click onto the chip, row by row.
    let mut rows: Vec<(f32, Vec<f32>)> = Vec::new();
    let mut y = top + 4.0;
    while y < top + 70.0 {
        let mut hits = Vec::new();
        let mut x = field[0] + 6.0;
        while x < field[0] + 126.0 {
            let t = trial(&mut rt, |rt| double_click(rt, x, y));
            if on_pill(&t, &pill) {
                assert_eq!(t.opened, Some(OpenVia::DoubleClick), "({x}, {y}) {t:?}");
                hits.push(x);
            } else {
                assert!(t.opened.is_none(), "({x}, {y}) {t:?}");
            }
            x += 4.0;
        }
        if !hits.is_empty() {
            rows.push((y, hits));
        }
        y += 4.0;
    }
    let spans: Vec<(f32, f32, f32)> = rows
        .iter()
        .map(|(y, xs)| (*y, xs[0], *xs.last().unwrap()))
        .collect();
    record(
        "wrapped_chip",
        &json!({
            "fieldWidth": 130.0,
            "fieldLeft": field[0],
            "rows": spans.iter().map(|s| json!({"y": s.0, "xFrom": s.1, "xTo": s.2})).collect::<Vec<_>>(),
        }),
    );
    // The chip fills its first line out to the right margin and ends part-way along a later
    // one: two lines of chip, 20 pt or so apart, with the hits on both.
    let first = spans.first().unwrap();
    let last = spans.last().unwrap();
    assert!(
        last.0 - first.0 >= 36.0 && first.2 > last.2 + 40.0,
        "the chip did not wrap: {spans:?}"
    );
}

// -- Q2: copy, cut and paste -----------------------------------------------------------------

struct Plain {
    edit: TextEdit,
}

fn plain_view(s: &Plain, _: &mut Cx<Plain>) -> El {
    text_input(&s.edit, TextStyle::new(14.0, 400.0))
        .key("field")
        .size(400.0, 100.0)
        .on_edit(|s: &mut Plain| &mut s.edit)
}

fn kudzu_with_clipboard(source: &str, board: &MemoryClipboard) -> Runtime<State> {
    let mut rt = runtime(state_for(source, None));
    rt.use_clipboard(board);
    focus_field(&mut rt);
    rt
}

fn copy_from(
    rt: &mut Runtime<State>,
    board: &MemoryClipboard,
    a: usize,
    b: usize,
) -> ClipboardContent {
    rt.state.edit.set_selection(a, b);
    rt.key(cmd('c'));
    rt.frame(1.0 / 60.0);
    board.content().expect("a copy")
}

#[test]
fn rune_copies_the_chip_label_as_plain_text_today() {
    let doc = from_template("Hello {{date}} there", None);
    let mut edit = load_edit(&doc);
    edit.focused = true;
    edit.set_selection(0, edit.text.len());
    let board = clipboard::memory();
    let mut rt = Runtime::new(
        Plain { edit },
        plain_view,
        (420.0, 120.0),
        Env::new(true, 1.0),
    );
    rt.use_clipboard(&board);
    rt.frame(0.0);
    rt.key(cmd('c'));
    rt.frame(0.0);
    let copied = board.content().unwrap();
    record(
        "rune_default_copy",
        &json!({ "text": copied.text(), "html": copied.html() }),
    );
    assert_eq!(copied.text(), "Hello Date \u{00b7} ISO date there");
    let html = copied.html().expect("a rich field copies HTML");
    assert!(
        html.contains("<span data-pill=\"time:{{date}}\">Date \u{00b7} ISO date</span>"),
        "{html}"
    );
}

#[test]
fn copy_puts_the_template_source_in_plain_text_and_the_chips_in_html() {
    let board = clipboard::memory();
    let mut rt = kudzu_with_clipboard("Hello {{date}} there", &board);
    let len = rt.state.edit.text.len();
    let all = copy_from(&mut rt, &board, 0, len);
    assert_eq!(all.text(), "Hello {{date}} there");
    let html = all.html().unwrap();
    assert!(
        html.contains("<span data-pill=\"time:{{date}}\">"),
        "{html}"
    );

    let pill = pill_of(&rt);
    let only = copy_from(&mut rt, &board, pill.start, pill.end);
    assert_eq!(only.text(), "{{date}}");

    // A selection that begins in the middle of the chip grows over it (set_selection snaps).
    let grown = copy_from(&mut rt, &board, pill.start + 3, pill.end + 3);
    assert_eq!(grown.text(), "{{date}} th");

    let tail = copy_from(&mut rt, &board, 3, pill.end);
    assert_eq!(tail.text(), "lo {{date}}");

    record(
        "kudzu_copy",
        &json!({
            "all": { "text": all.text(), "html": all.html() },
            "chipOnly": only.text(),
            "chipAndNextLetter": grown.text(),
            "tailAndChip": tail.text(),
        }),
    );
}

#[test]
fn copy_escapes_literal_braces_and_backslashes_like_the_serializer() {
    let source = r"a \{{ b \\ c {{date}} \{{d}}";
    let board = clipboard::memory();
    let mut rt = kudzu_with_clipboard(source, &board);
    let len = rt.state.edit.text.len();
    assert_eq!(
        rt.state.edit.text,
        "a {{ b \\ c Date \u{00b7} ISO date {{d}}"
    );
    let all = copy_from(&mut rt, &board, 0, len);
    assert_eq!(all.text(), source);
    // The text between chips, copied alone, keeps its escapes too.
    let text = rt.state.edit.text.clone();
    let literal = copy_from(&mut rt, &board, 0, text.find("Date").unwrap());
    assert_eq!(literal.text(), r"a \{{ b \\ c ");
}

#[test]
fn a_mixed_selection_with_bold_text_keeps_the_marks_in_html_and_the_source_in_plain() {
    let board = clipboard::memory();
    let mut rt = kudzu_with_clipboard("Hello {{date}} there", &board);
    rt.state.edit.set_selection(0, 5);
    rt.state.edit.set_mark(Mark::Bold);
    let len = rt.state.edit.text.len();
    let all = copy_from(&mut rt, &board, 0, len);
    assert_eq!(all.text(), "Hello {{date}} there");
    let html = all.html().unwrap();
    assert!(html.contains("<strong>Hello</strong>"), "{html}");
    assert!(html.contains("data-pill=\"time:{{date}}\""), "{html}");
}

fn paste_into_second_window(board: &MemoryClipboard, text: &str) -> Runtime<State> {
    let mut rt = runtime(State {
        edit: load_edit(&from_template(text, None)),
        ..State::default()
    });
    rt.use_clipboard(board);
    focus_field(&mut rt);
    let end = rt.state.edit.text.len();
    rt.state.edit.set_caret(end);
    rt.settle_all();
    rt.update(|_, tx| tx.request_paste());
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    rt
}

#[test]
fn same_app_paste_recreates_the_chips_and_is_one_undo_step() {
    let board = clipboard::memory();
    let mut a = kudzu_with_clipboard("Hello {{date}} there", &board);
    let len = a.state.edit.text.len();
    let copied = copy_from(&mut a, &board, 0, len);
    assert_eq!(copied.text(), "Hello {{date}} there");

    let mut b = paste_into_second_window(&board, "Say: ");
    let edit = &b.state.edit;
    assert_eq!(edit.text, "Say: Hello Date \u{00b7} ISO date there");
    assert_eq!(edit.pills().count(), 1);
    assert_eq!(
        chips::source_of(edit, 0..edit.text.len()),
        "Say: Hello {{date}} there"
    );
    let (range, id) = edit.pills().next().unwrap();
    assert_eq!(id, "time:{{date}}");
    assert_eq!(&edit.text[range], "Date \u{00b7} ISO date");

    // One Cmd+Z removes the whole paste.
    b.key(cmd('z'));
    b.frame(1.0 / 60.0);
    assert_eq!(b.state.edit.text, "Say: ");
    assert_eq!(b.state.edit.pills().count(), 0);
}

#[test]
fn a_literal_only_copy_pastes_back_as_the_same_display_text() {
    let board = clipboard::memory();
    let mut a = kudzu_with_clipboard(r"a \{{ b \\ c", &board);
    let len = a.state.edit.text.len();
    assert_eq!(a.state.edit.text, "a {{ b \\ c");
    let copied = copy_from(&mut a, &board, 0, len);
    assert_eq!(copied.text(), r"a \{{ b \\ c");
    let b = paste_into_second_window(&board, "");
    assert_eq!(b.state.edit.text, "a {{ b \\ c");
    assert_eq!(
        chips::source_of(&b.state.edit, 0..b.state.edit.text.len()),
        r"a \{{ b \\ c"
    );
}

#[test]
fn another_apps_clipboard_is_not_taken_for_a_chip_copy() {
    let board = clipboard::memory();
    // The same shape Rune's own tests use for a foreign copy: its pill id is not a Kudzu tag.
    board.set_content(
        ClipboardContent::new("a @x-agent b")
            .with_html("a <span data-pill=\"agent:x\">@x-agent</span> b"),
    );
    let foreign = paste_into_second_window(&board, "");
    assert_eq!(foreign.state.edit.text, "a @x-agent b");
    assert_eq!(foreign.state.edit.pills().count(), 0);

    // Plain template text from another app lands as literal text. Parsing it into chips is a
    // product choice for S52, not something Rune does.
    board.set_text("Hello {{date}}");
    let plain = paste_into_second_window(&board, "");
    assert_eq!(plain.state.edit.text, "Hello {{date}}");
    assert_eq!(plain.state.edit.pills().count(), 0);

    // HTML whose plain text is not the source of its own chips is another app's, even with a
    // Kudzu-shaped id in it.
    board.set_content(
        ClipboardContent::new("different text")
            .with_html("x <span data-pill=\"time:{{date}}\">Date</span>"),
    );
    let mismatch = paste_into_second_window(&board, "");
    assert_eq!(mismatch.state.edit.pills().count(), 0);
    assert_eq!(mismatch.state.edit.text, "different text");
}

#[test]
fn cut_puts_the_source_on_the_pasteboard_and_undo_brings_the_chip_back() {
    let board = clipboard::memory();
    let mut rt = kudzu_with_clipboard("Hello {{date}} there", &board);
    let pill = pill_of(&rt);
    rt.state.edit.set_selection(3, pill.end + 2);
    rt.key(cmd('x'));
    rt.frame(1.0 / 60.0);
    let cut = board.content().unwrap();
    assert_eq!(cut.text(), "lo {{date}} t");
    assert!(cut.html().unwrap().contains("data-pill=\"time:{{date}}\""));
    assert_eq!(rt.state.edit.text, "Helhere");
    assert_eq!(rt.state.edit.pills().count(), 0);

    rt.key(cmd('z'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.edit.text, "Hello Date \u{00b7} ISO date there");
    let (range, id) = rt.state.edit.pills().next().expect("the chip is back");
    assert_eq!(range, pill);
    assert_eq!(id, "time:{{date}}");
    let len = rt.state.edit.text.len();
    assert_eq!(
        chips::source_of(&rt.state.edit, 0..len),
        "Hello {{date}} there"
    );

    // Redo cuts again; the pasteboard is untouched by undo and redo.
    let mut redo = cmd('z');
    redo.mods.shift = true;
    rt.key(redo);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.edit.text, "Helhere");
    assert_eq!(board.content().unwrap().text(), "lo {{date}} t");
}

// -- Q3: a loaded document has no history ------------------------------------------------------

#[test]
fn the_first_undo_on_an_untouched_document_does_nothing() {
    let source = "Hello {{date}} there";
    let formatting = kudzu_core::formatting::Formatting {
        source: source.into(),
        spans: vec![kudzu_core::formatting::FormatSpan {
            start: 0,
            end: 5,
            style: kudzu_core::formatting::FormatStyle::Bold,
        }],
    };
    let doc = from_template(source, Some(&formatting));
    let mut edit = load_edit(&doc);
    assert!(edit.marks().iter().any(|m| m.mark == Mark::Bold));
    let (text, pills, marks) = (
        edit.text.clone(),
        edit.pills()
            .map(|(r, id)| (r, id.to_string()))
            .collect::<Vec<_>>(),
        edit.marks().to_vec(),
    );
    edit.undo();
    edit.redo();
    assert_eq!(edit.text, text);
    assert_eq!(
        edit.pills()
            .map(|(r, id)| (r, id.to_string()))
            .collect::<Vec<_>>(),
        pills
    );
    assert_eq!(edit.marks(), marks.as_slice());

    // The same through the keyboard, in a running field.
    let mut rt = field_runtime("Hello {{date}} there", None);
    let before = rt.state.edit.text.clone();
    rt.key(cmd('z'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.edit.text, before);
    assert_eq!(rt.state.edit.pills().count(), 1);

    // After a user edit, one undo reverts that edit and the next reverts nothing.
    rt.state.edit.set_caret(rt.state.edit.text.len());
    rt.key(Key::Text("!".into()).into());
    rt.frame(1.0 / 60.0);
    assert!(rt.state.edit.text.ends_with('!'));
    rt.key(cmd('z'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.edit.text, before);
    assert_eq!(rt.state.edit.pills().count(), 1);
    rt.key(cmd('z'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.edit.text, before);
    assert_eq!(rt.state.edit.pills().count(), 1);
}

#[test]
fn loading_without_clearing_history_lets_undo_strip_the_chips() {
    // What S13 shipped: the load's undo group stays on the stack.
    let doc = from_template("Hello {{date}} there", None);
    let mut edit = TextEdit::new(&doc.text).rich();
    edit.undo_group(|edit| {
        for pill in doc.pills.iter().rev() {
            edit.replace_with_pill(pill.range.clone(), &pill.id, &pill.label);
        }
    });
    assert_eq!(edit.pills().count(), 1);
    edit.undo();
    assert_eq!(
        edit.pills().count(),
        0,
        "the first undo removed the loaded chip"
    );
}

// -- Q4: the slash menu by keyboard ------------------------------------------------------------

fn slash_runtime(text: &str) -> Runtime<State> {
    let mut rt = runtime(State {
        edit: load_edit(&from_template(text, None)),
        ..State::default()
    });
    focus_field(&mut rt);
    let end = rt.state.edit.text.len();
    rt.state.edit.set_caret(end);
    rt.settle_all();
    rt
}

fn type_text(rt: &mut Runtime<State>, text: &str) {
    for ch in text.chars() {
        rt.key(Key::Text(ch.to_string()).into());
        for _ in 0..3 {
            rt.frame(1.0 / 60.0);
        }
    }
}

fn press_key(rt: &mut Runtime<State>, key: Key) {
    rt.key(key.into());
    for _ in 0..6 {
        rt.frame(1.0 / 60.0);
    }
}

fn row(rt: &Runtime<State>, id: &str) -> bool {
    rt.elements()
        .into_iter()
        .any(|e| !e.exiting && e.key.as_deref() == Some(&format!("slash-option-{id}")))
}

#[test]
fn slash_da_then_return_inserts_the_date_chip_with_no_mouse() {
    let mut rt = slash_runtime("Say ");
    type_text(&mut rt, "/da");
    assert!(row(&rt, "date"), "the menu did not open on /da");
    assert!(!row(&rt, "clipboard"), "the query did not filter the rows");
    press_key(&mut rt, Key::Enter);
    let edit = &rt.state.edit;
    assert_eq!(rt.state.chosen.as_deref(), Some("date"));
    assert_eq!(
        edit.text, "Say Date \u{00b7} ISO date",
        "no newline and no leftover query"
    );
    let (range, id) = edit.pills().next().expect("a chip");
    assert_eq!(id, "time:{{date}}");
    assert_eq!(&edit.text[range.clone()], "Date \u{00b7} ISO date");
    assert_eq!(chips::source_of(edit, 0..edit.text.len()), "Say {{date}}");
    assert!(!row(&rt, "date"), "the menu stayed open");
    record(
        "slash_keyboard",
        &json!({ "typed": "/da", "then": "Return", "text": edit.text, "chipId": id, "source": "Say {{date}}" }),
    );
}

#[test]
fn slash_arrows_move_the_row_escape_closes_and_a_miss_falls_through_to_return() {
    let mut rt = slash_runtime("Say ");
    type_text(&mut rt, "/");
    assert!(row(&rt, "date") && row(&rt, "cursor"));
    press_key(&mut rt, Key::Down);
    press_key(&mut rt, Key::Down);
    assert_eq!(rt.state.slash_active, 2);
    press_key(&mut rt, Key::Up);
    press_key(&mut rt, Key::Down);
    press_key(&mut rt, Key::Enter);
    assert_eq!(rt.state.chosen.as_deref(), Some("clipboard"));
    assert_eq!(
        rt.state.edit.pills().next().unwrap().1,
        "time:{{clipboard}}"
    );

    // Escape closes the menu; Return then types a newline.
    let mut rt = slash_runtime("Say ");
    type_text(&mut rt, "/da");
    press_key(&mut rt, Key::Escape);
    assert!(!row(&rt, "date"), "Escape left the menu open");
    assert_eq!(rt.state.edit.text, "Say /da");
    press_key(&mut rt, Key::Enter);
    assert_eq!(rt.state.edit.text, "Say /da\n");
    assert_eq!(rt.state.edit.pills().count(), 0);

    // No row matches: Return is a newline, not a pick.
    let mut rt = slash_runtime("Say ");
    type_text(&mut rt, "/zz");
    press_key(&mut rt, Key::Enter);
    assert_eq!(rt.state.edit.text, "Say /zz\n");
    assert!(rt.state.chosen.is_none());
}
