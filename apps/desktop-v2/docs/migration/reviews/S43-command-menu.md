# S43 review: command menu and cheat sheet

This is an independent review by Codex Sol of `d04b41d8 refactor(desktop-v2): add the command menu and shortcut cheat sheet`. The orchestrator checked M1 against `actions/main.rs:41–61` and Tauri's `command-menu.tsx:92–149` before acting on it.

**Verdict:** fixes needed, with no blockers. The 9 command-menu tests and the 3 existing shortcut tests pass.

Rust paths are relative to `crates/kudzu-ui/src/`; Tauri paths are relative to `apps/desktop/`.

## Majors

**M1. The command menu doesn't cover LIB-13's catalog** (`actions/main.rs:41–61`, `main/command_menu.rs:259–262`, `:431–445`; Tauri `components/app/main/command-menu.tsx:92–149`).

The registry holds only the window shortcuts. Tauri's menu also has entries the registry lacks, and the registry can't express Tauri's sections or labels:
- **Commands without shortcuts:** "Import snippets…", "Export library…", "Switch to light/dark appearance", "Pause/Resume clipboard history" and "Settings › Clipboard".
- **Group entries:** one "Go to" row per group, plus group settings.
- **Context labels:** the "Selected snippet" actions name the selection, and pause and resume each have their own label.
- **Sections:** Tauri groups rows under "Selected snippet", "Actions" and "Go to". Ours puts everything under "Actions".

**Fix:**
- Extend the typed registry with actions that have no shortcut, context labels, and section and order metadata.
- Add the group-derived entries, keeping Tauri's empty-query exclusions.
- Test representative queries, and the operation each one runs.

**M2. The cheat sheet keeps its own copy of the shortcuts** (`main/cheatsheet.rs:42–210`, `:248–254`; `main/tests/command_menu.rs:248–276`; Tauri `components/app/main/shortcuts.tsx:9–70`, `main-app.tsx:110–143`).
- **Problem:**
  - If an action's chord changes in the registry, the sheet still shows the old one, and the completeness test still passes because it compares only labels.
  - The rendered check searches the whole window, not the sheet's rows.
- **Fix:**
  - The sheet references typed actions and takes their chords from the registry.
  - Section placement and display-label overrides stay as presentation data.
  - Expose screen-local shortcuts, such as the clipboard keys, as metadata, without moving their execution into the window dispatcher.
  - Test action identity and the displayed chord inside the dialog.

**M3. Captured heights replace responsive sizing** (`main/command_menu.rs:136–138`, `:203–204`; `main/cheatsheet.rs:294–296`, `:376`; Tauri `command-menu.tsx:167–170`, `shortcuts.tsx:95`).
- **Problem:**
  - At the minimum window height of 560 pt (`shell/windows/main.rs:196`), the sheet's fixed 624.422 pt overflows the window.
  - The menu is always 489 pt, so a single result or an empty search leaves a large blank panel.
- **Fix:**
  - Derive list and body limits from the viewport, and the menu's height from its content.
  - Keep the 600 and 720 widths and the 18% placement.
  - Add tests at the minimum window size and with no results.

## Minors

1. **The toolbar's search button skips the text-field snapshot** (`main/toolbar.rs:296–298` against `main/command_menu.rs:46–54`, `:458`). Route it through the shared opening transition, and test both ways of opening the menu.
2. **Escape with both dialogs open dismisses the wrong one** (`main/keys.rs:27–36`, `main/state.rs:287–293`, `main/frame.rs:51–53`). Escape clears `command_open` first, even though the sheet is drawn above the menu, and it skips the query reset.
   - **Fix:** dismiss in drawing order, and always reset through `command_menu::close`.
   - **Test:** a regression test with both dialogs open.
3. **The title test allows 2.1 pt** (`main/tests/command_menu.rs:338–347`). Either confirm the font setup and meet 2 pt, or record the difference for S73 and S74 so the owner can approve it.
4. **(orchestrator) The menu field's `on_input_key` callback has no composition check** (`main/command_menu.rs:186`, handler `:68–86`). The window-level IME test doesn't cover it.
   - **Fix:** ignore keys while composing.
   - **Test:** a regression test for that case.
5. **(orchestrator) Untested behaviour:** no test covers the effect of running an action from the menu, or the order of the frequent snippets (only their count is checked). Add both.

## What holds

- **Opening and layout.** The ⌘K menu is 600 pt wide, placed 18% from the top at 1180 × 760. Selection wraps, and both Escape and ⌘K close and reset it.
- **Snippet results.** Snippets use S21's `search::index` and `rank`, with usage and summaries. Every query term must match the label or the keywords.
- **Return.** On a snippet, Return selects it in All. On an action, it closes the menu, then runs the action.
- **The cheat sheet.** It is 720 pt wide. Its five sections and column order match Tauri, and the clipboard keys ⌘P, ⌘S, ⌘O and the list keys are listed. Tauri doesn't list ⌘R either.
- **Guards and scope.** Window-level composition, input and modal guards pass. The shell changes are small, and deferring the scenes to S73 is allowed.

## S71 live checks

- **IME:** Japanese and Chinese composition in the menu field. Return confirms the composition without selecting a snippet or running an action.
- **Focus:** focus and selection are restored after Escape, an outside click, ⌘K and ⌘/, and after a snippet or an action runs.
- **Containment:** library and clipboard shortcuts behind each dialog don't leak through.
- **Keyboard navigation:** arrow through every row and wrap at both ends, with the selected row always visible.
- **Smallest window:** at 900 × 560, check both dialogs, scrolling, the close controls and sizing with no results.
- **Themes and accessibility:** check both themes, accessibility names, modal focus containment, and the title's metrics in the system font.
