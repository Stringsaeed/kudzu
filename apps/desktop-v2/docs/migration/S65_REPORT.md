# S65 onboarding acceptance

Current review-fix results are in the final section. The original report below records the initial S65 implementation.

Base merged before commit: `95368d01` from `origin/t3/rune-migration-plan`.
The existing clipboard service field remains intact. No dependencies or domain-service methods were added.

`main/` and `shell/` paths below are relative to `crates/kudzu-ui/src/`. Other paths are relative to `apps/desktop-v2/`. Tests use a real fixture-backed `Kudzu` through `main/tests/mod.rs`.

## PLAN S65 acceptance

| Accept item | Result and evidence |
|---|---|
| Welcome, Access, Import, Try it; Back, Skip, progress dots and checks | **done**. `walks_setup_with_back_skip_and_persistent_completion`, `crates/kudzu-ui/src/main/tests/onboarding.rs:5`. Fixed progress slots and a persistent progress row: `main/onboarding/view.rs:104`. |
| Centred content, maximum width 480, stable gutter | **done**. `reference_setup_regions_hold_in_both_themes`, `crates/kudzu-ui/src/main/tests/onboarding.rs:190`. All seven region rectangles match in both themes. The outer scroll viewport keeps the content width fixed. |
| Access readiness and permission companion | **done**. `access_continue_waits_for_real_readiness`, `crates/kudzu-ui/src/main/tests/onboarding.rs:114`. Review permissions calls `Shell::open_permissions` in `main/onboarding/steps.rs:125`. |
| Three import sources; detection states; folder fallback; counts, warnings, errors; busy guards; focus restoration | **done**. `busy_import_blocks_navigation_and_cancel_restores_focus` at `crates/kudzu-ui/src/main/tests/onboarding.rs:38`; `imports_through_the_real_data_service_and_continues_on_net_additions` at line 94; `espanso_retry_keeps_partial_count_and_skips_completed_folders` at line 137. The retry removes the completed file and imports only the remaining source. Detection is read directly from the service. Existing detection-state coverage is in `crates/kudzu-app/src/onboarding/tests.rs:99`. Chooser cancellation restores focus through `Tx::focus`; worker completion and failure queue focus through the main handle (`main/onboarding/actions.rs:114`, `:163`, `:169`). |
| Local practice substitution, celebration, glow, hotkey reminder | **done**. `practice_expands_case_insensitive_suffix_locally`, `crates/kudzu-ui/src/main/tests/onboarding.rs:23`. The field uses a case-insensitive suffix match and never invokes system insertion. Success styling and shortcut caps are in `main/onboarding/view.rs:175`. |
| Open library saves completion; Opening state; finish failure and retry | **done**. `walks_setup_with_back_skip_and_persistent_completion` at `crates/kudzu-ui/src/main/tests/onboarding.rs:5`; `failed_finish_keeps_practice_and_library_recoverable` at line 58. Finish calls `Onboarding::finish` on a worker. The existing frame observes completion and opens the library. |
| Seven requested scenes in both themes | **done**. `reference_setup_regions_hold_in_both_themes` at `crates/kudzu-ui/src/main/tests/onboarding.rs:190`, plus the 80 parity rows below. Scenes explicitly clear the fixture's completed setting. |
| Directional step motion, 12 pt and 240 ms | **done**. `main/onboarding/view.rs:78` reads the machine's direction and uses a 12 pt presence offset with the shared 0.24 s soft spring. The geometry test checks settled scenes. The continuous trajectory was not separately measured. |

Closes **ONB-1, ONB-3, ONB-4**.

## Gates

The workspace run passed **1,683 tests**, with **13 ignored** and **0 failed**. The focused onboarding run passed all eight tests after the final heading-width adjustment. Cargo builds used two jobs. The xtask alias was overridden for each call as `run -j 2 -p xtask --`.

These are each gate's final nonempty output lines. Formatting emits no output on success.

- `cargo fmt --all --check`

```text
No output; exit 0.
```

- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.82s
```

- `cargo test -j 2 --workspace`

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- `cargo xtask check-deps`

```text
workspace dependency and containment checks passed
```

- `python3 scripts/check-headless.py`

```text
All modes and window scripts passed under a filesystem write sandbox, on fixtures.
```

- `cargo xtask parity`

```text
606 rows, 0 failed, 96 not yet built; report: /Users/saeed/.t3/worktrees/text-expander/t3-eb095f49-93d36f22446a1cb0/apps/desktop-v2/target/parity/report.md
```

## Scene parity

| Scene | Light | Dark |
|---|---|---|
| onboarding-welcome | 3 rows, 0 failed | 3 rows, 0 failed |
| onboarding-access | 6 rows, 0 failed | 6 rows, 0 failed |
| onboarding-import | 7 rows, 0 failed | 7 rows, 0 failed |
| onboarding-practice | 5 rows, 0 failed | 5 rows, 0 failed |
| onboarding-espanso | 7 rows, 0 failed | 7 rows, 0 failed |
| onboarding-espanso-retry | 7 rows, 0 failed | 7 rows, 0 failed |
| onboarding-finish-retry | 5 rows, 0 failed | 5 rows, 0 failed |

Full report: `target/parity/report.md`. Main-only report: `target/parity-main-final/report.md`.
The full run reports 606 rows, 0 failed, and 96 rows marked not yet built and owned by other steps.

## Shared-file additions

Every added nonblank line in `shell/windows/main.rs` is below. Existing lines were not reordered.

```rust
pub onboarding: Arc<kudzu_app::onboarding::Onboarding>,
pub data: kudzu_app::data::DataService,
onboarding: kudzu.onboarding.clone(),
data: kudzu.data.clone(),
```

The one added line in `shell/windows/mod.rs:55`:

```rust
kudzu.onboarding.watch().subscribe(waker(&h.main)),
```

The one added line in `shell/lifecycle.rs:174`:

```rust
changed |= crate::main::onboarding::poll(self);
```

The revision stamp and asset readiness remain in the owned onboarding module.

The added scene registry lines in `main/scenes.rs`:

```rust
use super::onboarding::scenes as onboarding;
pub const SCENES: &[Scene] = &[
    ("onboarding-welcome", onboarding::welcome),
    ("onboarding-access", onboarding::access),
    ("onboarding-import", onboarding::import),
    ("onboarding-practice", onboarding::practice),
    ("onboarding-espanso", onboarding::espanso),
    ("onboarding-espanso-retry", onboarding::espanso_retry),
    ("onboarding-finish-retry", onboarding::finish_retry),
];
```

The test-module addition in `main/tests/mod.rs:8`:

```rust
mod onboarding;
```

As required by the parity brief, `xtask/src/parity/reference.rs` removes the seven corresponding S65 entries from `NOT_YET_BUILT`. No other pending entries changed.

The single added table row in `docs/migration/PARITY_MATRIX.md`:

```text
| `onboarding` (S65) | `main/onboarding.tsx` | Seven `main/onboarding-*` scenes, light and dark | 80 landmark rows, 0 failed. Closes ONB-1, ONB-3, ONB-4. Wordmark SVG uses Rune's image cache to preserve its aspect ratio; step presence uses the 240 ms fitted soft spring. Practice remains a local preview. |
```

## Rune gaps and proof limits

- Rune's square icon bounds included the wordmark's transparent letterbox in tree measurements. `main/onboarding/brand.rs:1` uses Rune's SVG image cache at the wordmark's actual aspect ratio. Decoding runs on Rune's workers, and fixture scenes wait for deterministic images. No issue was filed.
- Step presence uses the shared fitted soft spring with a 0.24 s duration. It does not reproduce CSS's exact sampled timing curve. Rune's existing Reduce Motion handling applies.
- The finish-retry scene injects the deterministic error presentation. The runtime test separately exercises a real rejected finish after fixture shutdown.
- These results prove headless behavior and measured landmarks. S71 still owns live permission grants, native focus, Reduce Motion observation, and expansion in another application. The practice field proves no system insertion.

There are no new service APIs for other steps to adopt. Parallel steps must preserve the six additive wiring lines listed above.

Boundary slip: the first failed Cargo attempt wrote a short error log to `/tmp/s65-red.log`, outside the worktree. All subsequent task logs stayed in this worktree's `target/` directory.

## S65 review fixes

These results supersede the original implementation evidence above. Review baseline: `5fae2ce7`, containing merged S65 `9ae212b0` and the S43, S50, S58, and S64 changes.

All sixteen review entries are fixed. No dependencies were added. The frozen Tauri app is unchanged.

| Item | Status and correction | Test and location |
|---|---|---|
| M1 | **fixed**. Engine-status title, subtitle, both permissions, setup/review variant, pending open, and error display. | `access_card_explains_missing_permissions` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:11`<br>`paused_access_card_uses_engine_summary_without_claiming_expansion` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:30`<br>`permission_open_shows_pending_and_the_window_error` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:49` |
| M2 | **fixed**. Display text is retained for DataError, TaskError, and chooser errors. Detected retry counts use completed folders over the original total. | `bad_raycast_import_shows_the_data_error_sentence` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:87`<br>`failed_safety_backup_explains_why_import_changed_nothing` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:104`<br>`a_second_detected_retry_reports_completed_over_original_folder_count` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:145`<br>`chooser_failures_show_the_window_error_sentence` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:428` |
| 1 | **fixed**. Finish alert uses 12/16 text, natural height, destructive tint, and rounded corners. | `finish_alert_has_the_reference_tint_and_wraps_without_a_fixed_height` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:198` |
| 2 | **fixed**. Singular and zero copy match the reference. Busy buttons retain their label and add an animated spinner. | `import_outcomes_use_singular_and_zero_copy` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:214`<br>`a_working_import_keeps_its_button_label_and_shows_a_spinner` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:237` |
| 3 | **fixed**. Welcome Get started and Access Skip remain enabled while busy. | `welcome_start_remains_enabled_while_setup_is_busy` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:261`<br>`access_skip_remains_enabled_while_setup_is_busy` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:268` |
| 4 | **fixed**. Back and ordinary navigation preserve finish errors. A new finish attempt clears the error. | `navigation_keeps_a_finish_failure_until_a_new_finish_attempt` at `crates/kudzu-app/src/onboarding/tests.rs:278` |
| 5 | **fixed**. The initial arrival starts 4 pt down and settles over 240 ms. Reduce Motion skips translation. | `first_arrival_rises_four_points_then_settles` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:310`<br>`reduced_motion_skips_the_first_arrival_translation` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:409` |
| 6 | **fixed**. The active progress label uses iris foreground. | `the_current_progress_label_uses_iris_foreground` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:276` |
| 7 | **fixed**. All three chooser titles match data-actions.ts. | `import_choosers_use_the_reference_titles` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:289` |
| 8 | **fixed**. Scenes reset the domain session. Practice remounts empty after Back. | `scenes_can_restart_setup_in_the_same_process` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:343`<br>`practice_remounts_empty_after_back` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:359`<br>`restarting_setup_clears_the_previous_session` at `crates/kudzu-app/src/onboarding/tests.rs:292` |
| 9 | **fixed**. Rejected finish now tests retained practice text. Success and a recovered retry assert the library frame. | `failed_finish_keeps_practice_and_library_recoverable` at `crates/kudzu-ui/src/main/tests/onboarding.rs:59`<br>`walks_setup_with_back_skip_and_persistent_completion` at `crates/kudzu-ui/src/main/tests/onboarding.rs:5`<br>`a_panicked_finish_worker_releases_opening_and_allows_retry` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:371` |
| 10 | **fixed**. The brand comment refers to draft S65-wordmark under PLAN section 3. No Rune issue was filed. | `wordmark_workaround_resolves_to_a_plan_draft` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:445` |
| 11 | **fixed**. Task completion wakes main even after a panic. Poll clears Opening and records a recoverable finish failure. | `a_panicked_finish_worker_releases_opening_and_allows_retry` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:371`<br>`a_failed_finish_worker_leaves_a_retryable_setup` at `crates/kudzu-app/src/onboarding/tests.rs:313` |
| 12 | **fixed**. Import and detection workers block on Task::wait using a condition variable. The 5 ms polling loops are removed. | `wait_delivers_completion_without_polling_and_wakes_after_panic` at `crates/kudzu-app/src/tasks.rs:92` |
| 13 | **fixed**. Long strings are split and rustfmt formats the import closure. A width check keeps this regression covered. | `the_import_worker_fits_the_formatter_width` at `crates/kudzu-ui/src/main/tests/onboarding_review.rs:450` |
| 14 | **fixed**. Import tests wait for the operation completion watch and a committed outcome. Ten consecutive full onboarding runs pass with eight test threads. | `imports_through_the_real_data_service_and_continues_on_net_additions` at `crates/kudzu-ui/src/main/tests/onboarding.rs:105`<br>`espanso_retry_keeps_partial_count_and_skips_completed_folders` at `crates/kudzu-ui/src/main/tests/onboarding.rs:148` |

### Test-first evidence and repetition

The original code failed the new Access-state, opening, import-error, alert-style, copy, busy-state, progress-color, title, arrival, restart, and finish-panic tests before correction. The new Task wait and restart/recovery APIs first failed compilation because the methods were absent. The draft-reference and formatter-width predicates failed before their edits, then became permanent Rust checks.

Red outputs are under `target/s65-review-red-*.log`. Item 9 strengthens coverage and also uses the finish-panic regression that went red. Item 14's original intermittent failure is recorded in the pushed review; no deterministic local reproduction of that original failure is claimed.

The final repetition command was:

```sh
cargo test -j 2 -p kudzu-ui --lib main::tests::onboarding -- --test-threads=8
```

Ten consecutive runs passed: 29 tests per run, 290 passed in total. Logs: `target/s65-review-repeat-1.log` through `target/s65-review-repeat-10.log`.

The safety-backup error test uses a separate failing backup destination on a real DataService sharing the fixture Kudzu library and settings. Startup backups cannot compete for that destination. The normal import tests wait on task completion and the committed source outcome.

### Six gates

Cargo compilation used two jobs. Xtask calls used `CARGO_ALIAS_XTASK='run -j 2 -p xtask --'` and `CARGO_BUILD_JOBS=2`. Formatting has no jobs flag. No out-of-memory kill occurred in the final gate run.

The workspace run passed 1,830 tests, with 13 ignored and zero failed. These are the final nonempty gate lines. Formatting emits no output on success.

- `cargo fmt --all --check`

```text
(no output; exit 0)
```

- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 13s
```

- `cargo test -j 2 --workspace`

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- `cargo xtask check-deps`

```text
workspace dependency and containment checks passed
```

- `python3 scripts/check-headless.py`

```text
All modes and window scripts passed under a filesystem write sandbox, on fixtures.
```

- `cargo xtask parity`

```text
608 rows, 0 failed, 96 not yet built; report: /Users/saeed/.t3/worktrees/text-expander/t3-eb095f49-93d36f22446a1cb0/apps/desktop-v2/target/parity/report.md
```

### Onboarding parity

| Scene | Light | Dark |
|---|---|---|
| onboarding-welcome | 3 rows, 0 failed | 3 rows, 0 failed |
| onboarding-access | 6 rows, 0 failed | 6 rows, 0 failed |
| onboarding-import | 7 rows, 0 failed | 7 rows, 0 failed |
| onboarding-practice | 5 rows, 0 failed | 5 rows, 0 failed |
| onboarding-espanso | 7 rows, 0 failed | 7 rows, 0 failed |
| onboarding-espanso-retry | 7 rows, 0 failed | 7 rows, 0 failed |
| onboarding-finish-retry | 5 rows, 0 failed | 5 rows, 0 failed |

### Shared additions and service changes

These are all added nonblank lines in the shared UI files:

| File | Added line |
|---|---|
| `crates/kudzu-ui/src/main/frame.rs:22` | `s.onboarding.set_reduced_motion(cx.is_reduced_motion());` |
| `crates/kudzu-ui/src/shell/lifecycle.rs:227` | `changed \|= crate::main::onboarding::poll_frame(s, tx);` |
| `crates/kudzu-ui/src/main/tests/mod.rs:19` | `mod onboarding_review;` |

The single additive draft row in `docs/migration/PLAN.md` is:

```text
| S65-wordmark | Square Icon bounds include transparent letterboxing when measuring a non-square SVG wordmark. | Draft only; no upstream issue filed. | Use Rune's SVG image cache at the wordmark's actual aspect ratio. | `kudzu-ui/src/main/onboarding/brand.rs` |
```

No existing shared UI line was restructured or reordered. The S65 report has this additive review section and a pointer at its top.

Application changes required by the fixes:

- `crates/kudzu-app/src/tasks.rs:20`, `:34`, `:44`, and `:56`: a condition variable, initialization, and completion notification. `Task::wait` is added at line 63. Its regression is at line 92.
- `crates/kudzu-app/src/onboarding/mod.rs:129`: `Onboarding::restart` resets an idle setup session and refuses a busy reset.
- `crates/kudzu-app/src/onboarding/mod.rs:313`: `Onboarding::fail_finish` clears a failed finish's busy state and records FINISH_ERROR. Ordinary finish errors use the same recovery method.
- `Onboarding::go` retains the error instead of clearing it during Back or ordinary navigation. A new finish attempt still clears the error.

### Remaining proof limits

The first-arrival rise is driven explicitly because Rune skips presence transitions on an initial frame. The ready scene keeps the Review permissions button name. Wordmark rendering keeps the existing image-cache workaround and now documents its draft gap.

These are headless results. Native window creation, real permission grants, cross-application insertion, and native focus remain S71 checks. The permission-card test supplies a typed window-error completion; production observes Rune's window_error and visibility after the companion handoff. Rune's existing Reduce Motion opacity fallback remains a separate S71 comparison.
