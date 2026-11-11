# S36 independent security re-review fixes

Worktree: `/Users/saeed/.t3/worktrees/text-expander/t3-4435258a-e789384073b71a61`.
Branch: `t3/s36-fixes`. Setup ran exactly `git fetch origin && git reset --hard origin/t3/rune-migration-plan`; the resulting base was `5815e16f`. The existing build directory was reused. Implementation: GPT-6.1 Sol (xhigh), with no delegated agents. The commit containing this report is identified in the implementation handoff. No push or PR.

## Findings: red, repair, green

Every finding below first failed as a behavior test against the preceding implementation. The exact failing names are also in the commit body. Paths and line numbers refer to this commit, relative to `apps/desktop-v2`.

| Finding | Test observed failing before repair | Repair and source |
| --- | --- | --- |
| Major 1: an unsigned raised disk version becomes a successful install/relaunch | `raised_unsigned_disk_version_cannot_skip_archive_authentication`; `skipped_current_or_older_bundle_does_not_schedule_a_relaunch` | A disk-version skip now requires matching bundle id and the pinned Developer ID signature. A failed disk check falls through to digest/extraction/package validation. The native requirement includes the captured version, tying the claimed value to the valid resource seal. See `crates/kudzu-system/src/update_install.rs:83`, `crates/kudzu-system/src/code_identity.rs:12`, the outcome in `crates/kudzu-app/src/ports/update_install.rs:35`, and S31 consumption in `crates/kudzu-app/src/updates/mod.rs:431`. |
| Major 2: an advisory owner record authorizes arbitrary open or a self-reopen loop | `untrusted_owner_record_never_opens_any_bundle`; `empty_owner_record_never_reopens_the_losing_bundle` | The primary writes a bounded versioned PID/bundle record. A loser must authenticate the recorded bundle id/team, match its location to the live PID's registered bundle, and verify that PID's loaded code against the same requirement. Empty, malformed, legacy, unsigned, wrong-id or unverified records return `Instance::UnverifiedOwner` and open nothing. There is no fallback to the loser's bundle. See `crates/kudzu-system/src/instance.rs:110`, `:146`, and loaded-code verification in `crates/kudzu-system/src/code_identity.rs:59`. |
| Major 3: ENOTSUP fallback can remove the installed name across a crash | `probe_recovers_a_missing_bundle_from_the_interrupted_legacy_backup`; `unsupported_exchange_never_removes_the_installed_name` | New installs fail closed on **both ENOTSUP and EXDEV**; no two-rename fallback remains. Probe acquires the abandoned stage's lease and renames an owned `previous.app` back when its recorded installed path is missing. Recovery is limited to an app in the same parent; unsafe or unsuccessful recovery retains the backup. A preserved stage explicitly unlocks its lease even if a fork inherited its descriptor. See `crates/kudzu-system/src/update_install/swap.rs:33` and `crates/kudzu-system/src/update_install/location.rs:148`. |
| Minor 1: failed login readback leaves a live item; Program overrides the checked arguments | `login_readback_rejects_program_and_removes_failed_registration` (both isolated child cases failed) | Enable validates readback inside a failure-cleanup boundary. Any `Program` key is rejected. Failed creation/readback removes the plist, and a removal failure is returned as an I/O error. Tests confirm no plist remains and `enabled()` is false. See `crates/kudzu-system/src/login_item.rs:48`, `:67`, `:83`. |
| Minor 2: the stage changes after its first signature check | `stage_mutation_after_first_signature_check_is_rejected_before_publication` | Revalidate exact package metadata and rerun the pinned, version-constrained signature check after the final disk-skip decision, directly before publication. The mutation test verifies the installed old bundle remains untouched and swap is never called. See `crates/kudzu-system/src/update_install.rs:120`. |

Red logs are preserved locally in the ignored build directory:

- `target/s36-rereview-major1-red.log`
- `target/s36-rereview-major1-service-red.log`
- `target/s36-rereview-major2-red.log`
- `target/s36-rereview-major3-red.log`
- `target/s36-rereview-minor1-red.log`
- `target/s36-rereview-minor2-red.log`

`cross_device_exchange_never_moves_either_bundle` was an initially passing control: EXDEV already failed closed. That policy is now explicit and remains tested. Unsupported atomic exchange is a hard install failure, not an attempt to copy or unlink the running app. Such a location must be moved to a filesystem that supports atomic exchange to self-update.

Additional checks include:

- `invalid_raised_disk_bundle_is_replaced_by_the_verified_package`: an invalid raised disk version does not prevent installing the valid candidate; result is `Installed`.
- `equal_or_newer_disk_version_skips_only_with_pinned_verification`: replaces the old test that required skipping verification; asserts authenticated `Skipped` outcomes.
- `version_requirement_accepts_only_the_version_in_the_valid_resource_seal`: a temporary ad-hoc fixture accepts its sealed version, rejects a different version, and rejects a raised plist value without a new signature. No Developer ID or keychain is used.
- `loaded_process_identity_is_checked_without_a_keychain_or_real_signing_identity`: dynamic Security.framework checks accept the temporary process's matching identity, reject a wrong identity and reject the full production team requirement for ad-hoc code. No real application is launched.
- `wrong_id_unsigned_bundle_and_forged_process_cannot_authorize_reopen`: metadata/signature failures and a failed loaded-process identity cannot reach open. Releasing the unverified holder lets the next launch become primary.
- `probe_recovers_the_legacy_backup_after_sigkill_skips_drop`: a child replays the old unsafe first rename in a temporary directory; live lease prevents premature recovery; SIGKILL releases it and probe restores the complete old app. The new installer itself never performs that unsafe rename.
- The original atomic-exchange crash and rollback tests still pass.

## New install outcome and S31

`kudzu_app::ports::InstallOutcome` is:

```rust
pub enum InstallOutcome {
    Installed,
    Skipped { installed_version: String },
}
```

`Installed` means actual successful atomic publication. `Skipped` means the existing disk bundle passed the pinned id/team/version requirement and is already at or above the candidate. Its version is constrained in the native signature requirement, not accepted only from a plist read. Production verification receives a parsed `semver::Version`; the positive installer fixtures inject verification only at the OS boundary.

S31's `Updates::install_pending` now handles the outcomes explicitly:

- `Installed`: permit the explicit restart path.
- `Skipped` with authenticated disk version strictly newer than `Config.current_version`: permit the explicit restart, without claiming a swap occurred.
- `Skipped` at or below the running version: publish `UpToDate`, return false and schedule no relaunch.
- Malformed skipped version or installation failure: restore the pending package and publish an error, without relaunching.

The existing 400 ms explicit-restart delay and install-on-quit behavior remain. `skipped_verified_newer_disk_bundle_relaunches_without_claiming_a_swap` verifies the positive skip case; installed-on-quit and restart-delay tests pass.

## Owner/lock decision and S40

A same-user process can still deny startup by holding an advisory lock. The app **does not steal or unlink an active lock**, which could create two expansion engines. It returns the explicit `UnverifiedOwner` diagnostic when the recorded owner cannot be authenticated; S40 must report this condition and exit. No recorded path or caller path is opened in that case.

The PID/path association and loaded-code check also prevent an otherwise valid signed app path, with no matching running application, from causing a repeated launch loop. Unbundled primaries intentionally have no openable bundle and cannot cause a bundled loser to open itself. Legacy path-only records are unverified until that primary exits and a new primary writes the new record.

S40 still acquires the instance lock **before NSApplication starts** and retains the primary guard through exit. It must handle both `Secondary` and `UnverifiedOwner` without starting another application instance.

## Gate and verification boundaries

Final required gate passes on macOS 27.2 (26B123), arm64:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| `cargo test --workspace` | PASS, exit 0; system suite: 53 passed, 9 intentionally ignored entries |
| `cargo xtask check-deps` | PASS, exit 0 |

All long commands ran sequentially in the foreground, and completed before the handoff. Final logs: `target/s36-rereview-{clippy,workspace-tests,deps}.log`.

S70's `the_archive_made_by_the_release_pipeline_passes_the_validator_and_extracts_clean` passes unchanged, including its `#[path]` inclusion of `xtask/src/archive.rs`. The archive parser, release-candidate version rules and artifact extraction checks remain covered.

The existing self-signed fixture test creates/lists/imports a disposable keychain, conflicting with this task's stricter constraint. It is now explicitly ignored with that reason and **was not run**. Its assertions remain intact. Other ignored entries are subprocess-only helpers exercised by ordinary parent tests, plus the existing explicit Launch Services smoke and its child, which were not run here. The existing xtask signing tests use a fake runner; no security/keychain commands execute in them.

All behavior-test filesystem mutations use temporary directories. Signature fixtures are ad-hoc only and use no real signing identity. No installed Kudzu, real login item, real LaunchAgents directory, keychain or system proxy settings were changed. The real Developer ID/loaded-process positive case and final product focus/relaunch behavior remain S72 live checks, not claims from these fixtures.

## Integration edits

S31 integration edits, explicitly authorized by the task:

- `crates/kudzu-app/src/ports/update_install.rs`: new outcome type and installer result.
- `crates/kudzu-app/src/ports/mod.rs`: export the outcome.
- `crates/kudzu-app/src/fixtures/update_install.rs`: fake outcomes distinguish skip from actual installation.
- `crates/kudzu-app/src/updates/{mod,tests}.rs`: outcome-aware restart policy and regressions.

System integration: new private `code_identity.rs` centralizes the requirement and static/dynamic verification, registered in `kudzu-system/src/lib.rs`. The login probe fake implements the changed result. Remaining edits are the owned adapters and their regressions, this report, and supersession links in the prior reports. No dependency, release pipeline implementation, frozen desktop application or root workspace file changed.
