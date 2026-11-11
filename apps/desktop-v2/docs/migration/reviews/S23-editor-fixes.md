# S23 editor document review fixes

This follow-up is local to `t3/fix-editor-doc-findings` in worktree
`/Users/saeed/.t3/worktrees/text-expander/t3-b4f68753-846e0af88de78ca4`.
It changes only `apps/desktop-v2`. No dependencies were added.

## Findings and test evidence

Tests below are in `crates/kudzu-core/tests/template_editor_review.rs` unless another file is named.
Each runtime regression listed as **red → green** was run and observed failing before its fix.
New API tests first failed compilation against the previous public interface. No test failure
is claimed for behavior that was already correct.

| Finding | Evidence |
|---|---|
| B1: real edit mapping | **red → green:** `deleting_the_first_repeated_character_keeps_the_second_bold`, `repair_keeps_bold_on_the_edited_word`, `pasting_repeated_text_stamps_marks_at_the_insertion`. The `abc·abc` example also has a passing regression; that particular deletion already passed on this checkout. No source diff remains. Spans pass through display coordinates, with saved interior endpoints retained when their source cell is untouched. |
| B2: literal caret positions | **red → green:** `escaped_braces_allow_an_insert_between_them`, `escaped_braces_allow_one_brace_to_be_deleted`, `scalar_boundaries_inside_combining_zwj_and_crlf_are_editable`. `replace_reports_the_applied_display_range` initially failed compilation because the old result was a bool. Only pills are atomic. |
| M1: Plain/Formatted | **red → green:** `formatted_mode_survives_empty_spans_on_load`. New API: `formatted_toggle_is_one_undo_step_and_clears_spans`. Mode is part of undo snapshots. |
| M2: formatting source equality | **red → green:** `empty_formatting_source_does_not_match_nonempty_content`. Mismatched formatting leaves the document Plain. |
| M3: bounded history | **red → green:** `doc::history::review_tests::two_thousand_edits_share_512_kib_of_images_and_bound_history`. It performs 2,000 edits with 524,286 bytes of image URLs, checks unique retained image allocations stay below 600 KiB, document/history string and step buffers below 16 MiB, and exactly 200 undo/redo steps. This is deterministic retained-buffer accounting, not a process RSS measurement. Data URLs are `Arc<str>`. Snapshots remain simpler than piece-level diffs; their count is bounded and dominant image allocations are shared. |
| M4: duplicate HTML ids | **red → green:** `duplicate_html_identity_does_not_change_when_an_earlier_chip_is_added`. IDs are exactly `{tone}:{source}`; no suffix. `PillId` remains unique within the document. |
| M5: missing chip operations | New API tests first failed compilation: `slash_replacement_and_configuration_preserve_chip_identity_and_marks`, `relabel_keeps_source_marks_identity_and_undo_history`. Replacement/configuration each form one undo step. Relabel refreshes current and historical labels without changing source or creating history. |
| M6: local repair | **red → green:** `escape_repair_does_not_rewrite_a_distant_windows_path`. Repair retains complete escape/grapheme cells and re-encodes the smallest conflicting suffix. Typed text keeps its display positions. |
| M7: property tests | `template_editor_properties::seeded_display_source_and_untouched_marks_survive_edit_sequences`: 4,096 cases × 32 operations, seed `0x5323_B1B2_2026`. Independent display-string edits; preserved source outside the affected escape neighborhood; untouched mark ranges; random formatting; combining scalars, ZWJ emoji, CRLF, braces and backslashes; plain/own paste, lists, undo and redo mid-sequence; exact save/reload. This stronger test failed during development and exposed the additional regressions below. |
| M8: realistic summaries | `search_perf::ranking_long_summaries_stays_within_the_keystroke_budget`: 5,000 distinct seeded English/mixed-script bodies near 2,000 units, real summarizer, 560 samples, including queries whose letters are present even when the whole term is absent. The diversity assertion failed against the old repeated-`a` corpus. The realistic release workload initially failed at **2.852 ms p95**. First-occurrence UTF-16 pair indexes (dense Latin pairs and sorted other pairs) accelerate matching queries without changing first-match scores. Search parity and Unicode/split-surrogate tests cover the optimization. |
| Rich-paste tone validation | **red → green:** `paste_rejects_a_forged_tone_even_when_the_metadata_agrees`. The parsed tag's tone must match both metadata and the HTML id. |
| Rune label normalization | **red → green:** `labels_have_the_same_control_normalization_as_rune`, `imported_image_labels_are_normalized_and_clearing_restores_literal_source`. Controls, NEL, LS and PS become spaces in pill labels; original source is retained. |
| Alt cleanup order | **red → green:** `image_alt_trims_before_replacing_braces`. Trim first, replace braces/line endings second, then apply the empty fallback and length limit. |
| Unsafe link | **red → green:** `unsafe_link_change_preserves_the_existing_link_and_history`. Reject before any mutation. |
| Clearing unaligned images | **red → green:** `clearing_middle_of_unaligned_image_drops_the_whole_image`. Any overlap drops that image span; it never duplicates image data into two halves. Undo restores it. |
| Span ordering in equality | Already correct: `doc::history::review_tests::span_equality_includes_cross_style_order` passed before changes and remains a regression test. The existing equality compares order numbers. |
| Lists and chips | **red → green:** `lists_do_not_insert_inside_a_multiline_image_chip`. Line starts inside pills are skipped. Normalized imported labels also match Rune's display. |
| Stale search comment | Corrected `ts_parity_search.rs` to describe the actual real-summarizer path; documentation-only, no fabricated red test. |
| Rune fragment adapter and literal typing | New API test first failed compilation: `fragment_adapter_validates_tags_tones_and_plain_text`. The adapter validates ordered ranges, tag sources, tones and exact plain text. Typing `{{date}}` remains escaped literal text; source paste deliberately creates a chip. |

The generator and follow-up probes also produced **red → green** tests for:

- `combining_text_after_a_chip_survives_save_and_reload`: a closing brace is separated from
  trailing combining text in the parser. The opening-delimiter behavior remains unchanged.
- `lists_include_the_last_selected_line_ending_at_a_newline`: a newline belongs to its
  preceding line, including CRLF; a trailing empty unselected line is not numbered.
- `an_empty_edit_inside_a_grapheme_does_not_create_history`: changes to internal piece
  segmentation alone are not user edits.
- `a_literal_backslash_and_brace_before_a_chip_do_not_escape_the_chip`.
- `copying_one_escaped_brace_keeps_its_mark`.
- `pasting_bold_brace_beside_a_brace_marks_only_the_paste`.
- `pasted_partial_chip_spans_keep_their_source_endpoints`.
- `copying_marks_after_a_named_chip_uses_the_copied_label_width`.
- `snippet_names_can_borrow_the_live_library` first failed compilation because the old
  callback signature required captured names to be static. The shared lookup alias now ties
  returned names to the callback borrow, including summaries.

The existing S23 fixtures and round-trip corpus remain. Assertions that specified the rejected
`#n` id convention, grapheme-only coordinate mapping, or trim-after-replace alt rule now assert
the requested contracts instead. The captured TypeScript fixture files were not changed.

## Public editor API

All edit offsets are UTF-8 display bytes. Persisted spans use template UTF-16 units.
`Names` below means `chips::SnippetNames<'_>`: `&'a dyn Fn(&str) -> Option<&'a str>`.
Names can borrow the live library; the document does not retain that borrow.

```rust,ignore
EditorDoc::from_template(source: &str, formatting: Option<&Formatting>) -> EditorDoc
EditorDoc::from_template_named(source: &str, formatting: Option<&Formatting>, names: Names) -> EditorDoc

to_template(&self) -> (String, Option<Formatting>)
text(&self) -> &str
pills(&self) -> &[EditorPill]
marks(&self) -> &[EditorMark]
images(&self) -> &[EditorImage]
pieces(&self) -> &[SourcePiece]
formatted(&self) -> bool
html_id(&self, id: PillId) -> Option<String>
display_to_source(&self, byte: usize, end: bool) -> usize
source_to_display(&self, unit: usize, end: bool) -> usize

replace(&mut self, start: usize, end: usize, text: &str) -> Range<usize>
insert_chip(&mut self, at: usize, source: &str, names: Names) -> Option<PillId>
replace_with_chip(&mut self, range: Range<usize>, source: &str, names: Names) -> Option<PillId>
set_chip_source(&mut self, id: PillId, source: &str, names: Names) -> bool
relabel(&mut self, names: Names)
set_formatted(&mut self, formatted: bool) -> bool
change_format(&mut self, start: usize, end: usize, change: FormatChange) -> bool
toggle_list(&mut self, start: usize, end: usize, ordered: bool) -> bool
insert_image(&mut self, start: usize, end: usize, data_url: &str, width: u32, alt: &str) -> Result<PillId, ImageError>
set_image_alt(&mut self, id: PillId, alt: &str) -> Result<(), ImageError>
set_image_width(&mut self, id: PillId, width: u32) -> Result<(), ImageError>
copy(&self, start: usize, end: usize) -> Option<CopyPayload>
paste_own(&mut self, start: usize, end: usize, payload: &CopyPayload, names: Names) -> bool
paste_source(&mut self, start: usize, end: usize, source: &str, names: Names) -> bool
can_undo(&self) -> bool
can_redo(&self) -> bool
undo(&mut self) -> bool
redo(&mut self) -> bool

CopyPayload::from_fragment(plain: &str, pills: &[(Range<usize>, String)], display: &str) -> Option<CopyPayload>
PillId::get(self) -> u64
```

`replace` returns the inserted range in the new display, or a collapsed caret range for deletion.
Chip changes validate one exact tag before mutating. Style/image insertion or rich paste with
spans enables Formatted as part of that operation. Invalid links do nothing. Empty selections
do not change styles. `set_formatted` is a no-op when the requested mode is already active.
`EditorImage::data_url` and `CopiedImage::data_url` are `Arc<str>`; saved `Formatting` still
contains the original JSON-compatible `String` data URLs.

## Integration calls

**S52:** load with `from_template_named`, build Rune text/pills/marks from the display accessors,
and call Rune `clear_history` after every load. Route the actual Rune replacement range to
`replace`; never reconstruct the edit by diffing repeated text. Use its returned range for the
caret. Use `replace_with_chip(slash_start..query_end, source, names)` for the slash menu and
`insert_chip` for insertion at a caret. Map Rune pill marks by document order to `PillId` plus
range/order: duplicate HTML ids are expected. Save only through `to_template`. Handle undo/redo
through one authoritative history and rebuild the other representation, rather than applying
both histories independently. Refresh renamed nested snippets with `relabel`.

For keyboard copy/cut, use `copy` and `Tx::copy_rich`, with source as plain text; cut via `replace`.
For own rich paste, parse HTML with Rune, pass ordered pill ranges/ids and fragment text to
`CopyPayload::from_fragment`, then `paste_own`. The adapter does not invent rich marks from HTML;
S54 must map Rune's supported formatting marks through the fragment's source map into
`CopiedMark`s. Plain external template source goes to `paste_source`. Ordinary typing always
goes to `replace`, including typed `{{date}}`. Keep D15's real-window gesture gate, keyboard
Return/Space configuration, and the documented native context-menu copy gap.

**S53:** call `set_chip_source(id, source, names)` to configure a tag without changing its
`PillId` or losing covering marks. Remove it with `replace(pill.range.start, pill.range.end, "")`.
Use `relabel` for library name changes. Keep tag popover behavior and focus ownership in the UI.

**S54:** drive Plain/Formatted with `formatted` and `set_formatted`; apply styles/links/clear
with `change_format`, lists with `toggle_list`, and compacted images with `insert_image`,
`set_image_alt` and `set_image_width`. Consult `can_undo`/`can_redo`. The UI still blocks style,
list and image commands during composition, validates asynchronous image requests, owns the
D2 preview popover, and performs its own native formatted-paste gate. Core tests do not prove
Rune UI, native pasteboard, focus, IME host behavior or visual parity.

## Final verification

All commands ran sequentially in the foreground from `apps/desktop-v2`, using this worktree's
own `target` directory, initially cloned with the requested APFS `cp -cR` command.

| Command | Result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --workspace` | Pass: 1,125 tests, 0 failures, 6 existing ignored tests, 57 suites |
| `cargo xtask check-deps` | Pass: dependency and containment checks |
| `cargo test --release -p kudzu-core --test search_perf --test ts_parity_search -- --nocapture` | Pass: long-summary p95 **1.649 ms** over 560 samples; original-library p95 **0.317 ms** over 540 samples; both ranking-parity tests pass |

The six ignored workspace tests are two clipboard performance measurements, three isolated
child-process helpers, and the explicit temporary-bundle Launch Services smoke. None of the
new editor regressions, property sequences, history-memory checks or search-budget tests is
ignored. The existing editor fixtures and both source round-trip generators pass.

Local command output is retained under `target/s23-*.log`; the final gate logs are
`s23-fmt.log`, `s23-clippy-final.log`, `s23-workspace-final.log`, `s23-deps-final.log` and
`s23-release-final.log`. These are build artifacts, not committed fixtures.
