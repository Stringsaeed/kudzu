# S65 onboarding acceptance

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
