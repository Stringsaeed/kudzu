# S51 report — editor shell

Built by Claude Opus 5.5 on `t3/snippet-editor-shell`, from Carbon's unreviewed draft (`b981342d`). Paths are under `apps/desktop-v2/crates/kudzu-ui/src/` unless they say otherwise.

## Commits

| SHA | Message |
|---|---|
| `41395899` | refactor(desktop-v2): add the snippet editor shell |
| `37d450af` | merge of `origin/t3/rune-migration-plan` (S42, S61). One conflict in `shell/lifecycle.rs`, resolved by keeping both lines. |
| `9362de02` | test(desktop-v2): cover the editor toolbar, behaviour and landmarks |
| `596429c6` | refactor(desktop-v2): write the editor draft before a delete |
| `d4d1638a` | refactor(desktop-v2): settle the editor's refusals, headless preview and idle status |
| `80a3c42d` | merge of `origin/t3/rune-migration-plan` (S60). No conflicts. |

The commit that adds this report follows them. Nothing is pushed.

## Accept list (PLAN `#### S51`)

**Tests first.** The pure models were written together with their unit tests: `Autosave`, `Session`, `Timed`, the trigger `commit` rules, the `Header` star, `Facts`, the behaviour sentences, and `off_banner`. The headless runtime tests (`main/tests/editor*.rs`) were written after the code they drive. The rule asks for tests first; this is what actually happened.

### Autosave

Code is in `main/editor/autosave.rs`.

| Item | State | Tests |
|---|---|---|
| The draft updates at once | done | `main/tests/editor.rs:196` `typing_a_name_updates_the_draft_at_once` |
| Saves after 350 ms | done | `main/editor/autosave.rs:229` `the_write_is_due_350_ms_after_the_last_edit`<br>`main/tests/editor_autosave.rs:72` `the_draft_is_written_350_ms_after_the_last_edit` |
| Flushes on blur, navigation, window close and ⌘S | done | `main/tests/editor_autosave.rs:170` `navigation_window_blur_and_hiding_write_the_draft` (leaving the library, opening another snippet, `window_focus(false)`, `commit_drafts`)<br>`:92` (⌘S) |
| An outside change is taken only when the draft is clean | done | `main/editor/model.rs:289` `an_outside_change_is_taken_only_while_the_draft_is_clean`<br>`main/tests/editor_autosave.rs:209` |
| "Saved" only once `library.saved()` reaches the draft's revision | done | `main/editor/autosave.rs:241`<br>`main/tests/editor_autosave.rs:92` `saved_appears_only_once_the_writer_reaches_the_drafts_revision` |
| A failed write shows "Couldn't save" until one succeeds, across navigation and hiding | done | `main/editor/autosave.rs:255`<br>`main/tests/editor_autosave.rs:114` `a_failed_write_shows_couldnt_save_until_a_write_succeeds` |

Notes on the failure test and the copy:

- The failure test uses a read-only data folder. It checks the status, the header's accessible name, the toast and the writer's sentence "The file could not be saved.". It then opens another snippet, hides and shows the window, restores the folder, and waits on `saved().durable`.
- A draft the library refuses (`That snippet no longer exists.`) toasts that sentence, and the refusal goes with its draft. See `main/tests/editor_autosave.rs:156` and `main/editor/autosave.rs:294`.
- Every status has Tauri's copy (`each_status_has_its_tauri_copy`).

### Name

| Item | State | Tests |
|---|---|---|
| Return moves to the triggers | done | `main/tests/editor.rs:171` |
| It does nothing mid-composition | done | `main/tests/editor.rs:184` |
| New and duplicate focus the name | done | `main/tests/editor.rs:210` (⌘N, ⌘D)<br>`main/tests/editor_header.rs:128` (More → Duplicate) |

### Triggers

Code is in `main/editor/triggers.rs`.

| Item | State | Tests |
|---|---|---|
| Prefix suggestion | done | `main/tests/editor.rs:171`<br>`main/tests/editor_triggers.rs:190` (a lone prefix is dropped on blur) |
| A whitespace paste splits atomically | done | `main/editor/triggers.rs:532`<br>`main/tests/editor_triggers.rs:57` |
| Space, Return, Tab and blur commit | done | `main/tests/editor_triggers.rs:33` |
| Backspace edits the last | done | `main/tests/editor_triggers.rs:168` |
| Escape clears | done | `main/tests/editor_triggers.rs:177` (clears, then returns to the list) |
| The exact messages for whitespace, duplicate and over 64 graphemes | done | `main/editor/triggers.rs:564`<br>`main/tests/editor_triggers.rs:135`, `:57` (on screen) |
| Shake and duplicate pulse, both expiring | done | `main/tests/editor_triggers.rs:88` `the_shake_and_the_pulse_expire`<br>`:111` (the chip pop expires too) |
| Amber on conflict | done | `main/tests/editor_triggers.rs:251` (fill `warning/15%`, help text, both shadowing sentences, "another snippet") |
| IME | done | `main/tests/editor_triggers.rs:216` |

### Behaviour, preview, banners and header

| Item | State | Tests |
|---|---|---|
| Behaviour controls and inherited-source text | done | `main/tests/editor_behavior.rs:46` (each control flips the draft and is saved; every sentence)<br>`:107` (inherited)<br>`:127` (formatted insertion)<br>`:149` (app rule, M2)<br>`:236`, `:268` |
| Preview: 120 ms debounce | done | `main/tests/editor_preview.rs:24` |
| Preview: stale results dropped | done | `main/tests/editor_preview.rs:51` (another snippet's answer). `rune::Latest` drops superseded answers. |
| Preview: the badges | done | Five badges, in Tauri's copy and order. `main/editor/preview.rs:361`<br>`main/tests/editor_preview.rs:86` (copy and help text on screen) |
| Preview: the refresh cue after 600 ms of stillness, and its expiry | done | `main/editor/preview.rs:381`<br>`main/tests/editor_preview.rs:117` |
| Preview: sample selection and "Nothing to show yet." | done | `main/tests/editor_preview.rs:69` |
| Collision banners that select the other snippet | done | `main/tests/editor_triggers.rs:251` |
| The disabled banner and the no-trigger note | done | `main/editor/view.rs:371`<br>`main/tests/editor.rs:244` |
| Header menu | done | `main/tests/editor_header.rs:35` (group menu)<br>`:107` (copy expanded, and the port's sentence on failure)<br>`:128` (duplicate, move, delete)<br>`:165` (Escape closes the menu) |
| The favorite burst only when turning on | done | `main/editor/header.rs:556`, `:575` (Reduce Motion)<br>`main/tests/editor_header.rs:55` (also expires, and stays still on a snippet switch) |
| Enabled/Off | done | `main/tests/editor_header.rs:94` (the label swap, and the banner) |

### Rules 12–15 checks

- **Motion expires.** Shake, pulse and pop: `main/tests/editor_triggers.rs:88`, `:111`. Star: `main/editor/header.rs:556`, `main/tests/editor_header.rs:55`. Preview cue: `main/tests/editor_preview.rs:117`.
- **Sizing.**
  - Every size comes from content and the viewport.
  - `main/tests/editor_landmarks.rs:215` covers the pane at 900 × 560 (the toolbar inside the pane, the body padded 32 and scrolling).
  - `:263` covers the empty state centred at both sizes.
- **IME.**
  - The name, trigger and content fields are covered at `main/tests/editor.rs:184`, `main/tests/editor_triggers.rs:216` and `main/tests/editor.rs:225`.
  - The picker's search field belongs to S55's `app_picker`.
- **Domain error text reaches the UI.**
  - The writer's sentence: `main/tests/editor_autosave.rs:114`.
  - The library's sentence: `:156`.
  - The pasteboard port's sentence on a failed copy: `main/tests/editor_header.rs:107`.
- **Toggles flip the source of truth.**
  - Favorite, Enabled, group and behaviour edit the draft, and the library holds them after the write.
  - ⌘⇧F and ⌘⇧E flip the library after writing the draft: `main/tests/editor_autosave.rs:234`.
- **Ports come from `MainServices`:** `library`, `expansion`, `apps`, `settings` and `clock`.

## Gates

Each was run from `apps/desktop-v2`, in the foreground, on `80a3c42d`. The logs are in `target/s51-gates/`.

| Command | Last printed line |
|---|---|
| `cargo fmt --all --check` | no output, exit 0 |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | `Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.12s` (exit 0) |
| `cargo test -j 2 --workspace` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` (the doc-tests, exit 0) |
| `cargo xtask check-deps` | `workspace dependency and containment checks passed` |
| `python3 scripts/check-headless.py` | `All modes and window scripts passed under a filesystem write sandbox, on fixtures.` |
| `cargo xtask parity` | `662 rows, 0 failed, 90 not yet built; report: …/target/parity/report.md` |

On the test log:

- `grep -c "^test result: ok" target/s51-gates/test.log` printed `64`.
- No `FAILED` line appears in it.
- The build profile was `dev`.
- The machine also ran other worktrees' builds at the time.

## The deferred scene

**`snippet-actions` stays in `NOT_YET_BUILT`.** The reason is in `xtask/src/parity/reference.rs`. The reference shows the content field's chip, quick toolbar and Plain text / Formatted selector, which S52 and S54 build. These tests cover the editor's region instead:

- `main/tests/editor_landmarks.rs:152` (`library`) and `:157` (`snippet-actions`), with the bundled fonts, in both themes, within 2 pt.
  - **Full rects:** Move to group, Remove from favorites, Enabled, More actions, Snippet name, Remove trigger ;sig and Add trigger.
  - **Content region:** x, y and width only. Its height is asserted to be the stand-in's 198 pt; the reference is 276.
  - **Preview, Behavior (region and heading), Instantly and After the word:** not literal full rects. Every rect is compared within 2 pt *after moving the reference up by the 78 pt* the stand-in field lacks. When S52 and S54 land, the shift should reach 0 and the constant goes.
- `:162` keeps the scene out of `SCENES` and listed as not yet built.
- `:169` checks the More menu's copy. `:188` checks the surface, radius and depth tokens in both themes.

## Edits outside the step's files

None of these restructure anything.

- **`main/editor/content.rs`.** This is a new file, outside S51's Owns: it is S52's. It holds the stand-in content field, so that S52 replaces one file (see "The content-field interface").
- **`shell/lifecycle.rs`.** `:41` adds `editor::flush` in `commit_drafts`. `:207` adds `editor::sync` in `sync`. `:239` adds `editor::poll` in `poll`.
- **`shell/windows/main.rs`.**
  - `:147` changes the constructor line to `editor::State::new(icon_wake.clone())`.
  - `:230` changes the `on_focus` closure to call `editor::window_focus` first.
- **`main/frame.rs:138`.** The `editor::view` call now passes `cx` and `overlays`. This is a signature change.
- **`main/nav.rs:66`.** `select_snippet`'s four-line `editor.show` block becomes one `editor::open_selected` call, which writes the old draft first.
- **`main/library/actions.rs`.**
  - `:103-104`: Duplicate moves focus to the editor and asks for name focus.
  - `:122`: ⌘⇧F and ⌘⇧E write the draft first.
  - `:153`: `delete_with_undo` writes the draft first.
- **`main/tests/group_settings.rs:76`.** The `landmark` helper now searches only the dialog's layer. The editor behind it has its own "Default" radio. This is a corrected test-rig helper.
- **`main/tests/mod.rs:15-21`.** Seven `mod editor*;` lines.

## App picker call sites

The picker is used through the current API, inside the uniquely keyed container `editor-app-rule`:

- **`main/editor/behavior.rs:457`.** `AppPicker { add_label, ..AppPicker::new(&rule.bundle_ids, installed, &query, &icons) }.open(..).build(theme, cx, edit, toggle_app, set_open, overlays)`.
- **`main/editor/behavior.rs:209`** (`request_icons`). `app_picker::icons_needed` drives the icon cache.

What depends on S55's fixes:

- The tests reach the picker through the container and labels ("Allow an app", the "Mail" row), not through its inner keys.
- An `on_key` on the container closes the list on Escape when focus is inside it. Nothing was added to `main/keys.rs`.
- Known until S55 lands: the list is placed from `cx.layout`, which ignores scrolling (S55 M8c). In a scrolled editor body it opens off by the scroll distance.

## Reused from the draft

Kept in substance:

- the conflict sentences and the `chip_keys` rule;
- the prefix-on-focus and quiet-commit-on-blur flow;
- the trigger field's handlers (rewritten);
- a timed favorite burst;
- autosave on `saved()` with a sticky failure;
- the preview as a debounced render with a stillness cue;
- the snippet-actions builder.

Discarded:

- **The "M2" rule that turned an empty rule into `None`.** It is a misreading of S55's M2. Tauri saves `{allow, []}` ("Disabled everywhere"). M2 is about keeping `insertion`.
- **`find_conflicts` on the UI thread for every dirty edit.** The editor reads `library.conflicts()`.
- **Monotonic-clock deadlines.** The frame clock is used, so tests drive time with `frame(dt)`.
- **The task-per-preview plumbing.** It is replaced by `rune::Latest`.
- **Its edits to shared files.**

## Differences from Tauri, and open items

The owner needs to see these:

- **A whitespace-only paste** into the trigger field shows "A trigger can't be only spaces." and shakes. Tauri drops it silently. This makes the validator's whitespace sentence reachable, which the Accept list names. It is new visible behaviour.
- **Tab mid-composition** in the trigger field is swallowed, not used for focus traversal. Rune reserves only Return and Escape for the input method, and the blur would commit a half-composed trigger.
- **Deletes and toggles write the draft first.** Deleting the open snippet and ⌘⇧F/⌘⇧E write the dirty draft before acting, so Undo restores the edited version.
  - Tauri has no equivalent ordering.
  - Not covered: bulk-bar actions, and outside moves on a dirty open draft, are still overwritten when the draft saves. That is Tauri's rule.
- **Duplicate copies the stored snippet**, not the dirty draft, as Tauri does.
- **"Created" and the relative times use UTC.** Insights does the same, and no local offset reaches the UI.
- **`MainModel.draft`** (S40's stand-in) is still in `shell/windows/main.rs`, and `commit_drafts` still writes it. It isn't the editor's draft: removing it means changing S40's shell and its tests (`shell/tests.rs:125-155`), which is outside this step.

## Rune gaps

These are drafts. Nothing was filed upstream.

- **Lucide icons draw as strokes only.** The favorite star is coloured, not filled. It is marked `RUNE-GAP(icon fill)` in `main/editor/header.rs` and has no issue number yet.
- **Beat has no rotation (#567).** The star burst runs as timed scale and rotate targets.
- **Text fields keep their own keys.** Rune runs no ancestor `El::on_key` while a text field is focused. Every editor field therefore ends its `on_input_key` in `editor::field_key` (⌘S, Escape), and the pane's `on_key` covers buttons and switches. This is a design constraint, not a defect.
- **`Cx::layout` ignores scrolling.** This affects the picker's placement (S55 M8c).

## The content-field interface (for S52)

`main/editor/content.rs` is the stand-in, and S52 replaces it. Its interface:

- **`Field`**
  - `load(&mut self, source: &str)`: show a source, keeping focus.
  - `source(&self) -> &str`: the source the draft saves.
  - `is_composing`, `is_focused`, `blur`, `focus(&mut self, tx)`.
  - The field must keep its keyed element at `editor::CONTENT_KEY`. ⌘E and the list's Return focus it, and tests read that key.
- **`toolbar(s, theme) -> Option<El>`:** the controls at the end of the "Expands to" row: the quick toolbar and Insert. The stand-in returns `None`.
- **`view(s, theme) -> El`:** the field in its card.
  - It calls `editor::content_changed(s, tx)` after every edit. That copies `source()` into the draft and restarts autosave and the preview.
  - Its key handler ends in `editor::field_key(s, ev, tx)`: ⌘S, Escape, and the IME guard.
  - Formatting edits should update `draft.formatting` through `editor::edit_draft`, which `content_changed` uses.
- **Landmarks.** The test constant `STAND_IN_CONTENT_H` in `main/tests/editor_landmarks.rs` holds the stand-in's 198 pt. Once S52/S54 reach the reference's 276 pt content region, remove the constant and its shift.
