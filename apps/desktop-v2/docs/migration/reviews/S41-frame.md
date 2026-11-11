# S41 review: main window frame

This is an independent review by Grok 4.7 of `a019419e refactor(desktop-v2): add the main window frame`. The orchestrator checked the cited lines on both sides before acting on it.

**Verdict: fixes needed first.** Wave 9 (S42, S43, S50, S55, S58, S64, S65) does not start on this frame until the fixes land.

**Gates at review time:**
- `cargo test -p kudzu-ui main`: 14 passed.
- `check-headless.py`: passed.
- Parity: 540 rows, 0 failed, 110 not yet built. Every `main/*` scene is "not yet built", and `main/scenes.rs` `SCENES` is empty.

## Blocker

**B1. `Route::NewSnippet` is dropped** (`shell/windows/main.rs:311-314`).

`reconcile_route` navigates to All and copies `content` into `editor.content`. It never reads `name` or `trigger`, never calls `create_snippet`, and never selects an id.
- The editor renders `EmptyState` whenever `selected_snippet` is `None` (`main/editor/mod.rs:50-51`, `109-111`), so the content never shows.
- `commit_drafts` persists only `model.draft` (`shell/lifecycle.rs:40-45`), and nothing assigns that field.
- Two callers already send this route: Quick Search ⌘N with a trigger or name (`palette/drive.rs:131-138`), and selection capture with content (`shell/lifecycle.rs:205-208`). In both cases the user lands on "Select a snippet", and hide or quit saves nothing.

**Fix:**
- Create the snippet from `name`, `trigger` and `content`, select it, and focus its name, as Tauri's `newSnippet` does (`apps/desktop/lib/store.ts:191-207`: `selectedId`, `focusNameOf`).
- Keep any pending field state on `editor::State`, so S51 doesn't depend on a route that has already been consumed.
- PLAN S41 requires this (`PLAN.md`, S41 Accept).

## Majors

**M1. `shell/windows/main.rs` collects every screen's logic.**
- The file is 585 lines, 15 under the 600-line cap.
- `dispatch_action` (`384-513`), `delete_snippets_with_undo` (`328-381`) and `reconcile_route` (`283-323`) all live there. The constructor builds the screen states as unit values (`129-136`).
- S50 (neighbour selection), S42 (new group) and S58 (clipboard service) would each edit this file in parallel, and the next one would cross 600.

**Fix:**
- Move each action body, and the delete/undo toast, into the screen module that owns it.
- Leave this file with only the services, the window spec and thin calls.
- Construct every screen state with `Default`.

**M2. Deleting the open snippet doesn't move Rune focus, and it clears the selection** (`shell/windows/main.rs:348-357`).
- The code sets `selected_snippet = None` and moves `Focus<MainPane>` to `List` without a `Tx`.
- `apply_focus_after_action` doesn't handle `DeleteSnippet` (`main/keys.rs:127`). Escape does it correctly with `tx.blur()` and `tx.focus(library::LIST_KEY)` (`main/keys.rs:59-62`).
- Tauri selects the neighbour (`store.ts:222-224`) and focuses the list (`main-app.tsx:96`).
- The result: Rune focus stays in the editor while the model says `List`, so the next key both types and is treated as not-in-input.

**Fix:**
- Handle `DeleteSnippet` the way `FocusList` is handled.
- Select the neighbour, as Tauri does.

**M3. Undo loses the snapshot when the id is already back** (`shell/windows/main.rs:368-376`).
- The handler calls `take()` on the batch before `restore` runs.
- `restore` skips ids that are already present (`kudzu-app/src/library/snippets.rs:65-75`). So if sync has put the id back, or `restore` returns `Err`, Undo dismisses, selects nothing and drops the batch.

**Fix:**
- Call `take()` only after `Ok`.
- If the id is already present, still select the first intended id.

**M4. ⌘N and ⌘⇧N don't match Tauri** (`shell/windows/main.rs:389-420`).
- ⌘N from Recent, Disabled, Insights, Settings or Clipboard must switch to All. From Favorites it must set `is_favorite`. Tauri does both (`store.ts:191-206`).
- ⌘⇧N is `createGroup("New group", null)`: a top-level group named "New group" (`main-app.tsx:112`). Ours parents the group to the current one and names it "New group {n}".
- Subgroup creation belongs to S42's sidebar action.

**M5. The content card fill is wrong, and it draws a second edge** (`main/frame.rs:124-135`).
- `build_main_card` fills `c.card` and adds `.border(1.0, c.hairline)` on top of `depth.card`. The first layer of `depth.card` is already a 1 px ring (`theme/depth.rs:123-127`).
- Tauri's `<main>` is `bg-background shadow-card` (`main-app.tsx:190`). In dark mode, `background` is `INK` and `card` is `STONE_850`.

**Fix:**
- Fill with `c.background` and drop the border.
- Keep `depth.card` and radius `XL2` (18 pt matches `--radius-2xl`).

## Minors

1. **Breadcrumbs** (`main/toolbar.rs:57-102`, `main/state.rs:43-47`).
   - All with no selection needs a non-clickable "All snippets" leaf (`toolbar.tsx:54`).
   - The clipboard leaf for the all-items filter is "History", not "All items" (`toolbar.tsx:36`).
2. **⌘, and ⌘K must open, not toggle** (`shell/windows/main.rs:386-387`, `489-494`).
   - Tauri always opens Settings (`main-app.tsx:129`) and only ever opens the command menu (`main-app.tsx:110`).
   - Update the runtime test that locks ⌘, as a toggle (`main/tests.rs:507-517`).
3. **`Action::enabled` ignores the text-field guard** (`actions/main.rs:256-267`, compared with `resolve_in_context` at `191-193`). A cheat sheet built from `enabled` would show Delete as available inside a field.
4. **The first navigation after mount always counts as an arrival.** The mount stamp is `f64::NEG_INFINITY` (`motion.rs:247`).
5. **Search should narrow at small widths** (`main/toolbar.rs:25`, `271`). It stays 192 × 32 at every width. Tauri uses `w-48 max-[1000px]:w-36`, so it is 144 pt wide below 1000 pt.
6. **The command-menu stub is 560 pt wide** (`main/command_menu.rs:15`). S43's Accept says 600. S43 owns the file, so leave this one to S43.
7. **`trigger_front_action` doesn't dismiss the toast** (`main/toasts.rs:249-255`). The on-screen Undo button does.

## Orchestrator findings

1. **No main-window scenes.** `main/scenes.rs` `SCENES` is empty, so no main-window parity row is compared.
   - `STATES.md` has no frame-only state, so the chrome is first compared in S50's `library` scene.
   - Until then, add a tree test for the chrome landmarks with rects within 2 pt, in both themes:
     - toolbar height 42 with the 88 pt inset;
     - sidebar open at 224 and collapsed;
     - breadcrumb overflow;
     - the search button;
     - the card;
     - the undo toast;
     - chromeless onboarding.
2. **`--snap --window main --theme light` renders dark.** The onboarding references exist in both themes (`STATES.md:22-26`). Find out why the main window ignores `--theme` when the other windows honour it.
3. **Fixtures open onboarding.** The Tauri mock starts with `onboardingCompleted: true` (`apps/desktop/lib/mock.ts:79`) and shows onboarding only with `?onboarding`. `Host::fixtures` must do the same.

## What holds

- **Shortcuts.** The chords are registered and the runtime test drives them.
- **Input guards.**
  - IME composition returns before dispatch.
  - Modals swallow shortcuts, except Escape, ⌘K closing the command menu, and ⌘/ closing the cheat sheet.
  - ⌘⌫ is the only action with `allow_in_input = false`.
- **Quit and hide.** ⌘Q is handled before the guards. Hide, quit and closed each call `commit_drafts`.
- **Layout.** These match the reference:
  - toolbar 42 pt, with an 88 or 12 pt left pad;
  - sidebar slot 224 or 0, `inert` and hidden from accessibility when closed;
  - gutters 8 pt;
  - onboarding replaces the whole window.
- **Architecture.** Domain reads use `library.watch().snapshot()`. Intents call the services directly, with no command bus.
- **Rule 9.** `NewSnippet`'s `Debug` prints lengths only, and `commit_drafts` logs the error, not the draft.
- **Extra bindings.** ⌘[, ⌘] and ⌘\ (history and sidebar) are additions. They don't replace any Tauri binding.

## S71 live checks

- **Toolbar inset.** Traffic lights at (18, 22) give an 88 pt inset, and fullscreen drops it to 12 pt.
- **Search width.** Search is 192 × 32, and 144 pt wide below 1000 pt.
- **Sidebar.**
  - The material follows the window's active state.
  - It animates from 224 to 0 and hides at width 0.
  - When collapsed, it is absent from the accessibility tree.
- **Onboarding.** It shows no toolbar, sidebar or vibrancy.
- **Card.** In dark mode the fill matches `INK`, the radius is 18, and `shadow-card` is a single ring.
- **Real input source.**
  - Shortcuts do nothing during IME composition.
  - ⌘Q hides the window, runs `commit_drafts` and leaves the app running.
  - A system quit still runs `on_quit`.
- **Undo.**
  - The toast lasts 6 s, restores the snippets and selects the first restored one.
  - It moves list focus when the open snippet was deleted.
  - It still works when an in-flight sync has already reinserted the id.
- **New snippet routes.**
  - Quick Search ⌘N opens `main` with the name or trigger applied and that snippet selected.
  - "New Snippet from Selection" does the same with the captured content.
- **Logs.** Create, delete and undo log no snippet body, trigger or clipboard text.
