# S65 review: onboarding

This is an independent review by Grok 4.7 of `9ae212b0 refactor(desktop-v2): add onboarding`. The orchestrator checked M1 against `onboarding/steps.rs:102–132` and M2 against `onboarding/actions.rs:149–162`.

**Verdict:** fixes needed. There are no blockers.

**Checked on the reviewer's tree:**
- 8 onboarding tests pass.
- `parity --window main`: 170 rows, 0 failed.
- The sandboxed `--tree` and `--snap` runs of three scenes exit 0.

## Majors

**M1. The Access card doesn't follow Tauri's permission card** (`steps.rs:102–132`; Tauri `components/app/permissions/permission-setup.tsx:14–46`, mounted from `onboarding.tsx:226–240`).

The card always shows one of two titles and one fixed body, and always the button "Review permissions". It never reads `engine.status()`: not `can_expand`, `summary`, or either permission row. It never shows "Opening…" or an error when the companion fails to open.

| State | Tauri | Ours |
|---|---|---|
| Ready and expanding (the captured scene) | "Permissions are set" / "Kudzu is ready to expand your snippets." / two rows / outline "Review permissions" | "Ready to expand" / the required-or-optional sentence / no rows |
| Not granted | "Enable typing in other apps" / "A guided setup, with every step beside you." / primary "Set up permissions" | "Accessibility · Required" / the same sentence / "Review permissions" |
| Granted but paused, or Secure Input on (`can_expand` false) | "Permissions are set" with `status.summary` | "Ready to expand" |

Parity passes only because the card body isn't a landmark.

**Fix:**
- Render the `PermissionSetup` states from `engine.status()`: the title, the subtitle (`can_expand` or `summary`), both rows, the button label and variant, and the open error.
- Never say "Ready to expand" while `!can_expand`.
- Keep the ready scene's button name.
- Add tests for the not-ready and paused cards.

**M2. Import failures throw away the real error** (`actions.rs:149–162`; Tauri `onboarding.tsx:107–108`, `127–129`, `151–154`; sentences in `kudzu-app/src/data/error.rs:18`, `:34`).

The worker keeps only `result.ok().and_then(Result::ok)`, so every failure becomes one fixed string. Users never see the real error, for example:
- "This isn't a valid Raycast file."
- "Couldn't back up your library first, so nothing was changed."

The Espanso "Imported N of M" prefix uses this attempt's `index` and `paths.len()`. Tauri uses the completed count over the originally detected folder count, so retrying and failing the last folder loses the prefix. The chooser failure also uses a fixed string, where Tauri shows `String(e)`.

**Fix:**
- Pass the `Display` text of the `DataError` or `TaskError` into `fail_import`.
- Prefix the count the way Tauri does.

## Minors

1. **Finish alert:** body 13/20 at a fixed 40 pt with no fill (`view.rs:92–101`). Tauri uses `text-xs` (12/16) at auto height with `bg-destructive/10` (`onboarding.tsx:298`).
2. **Import copy** (`view.rs:347`, `:354`, `:300–304`):
   - "Imported 1 snippet" and "No new snippets to add" should be singular-aware.
   - "1 item needs a look".
   - Keep the button label while working and add a spinner, rather than switching to "Importing…" (`onboarding.tsx:267–274`, `:333`, `:338`).
3. **Busy state:** Access Skip and Welcome "Get started" are disabled while busy; Tauri leaves both enabled (`onboarding.tsx:219`, `:234`).
4. **Back clears a finish failure:** `Onboarding::go` sets `error = None` (`kudzu-app/.../onboarding/mod.rs:321–326`). Tauri's `go` only changes the step (`onboarding.tsx:82–85`).
5. **First arrival:** there's no 4 pt rise. Tauri sets `--step-y` to 4px when `direction` is 0 (`onboarding.tsx:204`, `globals.css:523–527`).
6. **Progress label colour:** the current label uses `foreground`; Tauri uses `text-iris-foreground` (`view.rs:134–138`, `onboarding.tsx:301`).
7. **Dialog titles** should match `data-actions.ts:19–20`, `:32`:
   - "Choose your Espanso config folder"
   - "Choose your Raycast snippets export"
   - "Choose your TextExpander export"
8. **Re-entering setup in the same process:**
   - Scenes don't reset the `Onboarding` step.
   - Practice text survives Back, where Tauri remounts the field empty.
9. **Finish test gaps:** `failed_finish_keeps_practice_and_library_recoverable` never types into the practice field, and never checks that the frame becomes the library.
10. **`brand.rs` RUNE-GAP:** the marker has no issue number (PLAN rule 6).
11. **Stuck finish:** a panic in the finish worker before `opening.store(false)` leaves the button on "Opening…" for good.

## Orchestrator additions

12. **Busy-poll:** the import worker waits on the inner `Task` with a 5 ms sleep loop (`actions.rs`, the import worker). Wait on it without polling.
13. **Unformatted closure:** `rustfmt` skipped the import closure because a long string line couldn't fit, so the block is unformatted. Split the strings so `cargo fmt` formats it.
14. **Flaky import test:** an onboarding import assertion (`imported` was 0) failed once under `--test-threads=8` during the S43 gate, and passes alone. It is probably a timing assumption in the test. Make it wait on the import's completion, not on elapsed frames.

## What holds

- **Steps and progress.**
  - Steps run Welcome → Access → Import → Try it.
  - Back uses direction −1.
  - Skip appears on Access and Import.
  - Progress has four slots, with checks on finished steps.
- **Continue gating.**
  - Access waits for `permissions::ready`.
  - Import waits for net-new additions, so duplicates don't enable it.
- **Espanso retry and detection.**
  - Retry imports only `pending_espanso_folders`.
  - The detection states are in the domain.
  - A failed detection leaves Choose folder enabled.
- **Finish.**
  - A failure stays on Try it with `FINISH_ERROR`, and the flag stays false.
  - A finish that lands while the window is hidden shows the library on the next show.
- **Workers.**
  - Detect, import and finish run on `Task::spawn` workers.
  - Busy Back and Skip return `Busy`.
- **Practice substitution.**
  - It is local, case-insensitive and on a suffix, and never inserts into another app or logs the field.
- **Shared files and layout.**
  - Exactly the six shared-file lines the report names.
  - The card is at most 480 pt and centred; region heights match in both themes.
  - Reduce Motion follows the framework rule.

## S71 live checks

- **The Access card with real permissions.** Compare these states side by side with Tauri:
  - not ready;
  - ready and expanding;
  - paused, or Secure Input on.

  The Access button opens the companion, shows "Opening…", and an open error never looks like a grant.
- **Focus and motion.**
  - Native focus after a file dialog, and after a button that a busy import disabled.
  - Reduce Motion: the step doesn't travel. Rune still fades opacity for about 180 ms; Tauri turns the animation off entirely.
- **The practice field.**
  - It never inserts into another app.
  - IME: substitution waits for the composition to commit.
- **Finish while hidden.** Finish with the window hidden, then reopen: the library shows, and the button isn't stuck on "Opening…".
- **Import errors.** After the M2 fix, a bad file and a failed backup each show the `DataError` sentence.
