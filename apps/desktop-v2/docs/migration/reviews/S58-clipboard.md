# S58 review: clipboard history list

This is an independent review by Grok 4.7 of the six S58 commits, from `refactor(desktop-v2): let scenes seed the clipboard history and keep a deleted batch for Undo` to `refactor(desktop-v2): focus the list on a row press, test highlights, record the S58 parity notes`. The orchestrator checked M1 against `main/clipboard/actions.rs:198–210` and Tauri's `src-tauri/src/clipboard/mod.rs:366–368`.

**Verdict:** fixes needed. There are no blockers. The 31 clipboard tests pass in a debug build.

## Majors

**M1. ⌘O doesn't open a `www.` link or an `ftp://` link** (`main/clipboard/actions.rs:203–207`; Tauri `src-tauri/src/clipboard/mod.rs:366–368`; classification `kudzu-core/src/clipboard/classify.rs:224–239`).
- **Problem:**
  - `open_active` passes the text to `Tx::open_url` as it is, and Rune opens only `http`, `https` and `mailto`.
  - So `www.example.com` shows "Couldn't open it"; Tauri adds `https://` first.
  - An `ftp://` clip counts as a link, and Tauri opens it, but Rune refuses the scheme.
- **Fix:**
  - Add `https://` to text starting with `www.` (ASCII, case-insensitive).
  - Open `ftp://` through the file/URL opener port, or show a specific message without pretending the row isn't a link.
- **Test:** check `Runtime::take_url_requests` for `www.example.com`.

**M2. Space turns on an invisible mode that swallows the list** (`main/clipboard/keys.rs:17–18`, `:80–84`, the trap at `:99–107`; `view.rs:302–305`; Tauri `clipboard/clipboard-view.tsx:313–315`, `:602–617`).
- **Problem:**
  - Space sets `large_preview`, but nothing is drawn, because the preview is S59's.
  - After that, the trap returns before `chord` runs, so typing, ⌘C, ⌘P, ⌘O, ⌘S, ⌘R and the delete keys all do nothing until Escape.
- **Fix:**
  - Consume Space.
  - Enter the trap only once the large preview is actually in the tree.
  - Run `chord` before the trap, so the clipboard chords still work behind the preview, as they do behind Tauri's dialog.

**M3. The polite announcement never reaches the tree** (`actions.rs:68–70`, `:118–123`, `:363`; `main/toasts.rs:326–329`; Tauri `clipboard-view.tsx:238`, `:657–659`).
- **Problem:** `say` writes `State.announcement`, but nothing reads it. So "Pinned. It won't expire." is never spoken. This is the announcement half of CLP-11.
- **Fix:** add one visually hidden `role("status")` element showing `announcement`, and clear it the way Tauri's live region does.

## Minors

1. **`?` on the list types into search instead of opening the cheat sheet** (`keys.rs:88–91`; Tauri `clipboard-view.tsx:331–333`, `use-hotkeys.ts:23–31`).
   - **Fix:** when `is_bare_shortcuts_question` matches, return false so `MainAction::OpenShortcuts` runs.
2. **Neighbour rows don't glide** (`clip/list.rs:314–317`; `Census::settling` is never read; Tauri `clip-list.tsx:224`, `motion.ts:33–54`).
   - **Fix:** while `settling(now)` is true, give the rows that only moved the short transform transition. If Rune can't do that on virtual rows, record it in the S58 parity notes.
3. **A concealed row's subtitle says "hover to show",** but nothing reveals on hover yet (`clip/list.rs:198–201`).
   - **Fix:** drop the phrase until S59 adds the hover reveal.
4. **An empty kind folder shows the clipboard glyph** (`main/clipboard/view.rs:202–207`; Tauri `clipboard-view.tsx:342` uses `KIND_ICON[filter]`).
   - **Fix:** use the kind's glyph.
5. **Every landmark width gets 4 pt** (`main/tests/clipboard_landmarks.rs:88–92`).
   - **Fix:** use 2 pt for structural widths (the column, the search field, the footer), and 4 pt only for text-run widths.
6. **`clip/filters.rs` imports main-window state** (`clip/filters.rs:8` uses `crate::main::state::{clip_filter_label, CLIP_FOLDERS}`), which breaks `clip/`'s promise to S60.
   - **Fix:** move the folder order and labels next to `ClipFilter` in `kudzu-core`. The sidebar and `clip/filters.rs` then both call it there, and `main/state.rs` re-exports it.
7. **Off-screen headers are estimated at 58 pt** (`clip/list.rs:390–392`), not their real 30.
   - A jump to a far row lands about 28 pt high for each header not yet measured.
   - **Fix:** note it. One estimate is all the API takes.

## Gaps the implementer listed, confirmed

These are missing proof, not extra product bugs (except ⌘O, which is M1).
- **Untested keys and actions:** ⌘O and ⌘R, a real double-click, Clear history with the box ticked, the loading column, and the options-menu rows other than retention.
- **Clear history test:** `clear_history_can_also_delete_pinned_items` only tests the default, where pinned items survive.
- **Column clamp:** the 300 and 372 pt ends are untested.
- **Arrival:** no test asserts that a row actually carries the arrival transition.
- **Shift-Enter:** `shift_enter_flips_plain_and_formatted_for_rich_clips` doesn't read the pasteboard flavour.

## Performance

- **Scroll test.** It asserts 0 view runs and p95 ≤ 4 ms in release. The reviewer ran it in debug only.
  - The reported **1.12 ms isn't recorded** in any test output or doc.
  - The orchestrator is running the release test, `cargo test --release -p kudzu-ui scrolling_five_thousand_clips_runs_no_view_and_stays_inside_the_frame_budget`, and will record the number here.
- **Inline search.** It is the same `ClipboardService::query` that `empty_search_5000_entries_p95_within_five_milliseconds` holds to 5 ms p95, in release.
  - The first query after a history change still builds the index on the UI thread. That is the documented choice.

## What holds

- **Sections:** Pinned comes first only with no search and outside the Pinned folder.
- **Keys:** every tested key does what Tauri does. ⌘⌫ deletes clips, not the open snippet.
- **Copying back:** our own write is not captured again. `Pasteboard::is_own` handles this, not a UI special case.
- **Bulk delete:** it shows a count with a 6 s Undo. `DeletedClips` restores the removed values. Restoring twice does nothing.
- **Secrets:**
  - Never named in toasts, spoken labels or `Debug` output.
  - Errors log no bodies or paths.
- **Dialogs and empty states:** the Clear dialog, the welcome, recording-off, paused and no-match states, and the 36 pt footer match.
- **Snapshots:** `ClipResults` indices always travel with their own `Arc<History>`.
- **Edits outside the step:** limited to the named ones.
- **Early key handler:** it is correct apart from `?`. Unmatched ⌘ chords still reach `MainAction`.

## S71 live checks

- **⌘O and ⌘R:**
  - ⌘O on `www.`, `https://`, `mailto:` and `ftp://` links.
  - ⌘R and ⌘O on a real file and a missing file.
  - No path or URL in the toasts.
- **Space:**
  - Before S59: Space, then a letter, lands in search.
  - After S59: the preview shows, Space and Escape close it, the arrows move, and ⌘P and ⌘C still work.
- **VoiceOver:**
  - A concealed row speaks no title or body.
  - Pin, copy and "Cleared N items" are spoken.
  - Deleting a secret speaks "Deleted a secret".
- **Pasteboard:** copy a rich clip as plain and as original, and a large image. No new row appears, and the pasteboard flavour matches the Shift-Enter choice.
- **List actions:**
  - Double-click a row.
  - Clear history with "Also delete pinned items" ticked. Pinned rows go, and there is no Undo.
- **`?` with the list focused:** compare side by side with Tauri.
- **Speed:**
  - Write down the release scroll trace of the 5,000-clip scene against 4 ms.
  - Time one keystroke over 5,000 varied clips.
