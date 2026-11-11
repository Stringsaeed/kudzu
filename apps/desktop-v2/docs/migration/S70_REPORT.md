# S70 implementation handoff

Worktree: `/Users/saeed/.t3/worktrees/text-expander/t3-f2cb9958-5694c5f59f30243c`

Branch: `t3/xtask-release-pipeline` (unchanged).

Commit: the commit containing this report, with message
`build(desktop-v2): build, sign and package releases from xtask`.
The final handoff records its exact SHA.

Implementation is committed; **S70's live acceptance remains pending**. Nothing
was pushed, tagged, published or dispatched. No real signing credentials were
looked up or used, and no release DMG was created.

## Commands delivered

```
cargo xtask release
cargo xtask release --dry-run
cargo xtask release cleanup
cargo xtask feed --release FILE [--current FILE] --out FILE
cargo xtask set-version VERSION
```

The release command imports a CI P12 into a private temporary keychain, restores
the previous search list and deletes the keychain on success or ordinary failure.
A journal supports `release cleanup` after process interruption; CI invokes it
in an always-run step. Cleanup failures fail the job. Tests inject a security
runner; they do not create a real keychain or import any certificate.

## Acceptance

| Plan item | Result | Evidence and boundary |
|---|---|---|
| Feed SemVer precedence matches feed.mjs | PASS | `feed::tests::semver_precedence_matches_old_feed`: numeric/text prerelease identifiers, stable promotion, prefix v, ignored build metadata. CLI smoke advances beta to stable, then returns 3 without rewriting for an older candidate. |
| latest.json keys and both Darwin platforms | PASS (generator) | `wrapped_encrypted_key_feed_verifies_with_s31` checks all four top-level keys and both platforms from the same manifest generator used by release. A production artifact was not generated. |
| Existing-format key signature verifies with app | PASS | Encrypted throwaway minisign key, base64-wrapped like Tauri; release signer emits wrapped signature with timestamp/tab/versioned filename; the actual S31 verifier source verifies both platform entries with legacy signatures disabled. Tampering and wrong password fail. |
| SKIP_NOTARIZE=1 scripts/release/build.sh works locally | NOT RUN in full | `SKIP_NOTARIZE=1 scripts/release/build.sh --dry-run` passes. An ad-hoc universal fixture passes the actual app verification script with both slices at minos 14.0. Full packaging would create a release DMG, prohibited in this task. |
| DMG background, 660×440, icons (170,220)/(490,220) | NOT RUN visually | Layout arguments are tested and the 15-file design manifest passes. No DMG was created or mounted; this is not visual evidence. |
| Dispatched CI green: app/DMG notarization and stapling, spctl, minos, keychain gone | NOT RUN | Dispatch-only, contents-read workflow is implemented and YAML-parsed. No dispatch authorized. Mock keychain success/import-failure cleanup tests pass; real Apple/runner acceptance is pending. |

## Rule 8 gate

All final commands ran in `apps/desktop-v2`, sequentially for Cargo:

- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace`: PASS, 1,068 passed and 6 existing ignored tests across 55 suite summaries; xtask has 49 passing tests.
- `cargo xtask check-deps`: PASS.
- `python3 scripts/check-headless.py`: PASS, all four modes under the filesystem sandbox.

Logs are local under `target/s70-evidence/{clippy,tests,check-deps,headless,dry-run}.log`.
All release shell files pass `bash -n`. A generated test cask has the existing
description and no WebKit/HTTPStorages zap entries. `set-version` was exercised
with the current version and left `version.txt` unchanged.

An initial test run failed because TMPDIR inside the Cargo workspace caused
existing check-deps fixtures to be considered unlisted members. The final run
used a temporary directory outside the Cargo workspace but inside this git
worktree; residual test artifacts were moved under target afterward. No test
or architecture rule was weakened. The ad-hoc probe also caught this Xcode's
multi-architecture `lipo -verify_arch` failure; verification now invokes it once
per architecture, and both calls pass.

## Owner and S72 actions

The complete ordered procedure is in [RELEASING.md](../../RELEASING.md).

1. Owner makes the dispatch workflow available, selects the rebuild branch and
   verifies the commit. Confirm existing secrets without printing their values.
2. Dispatch Desktop v2 release candidate. Require signing, profile/certificate
   checks, app and DMG notarization/stapling, spctl, both minos checks and cleanup
   to pass. Save the run URL and logs; only run artifacts are uploaded.
3. Download the exact artifacts, verify SHA256SUMS, mount the DMG and inspect its
   background, window and icon layout. Install by dragging to Applications.
4. S72 records machine/macOS/display and the run/SHA in NATIVE_SYSTEM.md; quit
   installed Kudzu and back up production data first. Run natively and under
   Rosetta, then execute every lifecycle, permissions, UI, iCloud/account-change
   and budget item in S72.
5. Produce a higher-version candidate with the same production updater key;
   exercise S72's local-feed new-app-to-new-app flow, install-on-quit, Restart,
   needs-move and tamper rejection. Confirm the S36 Developer ID and BLAKE3 gates
   with real artifacts. Carry unresolved failures to S74.

## Integration outside Owns

- `xtask/Cargo.toml`: approved workspace dependencies minisign, base64, semver,
  chrono; dev-only minisign-verify and thiserror to compile S31's verifier tests.
- `Cargo.lock`: generated by Cargo, including minisign's transitive dependencies.
- `xtask/src/main.rs`: module declarations, subcommand dispatch and help text.
- `RELEASING.md`: S03's explicitly requested wordmark bitmap regeneration step
  plus the owner/S72 release runbook.
- This report: required handoff documentation.

The S31 verifier/error modules are referenced in tests from within desktop-v2;
no app or installer source was edited. Frozen desktop, website, existing release
workflows and root package files are unchanged.

API references consulted: [minisign 0.10](https://docs.rs/crate/minisign/0.10.0),
[create-dmg 1.2.3 source](https://github.com/create-dmg/create-dmg/blob/994a036532d3ac1bb1cd5a425a0d5d796ecbed83/create-dmg).
