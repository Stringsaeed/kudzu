# S51 review: editor shell

An independent review by Grok 4.7 of the S51 commits, from `refactor(desktop-v2): add the snippet editor shell` to `refactor(desktop-v2): move the editor's app picker to the S55 API`.

**Verdict:** fixes needed. There are no blockers. `cargo test -p kudzu-ui editor` gives 75 passed and 0 failed.

## Majors

**M1. Deleting the last snippet leaves the editor open on it** (`main/nav.rs:25–28`, `:61–63`; `main/library/actions.rs:163–166`; `main/editor/view.rs:114–116`; `main/editor/mod.rs:122–123`; Tauri `lib/store.ts:222–224`, `components/app/main/main-app.tsx:201`).

**The bug:**
- `refresh_library_snapshot` clears a missing selection, but doesn't touch the editor.
- `select_snippet` returns early when the frame selection already equals the requested id. After the last delete, that id is `None`, so `open_selected` never runs.
- Typing then fails with "That snippet no longer exists."
- Undo restores the flushed copy, but the later dirty text stays and overwrites it on the next flush.

**Why the tests miss it:** the existing test uses two snippets.

**Fix:**
- `select_snippet` calls `open_selected` whenever `editor.open_id()` differs from the requested id.
- Test deleting the only snippet: "No snippet selected" appears, nothing types, and Undo restores the flushed text unchanged.

**M2. The Created date, and relative dates older than 30 days, are UTC** (`main/editor/view.rs:313–320`; `kudzu-core/src/fmt.rs:41–61`; compare `main/insights/data.rs:50–54`; Tauri `components/app/editor/snippet-editor.tsx:493`, `lib/stats.ts:77`).
- **The bug:** a snippet created at 8 pm US Pacific shows the next day.
- **Fix:**
  - Use the offset Insights uses: native `Local` minutes, otherwise the injected offset.
  - Keep `Local` out of `kudzu-core::fmt`.
  - Assert the Created line with a non-zero offset.

## Minors

1. **A spaces-only paste refuses a token already in the field** (`main/editor/triggers.rs:301–312`; Tauri `trigger-input.tsx:169`, `:172–177`). Tauri commits it.
   - **Fix:** show "A trigger can't be only spaces." only when both the field and the clipboard are whitespace.
2. **An uppercase-fold duplicate shakes but doesn't pulse** (`main/editor/triggers.rs:120–128` against `kudzu-core/src/library/triggers.rs:56–62`).
   - **Fix:** point the pulse at the match the validator found.
3. **`RUNE-GAP(icon fill)` has no issue number** (`main/editor/header.rs:7`; rule 6).

## The implementer's open decisions, with the reviewer's recommendations

1. **Whitespace-only paste.** Show the message only when the field and the clipboard are both whitespace (minor 1).
2. **Tab during IME.** Keep swallowing it. A Tab blur would commit a half-composed trigger. Check a real IME in S71.
3. **Flush before a delete, and before ⌘⇧F / ⌘⇧E.** Keep it; without it, Undo restores the snippet as it was before the edit. The bulk bar matches Tauri. Leave it as is.
4. **Dates.** Match Insights (M2).
5. **`MainModel.draft`.** Leave it. Its removal belongs to the frame, per `S40_HANDOFF.md:25`.

## What holds

- **Autosave.** It matches the plan's deliberate difference from Tauri:
  - The draft updates at once and is written after 350 ms.
  - It flushes on blur, on navigation, through `commit_drafts` (hide, close, quit), and on ⌘S.
  - "Saved" waits until `library.saved().durable` reaches the draft's revision.
  - "Couldn't save" survives navigation and hiding.
  - An outside change is taken only while the draft is clean, and a clean flush doesn't bump the revision.
  - An edit made during a pending save stays "Saving…".
- **Name and triggers.**
  - Return in the name field moves to the triggers, and does nothing during IME.
  - New and duplicate focus the name.
  - The prefix is suggested, and dropped if nothing else is typed.
  - An atomic whitespace paste, the commit keys, Backspace and Escape all behave as specified.
  - The four validator sentences match.
  - Conflicts show in amber.
- **Behaviour.** The copy and the inherited-source sentences match. `with_mode` follows S55's M2 rule. The snippet screen in Tauri keeps `insertion` across a mode change, which is recorded for S71.
- **Preview.** 120 ms debounce, stale answers dropped, a cue after 600 ms, and all five badges, including "Uses clipboard".
- **Banners and the header.** Collision banners and their selection, the off banner, the no-trigger note, and the favorite burst only when turning on, which respects reduced motion.
- **Landmark shift.** The 78 pt shift is sound:
  - The reference Content height is 276, and the stand-in is 198.
  - Content's x, y and width are asserted, and its height is asserted exactly.
  - The shift goes away when S52 and S54 land.
- **The S52 seam.** `editor/content.rs` (121 lines) is enough for S52 to replace on its own.
- **Rules 13 and 14.**
  - Waits are on watches.
  - Copy is asserted for every state.
  - Motion expires.
  - The pane is tested at 900 × 560.
  - IME is checked in every field.
  - Domain sentences reach the UI.
  - There is no logging and no `#[allow]`.
  - Every file is under 600 lines.

## S71 live checks

- **Deleting snippets.**
  - Delete the only snippet: the empty state appears, focus moves to the list, and typing does nothing. Undo restores the flushed text.
  - With two snippets, delete the open one: the neighbour opens.
- **Dates in a US time zone.** Created in the evening, and "Edited" and "Last used" older than 30 days.
- **Japanese IME.** In the name field and the trigger field: Return, Tab and Escape.
- **Pasting spaces.** Into an empty trigger field, and after a token.
- **The Favorite burst.** Compare the stroke-only star with Tauri's filled one.
- **Insertion across a mode change.** On the snippet screen, compare the insertion setting with Tauri's.
- **A read-only data folder.** "Couldn't save" survives switching snippets and hiding the window, then clears.
- **VoiceOver.** "Saved" is not announced; "Saving…" and "Couldn't save" are.
