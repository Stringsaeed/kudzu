# S42 review: sidebar

This is an independent review by Grok 4.6 of `51bc649a refactor(desktop-v2): start sidebar navigation` and `fc064426 refactor(desktop-v2): add the sidebar`. The orchestrator checked M1 against `dnd.rs:60–93` before acting on it.

**Verdict:** fixes needed. There are no blockers.

**Tests run by the reviewer:**
- `cargo test -p kudzu-ui sidebar`: 87 passed.
- `cargo test -p kudzu-app settings`: 14 passed.
- `check-headless.py`: passed.

## Majors

**M1. A pointer drop on itself or a descendant still calls `move_group`, and shows a toast** (`crates/kudzu-ui/src/dnd.rs:69–80` `hover` against `:83–93` `drop_at` and `:13–39` `drop_payload`; Tauri `sidebar.tsx:364–371`).
- **Cause:** `hover` excludes `descendant_ids`, which includes the group itself, but `drop_at` doesn't. `move_group` returns `GroupCycle`, and the toast says "A group can't be moved inside itself."
- **Tauri:** it ignores these drops silently.
- **Why the tests miss it:** they call `drop_payload` directly, and the pointer test never checks that no toast appears.
- **Fix:**
  - Use the same self and descendant check in `drop_at` and `source_release`, so the drop does nothing.
  - Keep the toast only for callers of `drop_payload`.
  - Assert no toast appears after `pointer_up`.

**M2. Dragging a group row doesn't swallow the press that follows (Rune #551)** (`main/sidebar/mod.rs:94–98`, `:119–132`; the working pattern is in `library/drag.rs:23–25`, `:59–64`, `:71–73` and `library/row.rs:198–200`).
- **Cause:** after a group drag, Rune still delivers `on_press`.
- **Failure:** from All snippets, drag group A onto B. The move succeeds, and then A opens.
- **Fix:**
  - Mirror `library/drag.rs`: set `swallow_press` on the group drag, and skip the next press. Tag it `RUNE-GAP(#551)`.
  - Assert the view doesn't change after a group drag.

## Minors

1. **Home, End and the arrows walk one list that includes Pause** (`sidebar/mod.rs:279–308`, `:493–513`). In Tauri, Pause has no `data-nav-item`, so the tree and the footer are separate regions (`sidebar.tsx:169–177`, `status.tsx:70–73`).
   - **Fix:** split the key list and leave Pause out, or record this difference in `PARITY_MATRIX.md`.
2. **Disabled groups have no Off mark and aren't muted** (`sidebar/item.rs:16`; Tauri `sidebar.tsx:326`, `:331`, `:349`).
   - **Fix:** mute the row, add `CIRCLE_OFF` with the accessible name "Off", and use `FOLDER_OPEN` for an expanded group with children.
3. **The Dev ribbon on the wordmark is missing** (`sidebar/mod.rs:474–487`; Tauri `sidebar.tsx:500–505`; FEATURES §3).
   - **Fix:** use `s.shell.identity().dev`.
4. **The context menu has no separator, and the two deletes aren't styled as destructive** (`overlays.rs:101–160`; Tauri `sidebar.tsx:400–406`).
5. **Errors from saving section folds are swallowed** (`mod.rs:250`, `:254`, `:263` use `let _ =`).
   - **Fix:** show the `SettingsError` text, as rule 14 requires.
6. **Left and Right on a group have no happy-path test** (`mod.rs:107–116`; Tauri `sidebar.tsx:337–342`).
7. **A group drop always appends** (`dnd.rs:28` and `actions.rs:101` use `i64::MAX`). Tauri keeps `sortIndex` when it changes the parent (`sidebar.tsx:368–370`).
8. **The sidebar is built locally instead of with rune-kit `Sidebar*`** (PLAN §1.4).
   - **Fix:** add a `PARITY_MATRIX` note that the kit can't host this tree.
9. **Leftovers in `sidebar/mod.rs`:**
   - Its first line still has a stale comment ("group management remains to be built").
   - It's 518 lines, over the 400-line target.

## What holds

- **Sections and folders:**
  - Every section and filter, including Off when any snippet is off.
  - The clipboard folder rule, through `CLIP_FOLDERS` and `ClipFilter::contains`.
  - Recursive counts, and Recent counting unique ids.
- **Groups:**
  - Rename by double-click, F2 or ⌥↩, with trimming, a blank name keeping the old one, Escape cancelling and blur committing.
  - A new subgroup expands its parent and starts a rename.
  - The menu's seven items and both delete modes.
- **Drag and drop:** snippet to group, group to group, and unparenting at the root. The ghost, and hover stability.
- **Folding:**
  - Section folds persist in `ui-state.json`, as a second `JsonFile` on `SettingsService`, guarded by a mutex and kept out of sync.
  - Folded rows stay mounted, inert and hidden from accessibility.
- **Motion and layout:**
  - The selection pill passes its motion gate, and the tests wait for completion.
  - The collapse slot is a 224 pt panel, offset by −216.
  - Landmarks are checked as full rects in both themes.
- **Behaviour rules:**
  - Pause goes through `toggle_paused`.
  - The footer copy matches each status.
  - IME is checked on rename, the tree and the menu.
  - Domain errors reach the UI for rename, cycles and delete.
- **Hygiene:** the shared hooks are small. There's no file over 600 lines, no `#[allow]`, no logging of user text, and no dependency change.

## S71 live checks

1. Drag group A onto B: the view stays where it was.
2. Drag a parent onto its child, or a group onto itself: no toast appears.
3. Press End from All snippets or Favorites, and decide whether Pause should be the last stop.
4. A Dev build shows the Dev ribbon.
5. A group that's turned off shows CircleOff, a muted row and the accessible name "Off".
6. Clicking the collapse chevron of an unselected group folds it without opening it.
7. An expanded group with children shows the open-folder glyph.
8. A section fold survives a restart.
9. Dragging across a group's icon and label doesn't make the highlight blink.
10. Hiding the sidebar slides the 224 pt panel by 216 pt, and its subtree stays mounted and inert.
11. Renaming a group during IME preedit leaves Enter, Escape and F2 to the composition.
12. The status chip, in its Limited and Needs-permission states, opens Settings. The tooltip with the palette shortcut is still missing (`status.tsx:60–66`).
