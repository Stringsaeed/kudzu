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

`load_edit` is `TextEdit::new(&doc.text).rich().allow_marks([Bold, Italic, Underline, Link("")]).allow_blocks(empty iterator)`, then one `undo_group` of `replace_with_pill` (labels already match `text`) and `set_mark`. That load is itself one undo step.

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
