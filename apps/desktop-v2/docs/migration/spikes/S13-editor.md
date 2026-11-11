# S13: template editor on TextEdit

2026-11-11. Rune `29aacb688883a9712bc27175bdafd99fc6edb421` (read from the
provided read-only clone; the README lags, so the source wins). The spike
lives in `spikes/editor`. No installed Kudzu, system settings, permissions,
or upstream issues were changed. No comments were posted.

The rich field can host chips, marks, a slash menu, undo, IME, and spell
checking. One click on a chip does not open its configuration. That is a
**no-go for the owner** on the mechanism in §3 gap 6. It is not an outright
failure of `TextEdit`, and this step does not build the app-drawn editor
from §5 risk 1.

## Reproduce

```bash
cd apps/desktop-v2
cargo test -p editor-spike -- --test-threads=1
cargo run -p editor-spike -- --snap spikes/editor/snapshots/chips.png --scene chips
```

`--snap` is offscreen. Without a headless flag the binary runs the same proof
through `Runtime` and prints JSON; it does not open a window. Warm `target/`
was cloned with the requested `cp -cR` before the first build. Builds in this
worktree were sequential. Other agents were compiling, so timings are noisy.
Machine observations below are medians of repeated runs, not a budget.

Headless scenes: `spikes/editor/snapshots/chips.png` (mono, date chip),
`formatted.png` (sans), `slash.png` (`PopupList` of Date, Time, Clipboard,
Cursor). Numbers: `spikes/editor/evidence.json`.

## Decisions per question

| Question | Decision | Evidence |
|---|---|---|
| Tags as `Mark::Pill` from a parsed template | **go** | `loaded_pills_match_the_doc_and_typing_does_not_extend_them`. `Hello {{date}} there` displays `Date · ISO date` at bytes 6..22. Typing beside the pill does not extend it. A caret placed inside the label snaps to an edge. Snapshot `chips.png`. |
| One click opens chip configuration | **no-go** | `one_click_does_not_select_a_pill_and_the_rest_of_the_field_holds`. 238 single clicks, 0 left the selection on the pill. The aimed click's caret was collapsed at (6, 6), the pill's start. Double-click at that point selected (6, 22). The spike does not open the popover on double-click, Space, or a measurement overlay. |
| What a click selects, and how a popover anchors | **go** for the anchor, on the secondary path | A single click snaps to the nearer edge and selects nothing. Return or Space on a selection that is exactly one pill opens `chip-config` and keeps the selection (`enterOpens`, `spaceOpens`). After settling the selection, Enter places the popover on `Cx::caret`: field caret `[200.16, 8.0, 1.5, 18.0]`, chip `[200.16, 30.0, 146.0, 28.25]` (4 pt under the caret line). An external write does not rebuild the view until `settle_all` or a handler dirties it. |
| Spell checking | **go** | Menu key on the selected word `mispeled`, with `El::spellcheck(true)`. `popup_menu()` returned `learn-spelling Learn Spelling` and `suggestion-0 misspelled`, plus Cut/Copy/Paste. The element tree has no underline rects, so the squiggle was not measured. The Spellcheck toggle is absent because the spike does not set `on_spellcheck`. |
| Slash menu at a word start | **go** | `slash_opens_only_at_a_word_start` and the proof. `/` after `Say ` opens `slash-option-date`. Choosing it inserts pill id `time:date`. `hello/x` does not open a menu (exiting rows ignored). Snapshot `slash.png`. |
| Insert a pill at the selection | **go** | `replace_with_pill` from the slash row, and `load_edit` which replaces each chip range with its label. The label is the document text. |
| Undo groups a pill insert and a mark change | **go** | `undo_group_reverts_a_pill_insert_and_a_mark_together`. One `undo` removes the pill and the bold mark; `redo` restores both. |
| Bold, italic, underline, links; no block formats | **go** | `allow_marks` of bold, italic, underline, and link. ⌘B applies bold. A link mark applies. Strike is refused. `allow_blocks(std::iter::empty::<Block>())` — `allow_blocks([])` does not type-check. Paragraph stays allowed. `set_block(Bullet)` leaves paragraphs. Codec test round-trips bold, italic, underline, and a link over the whole fixture source, including across the date chip. |
| Image as a pill; bitmap over a reserved range | **go** for the pill (D2). **no** for a bitmap in the text run | `formatting_over_a_chip_and_an_image_round_trip` and `image_pill_is_atomic_in_the_field`. A safe image span whose range is one literal becomes a pill labelled with that alt (`Hello`, width 32). The field holds the image pill and the date pill. `element_at` on an absolute `El::pixels` sibling returns key `bitmap`, so a bitmap beside the field is drawn and hit. That overlay does not wrap, scroll, or share hit-testing with a text range. S76 cannot use it as an inline image. |
| IME next to pills | **go** | Preedit `かん` leaves `text` unchanged (`imeTextDuringPreedit` is `Hello Date · ISO date there`). Commit `感` lands outside the pill. The pill's range does not grow. |
| Mono for plain, sans for formatted | **go** | `TextStyle::mono` vs `family(Family::Sans)`, families `System Font, Figtree` and `Paper Mono, ui-monospace`. `text::measure` of `Hello 0123` is 85 pt mono and 69 pt sans. Snapshots `chips.png` and `formatted.png`. The element tree does not report the family. |
| Typing cost on a 2,000-character snippet (#535) | **go** as a measurement, no budget failure | Five runs of 20 `Key::Text("a")` plus `frame(1/120)` on a field of at least 2,000 characters with a pill. Committed run medians `[3632, 1577, 1581, 1586, 1581]` µs, median **1581 µs**. Three earlier process medians in this worktree: 1613, 1648, 1662 µs. The first sample of a process is colder (about 3.6–4.0 ms). |
| Codec round-trips the grammar corpus | **go** | `grammar_and_fixture_sources_round_trip_byte_exact` (more than 40 sources). `to_template(from_template(s)).0 == s` for the grammar corpus, every `template.json` case source, escapes, emoji, and unknown tags. `formatting_over_a_chip_and_an_image_round_trip` keeps span endpoints. |

## EditorDoc for S23

Rune's pill label is the document text. The field cannot keep `{{date}}` in
`TextEdit.text` and show `Date · ISO date`. Display text and template source
also diverge on escapes (`\{{`, `\\`). The four fields in the S23 sketch
cannot rebuild those bytes. S23 should store an ordered source map as well.

```rust,ignore
struct EditorDoc {
    text: String,                 // display string (literals + chip labels)
    pills: Vec<EditorPill>,       // display byte range, id, exact source slice, label, tone, kind
    marks: Vec<EditorMark>,       // display byte ranges; a span that cuts a pill grows to the pill
    images: Vec<EditorImage>,     // pill index, data_url, width, alt
    map: Vec<SourcePiece>,        // concatenating `source` is the template, byte-exact
}

struct SourcePiece {
    display: Range<usize>,
    source: String,
    source_utf16: Range<usize>,
    pill: Option<usize>,
    /// Literal graphemes: (display byte, source utf-16), both from the piece start.
    /// Pills leave this empty and map as a whole.
    steps: Vec<(usize, usize)>,
}
```

`from_template_named(source, formatting, &dyn Fn(&str) -> Option<&str>)` is
the snippet-name seam. The fixture
`{{snippet id="c5af65ac-e426-4046-802b-11ccdd3a62ba"}}` labels
`↳ Email signature` when that id is supplied, and `↳ Missing snippet`
otherwise (`a_snippet_with_no_name_reads_missing`). `from_template` passes an
empty lookup.

Apply image spans that pass `safe_image` before mapping marks. Apply
formatting only when `formatting.source` is empty or equals the source.
Unknown and verbatim tags keep their original bytes; the label is the source
and the tone is error for verbatim, form for any other unknown tag. Eleven
date presets. Chip ids in the field are `{tone}:{index}`.

`load_edit` is `TextEdit::new(&doc.text).rich().allow_marks([Bold, Italic, Underline, Link("")]).allow_blocks(empty iterator)`, then one `undo_group` of `replace_with_pill` (labels already match `text`) and `set_mark`. S13 left that load as one undo step; S13b clears it (see Follow-up).

## Upstream comments for the owner

No issues were filed and no comments were posted. Proposed text:

- Existing [#128](https://github.com/anomalyco/rune/issues/128):
  One click on a pill snaps the caret to an edge and does not select the pill, so Kudzu cannot open chip configuration that way.
  At 29aacb6, 238 single clicks on `Hello {{date}} there` selected bytes 6..22 zero times (the aimed caret was collapsed at 6..6); double-click selected 6..22, and `on_press` does not fire inside `text_input`.
- Existing [#353](https://github.com/anomalyco/rune/issues/353):
  There is no public hit-test from a window point to a byte, so an app cannot tell a click inside a pill from a click on its edge, or take a pill's rect.
  `element_at` returns only a path and a key; an absolute `El::pixels` sibling is hittable (`bitmap`), and that does not track a reserved range through wrap or scroll.
- Existing [#350](https://github.com/anomalyco/rune/issues/350):
  Snippet images need an atomic inline image with width and alt, not only a pill icon (`pill_icon` takes `&'static Icon`).
  Kudzu will keep a pill plus a preview popover (decision D2) until that exists; S13 found no way to draw a bitmap over a reserved text range.
- Existing [#354](https://github.com/anomalyco/rune/issues/354):
  The editor toolbar needs to know whether undo and redo are available. `undo` and `redo` exist, and `undo_group` around `replace_with_pill` plus `set_mark` is one step (`undo_group_reverts_a_pill_insert_and_a_mark_together`).
  Nothing on `TextEdit` reports can-undo or can-redo.
- Existing [#535](https://github.com/anomalyco/rune/issues/535):
  A keystroke plus one frame on a field of at least 2,000 characters with one pill measured a median of 1581 µs in the committed run (samples 3632, 1577, 1581, 1586, 1581).
  Three other process medians were 1613, 1648, and 1662 µs; the first sample of each process was about 3.6–4.0 ms. This is a shared-machine observation, not a failure at this size.

## Integration

`Cargo.lock` gains one local package, `editor-spike`, depending on `kudzu-core`,
`rune`, `rune-kit`, `serde_json`, and `unicode-segmentation`. No new
third-party crate. The workspace already globs `spikes/*`, so the package is
a member once `spikes/editor/Cargo.toml` exists. That lock entry is in this
commit.

## What the owner decides

S52's accept says one click opens chip configuration. On this revision it
does not. Double-click selects the pill. Return or Space on that selection
opens a popover anchored at `Cx::caret`. Using either of those as the
product gesture is a change to the requirement, not something S52 should
quietly adopt. If the owner rejects `TextEdit` because of the click, the
fallback is the app-drawn editor in §5 risk 1, which this step did not start.

S23 should add `map` and the snippet-name callback. S76 should stay on D2
(a chip and a preview popover). The absolute bitmap overlay is not a reserved
range.

## Acceptance

- Every question answered: **pass** (table above).
- One-click chip configuration demonstrated, or a no-go: **pass**, as a no-go with the click counts above. The popover was not wired to double-click, Space-as-click, or an overlay.
- `EditorDoc` shape proposed: **pass** (section above).
- Comments on #128, #350, #353, #354, #535: **pass as proposed text**. Not posted. The orchestrator files them.
- `cargo fmt --all --check`: **pass**.
- `cargo clippy --workspace --all-targets -- -D warnings`: **pass**.
- `cargo test --workspace`: **pass**, 515 tests, including this spike's 11.
- `cargo xtask check-deps`: **pass**. The `editor-spike` lock entry is in this commit.

## Owner decision (2026-11-11)

D15: a chip's settings open on a double-click, or with Return or Space when the selection is exactly that chip, until Rune ships a pill press handler (#128). Then it becomes a single click. Everything else in this spike is a go and feeds S23 (the `EditorDoc` with a source map, above), S52, S53 and S54.

## Follow-up (S13b)

2026-11-11, same Rune rev `29aacb6`, same crate. A review found S13 had not proved D15 or chip
copy and paste. Everything below is headless, in `spikes/editor/src/followup.rs`, with a
`MemoryClipboard`. No real pasteboard, window or setting was touched. Numbers are in
`spikes/editor/evidence-s13b.json`; the helpers are `chips.rs`.

```bash
cd apps/desktop-v2
cargo test -p editor-spike -- --test-threads=1     # 32 tests: 11 from S13, 21 here
```

| Question | Verdict | Evidence |
|---|---|---|
| 1. Double-click on a chip | **fallback.** Rune exposes no click count, press or selection source for a text field. An inferred detector passes every case required, headless. It misses a nudged double-click and has not run in the real window host. Return and Space are the contract; the mouse path is best effort until S52 checks it on a device. | below |
| 2. Copy, cut and paste | **go**, with one gap: the field's own context menu Cut and Copy skip the hook. | below |
| 3. Loading is not undoable | **go** | `the_first_undo_on_an_untouched_document_does_nothing` |
| 4. Slash menu by keyboard | **go** | `slash_da_then_return_inserts_the_date_chip_with_no_mouse` |

### 1. Double-click on a chip

**What Rune exposes.** A press on a `text_input` goes straight to `Runtime::input_down` (`runtime/selection/click.rs`)
and `pointer_down` returns. The field's `on_press` and `on_double_press` never run, `Drag::clicks`
only reaches an element with `on_drag`, `release` returns at `if input_capture { return; }`,
and `El::on_selection_change` belongs to `selectable` text. There is no public hit test
from a point to a byte and no pill rect: `rune::text` only measures (`Cx::caret` gives the caret
line, not a chip). So the app cannot be told "this was a double-click on that chip".

What does reach the app, in order, for one gesture:

- `on_pointer_move_local` on the field. `Runtime::release` calls `move_pointer` with the
  release point, so it runs once on every mouse-up too, after the press's selection step.
  That is the only pointer hook after a click. `Tx::live_only()` keeps it from rebuilding the
  view: 200 moves cost 7 view runs.
- `on_input_key`, for every key the field gets, before the editor.
- The field's `TextEdit` (`anchor`, `focus`, `pills()`).

`ChipGesture` (`chips.rs`) infers the double-click from those. A double-click on a chip is the
approach move and two releases: three events at one point, and the second release is the
first to see the selection go from a caret to exactly the chip. A single click is two events.
A drag moves the point, and its selection change is seen on the event after the move that
caused it, so never on the third event at one point, however many the host repeats. Any key
since the last event hands the selection to the keyboard. Rune's window host sends
`pointer_move` only for `CursorMoved` and releases at the last point
(`window/events.rs`), which is the sequence the tests send.

**Results** (`double_click_opens_a_chip_at_both_edges_and_not_beside_it` and the tests under it).
The chip's clickable extent was measured by sweeping double-clicks along the line: x 59 to 200 for
the pill at bytes 6..22.

| Gesture | Selection | Opened |
|---|---|---|
| double-click at the first x of the chip (59) | 6..22 | yes |
| at the last x (200), and in the middle | 6..22 | yes |
| 4 pt before / after the chip (the spaces) | 5..6 / 22..23 | no |
| the word 20 pt before / after | 0..5 / 23..28 | no |
| word touching the chip (`Hello{{date}}there`) | 0..21, the word and the chip | no |
| single click, every 2 pt along the line | caret | 0 opened |
| drag from before the chip to after it | 2..26 | no |
| drag from the chip's left edge to its right, 3 pt and 1 pt steps | 6..22 | no |
| 2 pt drags across the point where the caret flips (x 121, 122) | 6..22 | no |
| Shift+Right from the chip's left edge, then pointer events at a resting point | 6..22 | no; Return then opens it |
| chip wrapped over two lines, field 130 pt wide: 4 pt grid over 120 x 76 pt | first line x 6..122, second line x 6..46 | every hit on the chip opened it; no hit beside it did |
| Return or Space with a caret only / with the chip selected | | no / yes (`opened_via` Return or Space) |

**Failure modes, tested as limits.**

- A double-click with a 1 pt nudge between the presses is still a double-click to Rune (it allows 4 pt) and selects the chip, but the detector sees a moved pointer and does not open it
  (`known_limit_a_double_click_with_a_nudge_selects_the_chip_but_is_missed`).
  A user hits this on a trackpad. The cost is a missed shortcut, not a wrong action.
- A programmatic selection change of the exact chip, with the pointer resting on it and no key, would look like a double-click. S13b has no such path. S52 must not add one without a key or a flag.
- The sequence was tested through `Runtime`, not the winit host. S52 must run it once in the real window before D15 is closed.

**If that check fails**, the smallest alternative is: Return or Space on a chip, plus a visible
"Configure" button next to the caret whenever the selection is exactly one chip or the caret
touches one. That needs no pointer inference. Do not ship the double-click path in place of it.

**Draft comment for [#128](https://github.com/anomalyco/rune/issues/128):**

> Kudzu's snippet editor needs to open a chip's settings when the user clicks or double-clicks it. At 29aacb6 a press on a `text_input` goes to `input_down` and nothing reaches the app: not `on_press` / `on_double_press`, not `Drag::clicks`, and not `on_selection_change` (that is for `selectable` text). A single click snaps the caret to a pill edge and never selects it; a double-click selects it (`unit_at(Word)`), but the app only sees `TextEdit.anchor/focus` change.
> We infer a double-click from `on_pointer_move_local` firing on each mouse-up plus a key flag from `on_input_key`. It works under `Runtime`, but it leans on event order that is not documented, and it misses a double-click with a 1 pt nudge.
> Would you accept either (a) `El::on_pill_press(|s, id, range, clicks, tx|)` (and a way to ask for the pill's rect), or (b) a `SelectionChange` for text fields that says whether a pointer press, a drag or a key made it, with the click count? (a) is all we need.

**Also for [#353](https://github.com/anomalyco/rune/issues/353):** an app cannot ask where a pill is. A `Cx::pill_rects(key)` (one rect per line fragment, as `Cx::selection_rects` does for `selectable` text) would let us anchor the popover on the chip rather than the caret, and would let us hit-test the chip ourselves.

### 2. Copy, cut and paste

**Rune today** (`rune_copies_the_chip_label_as_plain_text_today`; a rich field with no hooks, whole text selected):

- plain text: `Hello Date · ISO date there`. This is the chip label, so pasting into another app loses the tags.
- HTML: `<p>Hello <span data-pill="time:{{date}}">Date · ISO date</span> there</p>`.

Rune restores pills on paste only for its own copy: `clipboard.rs` keeps a keyed hash of the plain text and the pill list it wrote, and `Action::Paste` calls `paste_html_with_pills` when they still match and the clipboard has not changed. That flag (`ClipboardContent::has_own_pills`) is `pub(crate)`.

**The hooks** (`chips.rs`, `field.rs`):

- Copy and cut: `on_input_key` runs before the editor for Cmd+C and Cmd+X (`route_key`, `input_handles`). The handler calls `Tx::copy_rich(ClipboardContent::new(source).with_html(edit.selected_html()))` and returns true. For cut it then calls `edit.insert("")`, which is one undo step as Rune's own cut is.
- Paste: `on_input_paste` gets every form of the clipboard. `own_fragment` parses the HTML with `TextEdit::from_html`, accepts it only when every pill id carries one valid tag and the plain text is exactly the source those pills and the text between them make, and then calls `paste_html_with_pills(html, fragment.text)`. Anything else returns false and Rune pastes it as it always did.
- `source_of` builds the plain text. It writes each chip's tag and escapes the literal runs with `kudzu_core::template::serialize`, so `\` and `\{{` come out as the grammar says.
- **Chip ids now carry their source: `{tone}:{source}`** (`time:{{date}}`). Rune's HTML keeps only the id, so this is what lets chips survive a copy into another field or window, and it removes the side table. S23's `EditorPill.id` should be built this way.

**Results** (all `MemoryClipboard`):

| Case | Plain text | Result |
|---|---|---|
| whole text, `Hello {{date}} there` | `Hello {{date}} there` | HTML has `<span data-pill="time:{{date}}">` |
| the chip only | `{{date}}` | |
| selection starting inside the chip's label | `{{date}} th` | the selection grows over the chip first |
| `lo ` and the chip | `lo {{date}}` | |
| `a \{{ b \\ c {{date}} \{{d}}` (escapes) | the same bytes | `copy_escapes_literal_braces_and_backslashes_like_the_serializer` |
| bold `Hello` + chip | `Hello {{date}} there` | HTML has `<strong>Hello</strong>` and the chip |
| paste into another field/window | | `Say: Hello {{date}} there` with one chip, id `time:{{date}}`; one Cmd+Z removes all of it |
| literal-only copy, `a {{ b \ c` | `a \{{ b \\ c` | pastes back as the display text `a {{ b \ c`, not the escaped source |
| another app's HTML with its own pill id, or plain text that does not match | | pasted as Rune does: labels, no chips |
| plain `Hello {{date}}` from another app | | literal text; parsing it into chips is a product choice for S52 |
| cut `lo {{date}} t`, then Cmd+Z, then redo | `lo {{date}} t` | text and the chip (same range, same id) return; redo cuts again; the pasteboard is not touched |

**Gap: the field's context menu.** `field_menu_pick` (`runtime/input/field_menu.rs`) runs Cut and Copy with
`with_focused(|e| e.key(..))`, which does not go through `input.key`, so the hook never sees them.
A copy from that menu puts the label on the pasteboard. I read this in the source and did not run it (a native `PopupMenu` cannot be picked headlessly). Options for S52: accept it, or set the field's own `on_secondary_press`, which turns Rune's field menu off entirely (`queue_field_menu`) and costs the spell menu. I did not check whether the macOS Edit menu's Copy role (`native_menu/standard.rs`) arrives as Cmd+C; S52 must.

**Draft upstream comment** (new issue, or on #128):

> `El::on_input_paste` lets an app take a paste, but there is no matching hook for copy and cut. `on_input_key` sees Cmd+C and Cmd+X, but the field's context menu runs Cut and Copy through `with_focused(|e| e.key(..))` and skips it, so an app that writes its own pasteboard form (Kudzu writes the template source `{{date}}` as the plain text, and Rune's HTML beside it) is bypassed from the menu.
> Could `field_menu_pick` call `input.key` first, or could there be an `on_input_copy(|s, tx| -> Option<ClipboardContent>)` that both paths use? Also: `ClipboardContent::has_own_pills` is `pub(crate)`, so a paste hook cannot tell Rune's own copy from another app's. An accessor would let an app skip its own check.

### 3. Loading is not undoable

`load_edit` now ends with `edit.clear_history()` (public on `TextEdit`; it keeps text, marks and selection). Tests:

- `the_first_undo_on_an_untouched_document_does_nothing`: undo and redo on a freshly loaded document with a bold mark change nothing; the same with Cmd+Z through the runtime.
- After typing `!`, one Cmd+Z removes only that, and a second does nothing.
- `loading_without_clearing_history_lets_undo_strip_the_chips` keeps S13's behaviour as a control: the first undo removed the chip.

S52 must clear history after every load, including a snippet switch. `replace_text` also clears it but drops marks, so use `clear_history`.

### 4. Slash menu by keyboard

`PopupList` takes no keys ("the popup takes no keyboard focus"), so the field handles them in `on_input_key`: `slash_key` in `field.rs`.

- `/da` then Return inserts the Date chip: text `Say Date · ISO date`, id `time:{{date}}`, source `Say {{date}}`, no newline, no leftover query, menu closed.
- Up and Down move the row (and must be consumed, or the field moves its caret and the menu closes); Return and Tab pick the active row; Escape closes the menu until that `/` goes.
- When no row matches (`/zz`), Return is a newline.

### What S52 must do

1. Build chip ids as `{tone}:{source}`; put `chip_id`, `chip_source`, `source_of`, `copy_content` and `paste_own` in `kudzu-ui/src/main/editor/` (tag the double-click code `RUNE-GAP(#128)`).
2. Wire the field with `on_input_key` (call `ChipGesture::key`, copy/cut, slash keys, Return/Space), `on_input_paste`, and `on_pointer_move_local` with `Tx::live_only()` unless the detector fires. Return and Space are the contract; double-click is best effort.
3. Check the double-click path once in the real window (nudge, trackpad tap). If it misses, add the Configure button from section 1.
4. Clear history after every load.
5. Decide on the field context menu (accept the gap or take over `on_secondary_press`) and on parsing pasted `{{tags}}` from another app into chips.
6. Image pills carry no tag source, so copy and paste treat them as their alt text. S76 owns that.
