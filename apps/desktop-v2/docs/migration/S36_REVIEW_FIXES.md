# S36 security review follow-up

The [independent security re-review fixes](S36_REREVIEW_FIXES.md) supersede the skip, owner-authentication, recovery and login cleanup details below.

Worktree: `/Users/saeed/.t3/worktrees/text-expander/t3-4435258a-e789384073b71a61`.
Branch: `t3/s36-fixes`, created with the requested `git switch -c t3/s36-fixes t3/rune-migration-plan`. The starting commit was `d7974a2f`, including merged S36 (`1794a645`) and the owner's D22/D23 notes. Implementation: GPT-6.1 Sol (xhigh); the owner supplied the Claude Opus review findings. No additional agent was spawned. The existing build directory was reused. No other worktree was changed, and no push or PR was made.

Commit message: `refactor(desktop-v2): atomic update swap, focus the running app, and S36 review fixes`. The final commit SHA is supplied in the handoff; this report is part of that commit.

## Verification and fixes

Five new regressions were first run against the merged implementation and failed: publication ordering, the second copy's bundle path, equal/newer disk versions, writable archive modes, and a query-only redirect. The red run is recorded in `target/s36-review-red.log`. Source inspection confirmed the remaining findings: the probe inspected parent writability only, no staging sweep existed, login writes had no readback/probe guard, and Fetch selected neither platform trust nor system proxies while enabling gzip decoding.

| Finding | Change | Evidence |
| --- | --- | --- |
| Two renames can empty the installed path | `update_install/swap.rs` calls macOS `renamex_np(extracted, bundle, RENAME_SWAP)` through FFI. After exchange, the old app is moved from the extracted name to `previous.app`. Failure in that retention step rolls back with a second atomic exchange. | `review_swap_failure_between_steps_never_removes_the_installed_path` injects failure after exchange and checks the installed path contains the full new app before rollback, then reads the restored old bytes. `validated_fixture_extracts_on_same_volume_and_keeps_backup_until_swap_succeeds` uses the real exchange on the temporary filesystem. |
| Crash between publication steps | Atomic exchange leaves both whole apps named; the backup-retention step never removes the installed name. | `review_killing_the_installer_between_exchange_and_backup_keeps_a_whole_app` starts a child, pauses it after the actual exchange, reads both apps, kills it, and confirms the new app remains usable and the next probe sweeps the released staging lease. |
| Rollback/fallback behavior | A failed second exchange preserves the stage and both copies. Only raw `ENOTSUP` (45 on macOS) enters the legacy two-rename fallback. | `review_failed_atomic_rollback_still_keeps_the_published_app_and_old_copy` checks both copies after injected rollback failure. `errors_other_than_enotsup_never_enter_the_fallback` checks six other error codes. The existing fallback rollback test now explicitly injects ENOTSUP and tests both successful and failed recovery. |
| Second copy focuses itself | Immediately after acquiring flock, the primary truncates/writes its bundle path into the same locked inode. A bundled secondary reads at most 4097 bytes, validates the recorded bundle, and reopens that path; absent, invalid or oversized records fall back to its own path. Unbundled secondaries still exit without reopening. | `review_second_copy_reopens_the_running_bundle_elsewhere` uses two actual temporary `.app` directories, asserts the recorded owner is reopened, and covers missing/empty/oversized records and an unbundled loser. Existing crash, identity separation and inherited-descriptor tests remain. |
| Probe lacks ownership and bundle access checks | Probe requires bundle metadata UID to equal `getuid()` and `access(bundle, W_OK)` to succeed, in addition to the existing location and parent-writability checks. | `probe_requires_bundle_ownership_and_access_not_only_parent_writability` tests a writable parent with a nonwritable bundle and an injected different UID. No real ownership change is performed. |
| A newer app on disk can be overwritten | A valid on-disk short version at or above the package's semantic precedence returns success without extraction or publication. The check runs both before staging and after verification, immediately before publication. The signed version must still satisfy the running-version and identity contract. | `review_equal_or_newer_disk_version_skips_without_verifying_or_swapping` covers equal, build-metadata-only, and newer versions. `review_disk_version_is_rechecked_after_verification_before_publication` changes the disk plist during the verifier callback and proves no swap occurs. |
| Archive permissions depend on umask | The validator rejects `mode & 0o6022`, including group/world write bits and the existing setuid/setgid prohibition. The umask can only remove more permissions. | `review_group_and_world_writable_modes_are_rejected` checks 0775, 0757 and 0777. All previous traversal/link/device/cap/signature tests remain in the suite. |
| Abandoned staging folders accumulate | Both probe and install entry paths sweep owned, private `.kudzu-update-*` directories. A live folder holds an exclusive file lock on `.active`; probes skip it. New stages record their installed bundle path in a bounded `bundle-path` record. A directory containing `previous.app` is retained when that installed bundle is missing. Cleanup is best effort. | `sweeps_only_stale_owned_directories_and_preserves_the_only_backup_and_live_leases` checks stale removal, a live lease, a symlink, an injected different UID, and preservation/removal of a sole backup as the bundle disappears/reappears. The quiet-skip install test also checks stale removal. The process-kill test demonstrates release and cleanup after a crash. |
| Login write is not verified | After `auto-launch` returns from enable, the adapter reads the actual plist and requires its Label and exact two ProgramArguments (executable, `--hidden`) to match. | The temporary-HOME child runs the ordinary enable/readback path, then injects wrong labels, wrong executables, and missing arguments; each is rejected. |
| Login item can target an unsuitable location | `LaunchAgent::new` now takes the shared `Arc<dyn UpdateInstaller>`. Enable calls its probe before writing and refuses `needs_move`; disable remains available. Constructor documentation tells S40 to classify failure as login item unavailable. | The isolated HOME child injects a needs-move probe and proves enable returns an error without creating a plist. The probe itself has separate real-filesystem permission tests. |
| D22: macOS trust | Workspace ureq enables `platform-verifier`; `rustls-platform-verifier = "0.7"` is declared as approved and resolves to 0.7.1. Fetch explicitly selects `RootCerts::PlatformVerifier`; enabling the feature alone would retain ureq's default WebPKI selection. | Compiled configuration in `files.rs`, resolved Cargo metadata/lock, strict Clippy and dependency gate. Positive TLS acceptance with the owner's managed trust roots remains a live check, not a claim from HTTP fixtures. |
| D22: system proxy | Environment overrides are honored. Otherwise CFNetwork resolves macOS HTTP/HTTPS/SOCKS settings, per-URL exceptions and PAC. SOCKS support uses ureq's existing feature without another direct dependency. PAC files bootstrap directly through the same bounded/cancellable transport (1 MiB cap), then CFNetwork evaluates the script on a bounded run loop. | `system_dictionary_proxy_and_exceptions_are_resolved_per_url` injects macOS settings. `environment_proxy_is_honoured_by_the_actual_socket_request` uses an isolated child and real loopback CONNECT proxy, with no target DNS lookup. PAC tests evaluate inline scripts, download a PAC from a loopback server, check cancellation/deadline errors, and load/cap a temporary file PAC. No system proxy preferences are changed. |
| D22: bytes and Content-Length | ureq's default compression features are disabled and `Accept-Encoding` is explicitly absent. Even an unsolicited Content-Encoding cannot transform archive bytes. | `requests_send_no_accept_encoding_and_preserve_the_body_and_length` inspects the actual HTTP request and checks exact response bytes and `(5, Some(5))` progress. `lying_lengths_are_bounded_and_content_encoded_archives_stay_byte_for_byte` retains misleading-length checks and asserts exact gzip bytes. Existing endless-body, cap and stalled-read cancellation tests remain. |
| D22: query-only relative Location | `?x` is resolved against the current resource path before HTTP URI parsing, replacing its query rather than changing its directory. HTTPS enforcement still runs on the resolved target. | `query_only_redirect_replaces_query_on_the_same_resource` failed before the fix and now checks the literal expected URL. Existing downgrade-refusal tests pass. |
| D16 field comment | `UpdatePackage.digest` now says BLAKE3, matching the module contract and implementation. | The comment change in `kudzu-app/src/ports/update_install.rs`; digest tamper-rejection coverage is unchanged. |

The ENOTSUP fallback deliberately retains the old non-atomic window on filesystems without swap support, as requested. The atomic guarantee applies to the normal `RENAME_SWAP` path. Failed fallback recovery preserves `previous.app`; a missing installed bundle prevents cleanup from deleting its only backup.

## PAC loading investigation

The native CFNetwork PAC URL-loading API returned no callback and made no request to the loopback fixture within the test budget on this worker-thread harness, including after trying Chromium's documented direct-lookup initialization. The inline CFNetwork evaluator and ureq loopback transport worked. The diagnostic census in `target/s36-pac-census.txt` recorded:

| Actor | Before replacing the native downloader |
| --- | --- |
| PAC HTTP fixture | 0 requests |
| Native PAC URL callback | 0 completions, test exit 101 |
| Inline PAC evaluator | test exit 0 |
| ureq loopback transport | test exit 0 |

This is an observed limitation of that experiment, not a claim that the native API always fails. The final implementation gives network I/O to ureq and uses CFNetwork only for script evaluation. The same real PAC-file regression then passes. The native URL loader is absent from the final code. Apple's [proxy API documentation](https://developer.apple.com/documentation/cfnetwork/cfnetworkexecuteproxyautoconfigurationurl(_:_:_:_:)) describes the combined download/evaluation operation; separating those responsibilities preserves the app's timeout, cancellation and size limits.

## S40 integration requirements

- Acquire and retain `InstanceGuard` **before NSApplication starts**. A secondary exits before it can register itself with Launch Services or consume the owner's reopen event.
- Construct one bundle installer/probe and pass its `Arc<dyn UpdateInstaller>` as the third argument of `LaunchAgent::new(bundle_id, executable, installer)`.
- A failed login-item constructor means **login item unavailable**, not application startup failure. Do not substitute a fake that silently succeeds.
- Keep the existing ordered shutdown and explicit-restart callback contract. A successful quiet skip may mean the on-disk bundle was already newer; it does not authorize an automatic restart.

## Gate and scope

The final gate **passes** on macOS 27.2 (26B123), arm64:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| `cargo test --workspace` | PASS, exit 0; system suite: 39 passed, 6 intentionally ignored entry points/smoke |
| `cargo xtask check-deps` | PASS, exit 0; dependency and containment checks passed |

Commands ran one at a time in the foreground from `apps/desktop-v2`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-deps
```

Final logs are under `target/s36-review-{clippy,workspace-tests,deps}.log`. Earlier focused logs include intermediate failures; the final workspace log is authoritative. The new proxy and atomic-swap child entry points are intentionally ignored when selected by themselves, and invoked by ordinary parent tests with isolated environment variables. The existing explicit Launch Services smoke remains ignored in the default gate; this follow-up does not claim a new live product-app focus/relaunch session.

The installer helpers were split into `update_install/{swap,location}.rs` and proxy resolution into `files/fetch/proxy.rs` to keep responsibilities separate and production modules below the plan's 600-line limit. Other integration changes are the approved workspace dependency/features and Cargo-generated lockfile, the system crate's existing-approved CoreFoundation dependency, the explicitly requested port comment, and this report. No frozen desktop app, release workflow or root JavaScript workspace file changed.

All filesystem mutations for behavior verification use disposable fixtures. Login tests run only in a subprocess with a temporary HOME. The existing signing negatives use only ad-hoc and generated self-signed fixture keys in a disposable keychain; no real keys or installed Kudzu application are used. No real login item or system proxy configuration was registered or changed.

S72 must still verify the real Developer ID/notarized artifact, live second-copy main-window focus and lifecycle ordering, real login behavior, mounted read-only/translocated locations, and positive TLS acceptance/routing on the owner's managed network. The local tests establish adapter behavior and negative signature checks, not those live product claims.
