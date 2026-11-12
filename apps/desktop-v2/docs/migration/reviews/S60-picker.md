# S60 review: clipboard picker window

This is an independent review by Codex Sol of `418278dc refactor(desktop-v2): add the clipboard picker window`. The orchestrator checked B2 against `kudzu-app/src/clipboard/pick.rs:43–48`, which captures `frontmost()` without filtering this process, and against Tauri's `src-tauri/src/windows.rs:242`, which filters its own pid.

**Verdict:** fixes needed, including **two blockers in paste safety**. The picker tests pass (67 passed). `parity --window picker` reports 60 rows, 0 failed.

## Blockers

**B1. A second pick can change what the pending paste inserts.**
- **Code:** `crates/kudzu-ui/src/picker/drive.rs:217`, `:502`; `shell/windows/picker.rs:110`; `crates/kudzu-app/src/clipboard/pick.rs:87`; `tasks.rs:25`.
- **Failure:** pick A, reopen, and pick B while A waits for its target to activate. Reopening drops A's `Task` handle, but that doesn't cancel the worker. B overwrites the pasteboard before A posts ⌘V, so A inserts B's content. Several copy jobs within one opening aren't coordinated either.
- **Fix:**
  - Run each copy-through-paste as one serialized operation, owned across reopenings.
  - Queue or reject another pick until it finishes.
  - **Test:** block activation, start a second pick, then check the content each paste delivers.

**B2. The picker can paste into Kudzu itself.**
- **Code:** `kudzu-app/src/clipboard/pick.rs:43–48`, `:91`; `shell/windows/picker.rs:113`.
- **Tauri:** `src-tauri/src/windows.rs:242` filters its own pid.
- **Failure:** open the picker while editing a snippet in Kudzu. The session keeps Kudzu as the target and pastes into it.
- **Fix:**
  - Reject this process as a target.
  - Fall back to copy-only when there is no safe external target.
  - Validate the destination before posting.
  - **Tests:** a self-target and an absent target.
- **Note:** this is a clipboard-service gap from S28, exposed by S60.

## Majors

**M3. Paste failures, and a permission fallback that comes late, are lost after the picker hides.**
- **Code:** `picker/drive.rs:367`, `:418`, `:508`.
- **Tauri:** `components/app/clipboard/clipboard-picker.tsx:107`; `src-tauri/src/clipboard/mod.rs:282`.
- **Failure:** `finish_jobs` runs only while the picker is open, so a target that quits never reshows the error. `PasteOutcome::Copied` is ignored, so losing permission after opening never shows the fallback.
- **Fix:**
  - Consume work while hidden, and handle both outcomes.
  - Keep stale completions away from a newer opening.
  - **Tests:** the target disappearing, and permission loss after capture.

**M4. The preview's file actions are wired wrong.**
- **Code:** `picker/view.rs:166–167`; `picker/drive.rs:335–347`.
- **Tauri:** `clip-content.tsx:259`, `:267`.
- **Failure:** Open on the second file opens the first, because the index is dropped and the action uses `files.first()`. Show in Finder does nothing.
- **Fix:**
  - Keep the index, and run both actions through the file port.
  - **Tests:** the second file, and failure feedback.

**M5. The comparator change can hide unrelated colour regressions.**
- **Code:** `xtask/src/parity/compare.rs:197–208`; `tree.rs:139–144`.
- **Failure:** the exception covers every `complementary` landmark, sidebars included. A single descendant with the expected RGB passes the container, and copying the expected alpha lets zero-opacity text pass.
- **Fix:**
  - Measure a designated text sample, or an explicit inherited colour, including its opacity.
  - Narrow the exception.
  - **Negative tests:** an unrelated matching descendant, invisible text, and a sidebar regression.

**M6. Keyboard focus doesn't reveal a concealed preview.**
- **Code:** `picker/view.rs:176`, `:200`; `picker/drive.rs:362`.
- **Tauri:** `secret.tsx:53–64`.
- **Problem:** the reveal comes only from `cover_hover`, although the S60 parity notes claim keyboard focus too.
- **Fix:**
  - Read keyboard focus as the main clipboard screen does.
  - **Tests:** focus reveals the content, and it stays readable and selectable; losing focus covers it again.

## Minors

7. **Some tests don't prove what they claim.**
   - The opening test focuses search itself (`picker/tests/mod.rs:164–172`).
   - The hide-before-paste test checks the hide only after the paste (`flow.rs:63–67`).
   - The pill test derives "checked" from the model (`rows.rs:38–42`).
   - The search-field width is allowed 4 pt (`landmarks.rs:31–33`).

   **Fix:** drop the focus repair, record visibility when the paste is called, assert the radio's checked property, and keep structural widths to 2 pt.
8. **⌘Q is handled before the composition check** (`picker/drive.rs:83–89`). **Fix:** check composition first.

## What holds

| Area | Evidence |
|---|---|
| Recording off | `picker/tests/flow.rs:11` |
| Reset on opening | `flow.rs:34` |
| Ordering and sections | `rows.rs:22`; `flow.rs:221` |
| Pills | `rows.rs:35` |
| Key navigation | `rows.rs:79` |
| Paste and pick keys | `flow.rs:63`, `:90`, `:128`, `:155` |
| Copy-only and no-permission timers | `timers.rs:32`, `:56` |
| Pin, snippet, open and settings | `flow.rs:221`, `:278`, `:296`, `:316`, `:336` |
| Escape, delete and Undo | `flow.rs:246`, `:269`, `:347` |
| Footer expiry | `timers.rs:11–99` |
| Four empty states | `rows.rs:119–148` |
| Dimensions and landmarks in both themes | `rows.rs:154`; `landmarks.rs:39` |

- **Panel:** a windowless launcher panel with radius 18. Its show is deferred, and a reset cancels a pending close.
- **Loader extraction:** it preserves the request logic, and `Look.keep_focus` defaults to false.
- **Ports and copy:** ports come from `Kudzu`, and copy-back uses own-write tracking.
- **Logs:** they hold classified errors only.

## S71 live checks

- **Hotkey latency:** 100 real hotkey opens, timed from `shortcut_received` to `panel_frame`, to investigate S11's key stalls.
- **Stale frame:** history changed while the picker is hidden; check its first visible frame.
- **Window behaviour:**
  - blur dismissal, the first mouse event, search focus and focus restoration;
  - Spaces, fullscreen and display changes, including mixed scales.
- **After the safety fixes:** overlapping picks, a target that disappears, permission loss, and opening from Kudzu's own editor.
- **Pasteboard:** rich and plain flavours, and no recaptured history entry.
- **Visual comparison with Tauri:** material, shadows, clipping, motion, and concealed-preview accessibility.
- **Performance:** release measurements, with the machine load recorded.
