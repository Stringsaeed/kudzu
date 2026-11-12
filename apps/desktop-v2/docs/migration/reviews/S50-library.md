# S50 review: library list

This is an independent review by Grok 4.7 of the seven S50 commits, from `refactor(desktop-v2): add the library list model` to `refactor(desktop-v2): reset the library sort outside the library and drop native tooltips`. The orchestrator checked M1 against `main/library/listing.rs:51–92` before acting on it.

**Verdict:** fixes needed, with no blockers.
- `main::tests::library`: 34 tests pass.
- The `library` filter: 52 tests pass.

## Major

**M1. List motion sticks for the whole session** (`main/library/listing.rs:51–66`, `:85–88`; `view.rs:274`, `:282–284`; `row.rs:189–191`; Tauri `snippet-list.tsx:121`, `:251–264`, `:265`, `:524`).

**The cause:**
- `census` returns `still: false` for any change with at most 12 additions and removals, including a change of zero.
- Nothing ever clears it.
- The same `Motion` carries over to later queries and sorts on the same snapshot.
- `fresh` ids keep `transition(rise())` for as long as they stay in the set.

**What the user sees:**
- **After starring one snippet**, picking Sort → Name makes every row glide, and rows leaving the list play an exit. This lasts until a later change adds or removes more than 12 snippets.
- **After creating a snippet**, filtering it out and pressing Clear filter makes it rise a second time, because its key was dropped from the virtual list and it mounts again with `rise()`. The scroll remount test can't catch this, because scrolling keeps the keys.

Tauri starts motion only when `fresh.size + gone.size > 0` and clears it after 320 ms.

**Fix.** Follow `clip/motion.rs:70–76`:
- A change with no additions or removals stays `still`.
- A small change sets a deadline of about 320 ms. When it passes, clear `fresh` and set `still`, so the next sort, and the next mount of that row, land at once.
- Add tests for both scenarios above.

## Minors

1. **The row's content inset is 13 pt** (`row.rs:170`, `.px(13.0)`), against Tauri's `px-3`, 12 pt (`snippet-list.tsx:133`). Use 12, and assert the text inset.
2. **Bulk edits scan the library once per id** (`kudzu-app/.../library/snippets.rs:157–158` `iter_mut().find` per snippet; `bulk.rs:145–152` and `actions.rs:199` use `ids.contains` on a slice). This makes Favorite all, or ⌘⌫, on 5,000 rows O(n·k). Collect the ids into a `HashSet` first.
3. **Back and Forward skip `library::sync` for one frame** (`state.rs:331–349`, `nav.rs:142–156`; `navigate_to` does call it at `nav.rs:57`). ⌘[ from Favorites, with several rows selected, paints the old bulk bar for one frame. Call `library::sync` from `back` and `forward`.

## Implementer's notes, confirmed

- **Tests weren't written first.** The landmark, scroll and geometry tests came after the code.
- **Two items are unasserted:** the 8 pt bottom padding, and the summary text (only checked to be non-empty, `model_tests.rs:208`).
- **The breadcrumb x gate is a flat 3 pt** (`library_landmarks.rs:111–116`), looser than the parity note's "a point per crumb".
- **The 1.25 ms p95 scroll figure is the implementer's own,** measured under load. There's no release trace of `library-5000` on a quiet machine yet; that goes to the S71 live checks.

## What holds

- **Intentional differences.** Each one in the S50 parity notes is justified:
  - select on release;
  - dragging the whole multi-selection;
  - deleted ids leaving the selection;
  - Rune's exit animation;
  - no count tick;
  - `?` opening the cheat sheet;
  - no tooltips.
- **List contents.**
  - Scopes, the four sorts, unnamed snippets last, and the 10,000 cap match `snippet-list.tsx:191–218`.
  - A group includes its descendants.
  - `keep_valid` matches `snippet-list.tsx:269–274`.
- **Opening on the first snippet doesn't break routes.** `library::sync` selects only once, only when nothing is selected, and before `reconcile_route`.
- **Selection and deletion.**
  - A filter or a sort doesn't clear the multi-selection.
  - The bulk bar acts on the full selection, including hidden rows.
  - ⌘⌫, Undo and neighbour selection match `store.ts:217–233`.
- **Scrolling.**
  - The rows are memoised and virtual.
  - 600 wheel frames run the view 0 times.
  - The search index is rebuilt only when the library `Arc` changes.
- **Empty states and dragging.** The empty states match `EmptyList`. The drag payload is snapshotted on pointer-down, and the release after a drag is swallowed.
- **Hygiene.**
  - Logs contain only error values.
  - No `#[allow]` and no dependency changes.
  - Edits outside the step stay in scope.

## S71 live checks

1. Star one snippet, then change the sort: rows snap into place.
2. Create a snippet, filter it out, then press Clear filter: it doesn't rise again.
3. Quick Search ⌘N, and New Snippet from Selection, open the new snippet with its name focused.
4. Opening a snippet far down a large library from Quick Search selects it and scrolls it into view.
5. ⌘-click several rows, press ⌘⌫, then Undo: the first deleted snippet, in library order, comes back selected, and the bulk bar stays gone.
6. Drag a row in the multi-selection, then a row outside it, releasing inside the row: the selection doesn't change.
7. Run the release `library-5000` scroll trace on a quiet machine: frame CPU p95 is at most 4 ms, with 0 view commits.
8. Press `?` with the list focused: the cheat sheet opens and the filter stays empty.
9. After a failed save or restore, the log holds only the error sentence, with no snippet text.
