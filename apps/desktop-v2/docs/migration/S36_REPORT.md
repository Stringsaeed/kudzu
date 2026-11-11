# S36 implementation report

The [S36 security review follow-up](S36_REVIEW_FIXES.md) supersedes the swap, instance, probe, login and Fetch implementation details below.

- Worktree: `/Users/saeed/.t3/worktrees/text-expander/t3-4435258a-e789384073b71a61`
- Branch: `t3/system-adapter-install`. The worktree branch was externally renamed from its initial `t3/4435258a` while HEAD stayed at `6086bc0d`; no branch/checkout/reset command was performed by this implementation.
- Commit message: `refactor(desktop-v2): add the login item, file, instance and update-install adapters`.
- Commit SHA: provided in the implementation handoff; this report is included in that commit.
- Implementer: GPT-6.1 Sol, xhigh, in the assigned implementation worktree. No delegates or separate reviewer.
- Machine: macOS 27.2 (26B123), arm64. The required APFS clone of the warm target cache was made before the first Cargo build. Cargo operations ran one at a time in the foreground.

## Adapter surfaces

| Surface in `kudzu-system` | Contract |
| --- | --- |
| `login_item::LaunchAgent::new(bundle_id, executable)` → `LoginItem` | `auto-launch` 0.6 LaunchAgent; identity is both plist name/Label and AssociatedBundleIdentifiers; executable receives `--hidden`. Construction has no registration side effect. Repeated enable refreshes the path, repeated disable succeeds. |
| `files::WorkspaceFiles` → `FileOpener` | NSWorkspace file URLs for open and Finder reveal. Missing files fail before invoking an application. AppKit reveal acknowledges the request, not Finder's eventual selection. |
| `files::fetch::HttpFetch` → `Fetch` | `ureq` 3.5.1; HTTPS initial requests and redirects; one global deadline across redirects/body; streaming byte cap independent of Content-Length and decompression; known/unknown progress; cancel before and between reads plus socket shutdown for a blocked read. The cancellation watcher is stopped/joined on every return. |
| `instance::Instance::acquire(directory, bundle_id, bundle)` | `Primary(InstanceGuard)` owns a nonblocking exclusive `flock` until dropped/process exit; `Secondary` means exit, even if the reopen request fails. Bundled second launches use Launch Services; unbundled ones do not reopen. The lock inode is deliberately retained. |
| `relaunch::BundleRelauncher::new(bundle, quit)` → `Relauncher` | Spawns a helper with positional path arguments, waits for the old PID to exit, then `open -n`s that bundle. Calls the supplied ordered-shutdown/exit callback only after spawn succeeds. No implicit restart after installation. |
| `update_install::BundleInstaller::new(bundle, bundle_id, running_version)` → `UpdateInstaller` | Probe for translocation, `/Volumes`, and parent writability. Private 0700 staging directory beside the running bundle; digest-checked private archive snapshot; bounded archive normalization/validation; extracted plist and pinned codesign verification; rename old→backup, new→installed; rollback on failure; preserve the backup directory if rollback itself fails. |

`files::fetch` is a separate Rust module inside the owned `files.rs`; no extra implementation file or parent registration was needed outside S36's Owns list.

S40 must pass the resolved identity, its data directory and bundle/executable paths. Pass the running binary's compiled version to `BundleInstaller`, not the currently installed file's version. Hold `InstanceGuard` until process exit; exit on `Secondary`. The relaunch quit callback must arrange the complete ordered application shutdown and process exit. The update service already owns the 400 ms restart delay.

## Acceptance evidence

The fixture generator in `update_install.rs` builds USTAR headers and adversarial archives without a third-party test crate. All test writes stay in temporary directories. Login item tests run in a subprocess with a fresh HOME; no real login item was installed, registered or removed. No installed Kudzu bundle or real signing key was used.

| S36 acceptance item | Result and evidence |
| --- | --- |
| Identity-labelled LaunchAgent, `--hidden`, idempotent write/remove with temporary HOME | **PASS** — `login_item::tests::launch_agent_round_trip_uses_only_child_temporary_home` starts the isolated child and reads the actual plist, label, associated id, argument vector and RunAtLoad. Checks a separate identity and repeated enable/disable. |
| OS-held flock, release after crash | **PASS** — `instance::tests::crash_releases_the_os_lock_without_removing_the_file` kills a process holding the lock, then reacquires the same inode without removing it. The other instance tests check simultaneous identity separation and release on drop, including a fork-inherited open-file description. The latter regression was reproduced red before adding explicit unlock on guard drop. |
| Bundled second launch reopens, unbundled second launch exits | **PASS (adapter boundary)** — the instance test observes exactly the bundled reopen request and `Secondary` in both cases, including reopen failure. The final shell's actual main-window focus is S72. |
| Reject traversal and absolute paths, second top-level item | **PASS** — `traversal_absolute_second_root_and_unsafe_archive_metadata_are_rejected` runs through actual `/usr/bin/tar` normalization and the real pre-extraction validator, and confirms no swap or stray files. |
| Reject wrong bundle id, version lower/equal to running, mismatch with signed version | **PASS** — `signed_version_identity_and_digest_are_checked_against_running_version`; covers a wrong plist id and wrong package id, an embedded/signed mismatch, lower/equal versions and a build-metadata-only change. Changing the on-disk installed plist does not change the running-version baseline. |
| BLAKE3 staged archive digest | **PASS** — same test changes the digest and proves refusal before signature verification or rename. The installer hashes while copying into its private directory and extracts only a validated derivative of that snapshot. |
| Pinned team/id, deep/strict signature verification; reject ad-hoc and self-signed | **PASS (negative fixtures)** — both fixtures pass ordinary `codesign --verify --deep --strict`. Both fail the installer requirement; the self-signed fixture has the expected bundle id and OU `V3HN8HXZYK`, and asserts a real requirement failure rather than invalid requirement syntax. It also goes through `UpdateInstaller::install` and leaves the old bundle intact. Temporary fixture keychain is deleted; user search list and default keychain are unchanged. A valid Developer ID fixture is deliberately reserved for S72. |
| Reject escaping symlinks/hardlinks, devices, setuid/setgid | **PASS** — traversal/metadata test plus `internal_links_work_but_link_chains_cannot_escape_or_be_written_through`. Includes chained symlink escape, cycle, writes beneath a symlink, character/block devices, FIFO, duplicate/case-alias entries, setuid and setgid. Internal symlink/hardlink contents and shared inode are checked after successful extraction. |
| Cap extracted bytes and entries | **PASS** — `unpacked_bytes_and_entry_count_are_capped_even_without_a_large_archive` runs the production validator with small caps and a >1 GiB declared file; rejects before attempting to read that data. |
| Good fixture extracts beside bundle on same volume; backup survives until success | **PASS (injected successful verifier)** — `validated_fixture_extracts_on_same_volume_and_keeps_backup_until_swap_succeeds` checks parent paths, `st_dev`, actual extracted contents and backup during both real renames. The success-only verifier seam is private. No production signature bypass is exposed. |
| Failed swap recovers old bundle; failed recovery preserves backup | **PASS** — `failed_swap_rolls_back_and_failed_rollback_preserves_the_backup` injects failures into the real rename sequence and reads the retained old bytes. |
| Translocation/read-only refused with `needs_move`, probe leaves no debris | **PASS (temporary paths)** — `translocation_read_only_and_probes_leave_no_debris` checks injected AppTranslocation and `/Volumes` paths, a 0555 parent, and actual probe directory cleanup. A real mounted DMG/read-only volume remains S72. |
| Relaunch works from a bundle | **PASS** — the ordinary test checks arguments, shutdown ordering and spawn failure. The explicit ignored smoke `relaunch::tests::launch_services_restarts_a_temporary_bundle_after_the_old_process_exits` was run separately: a child calls the real adapter and exits; Launch Services launches a temporary accessory fixture whose executable writes a marker. No product app is launched. |
| HTTPS-only redirect policy, timeout, cap, cancel, progress | **PASS** — `files::fetch_tests` use joined loopback HTTP servers through a private policy parameter used by tests; the public adapter always requires HTTPS. Tests cover known/unknown totals, excessive/conflicting/truncated Content-Length, gzip expansion beyond cap, endless chunked size/deadline bounds, active cancellation and cancellation of a stalled body; a slow valid read still completes. HTTP redirects and production HTTP are refused. No Fetch test contacts the real network. |

## Required gate

All required gates **PASS** on the final implementation:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 |
| `cargo test --workspace` | PASS, exit 0; the system suite has 22 passed and four intentionally ignored entries described below |
| `cargo xtask check-deps` | PASS, exit 0; workspace dependency and containment checks passed |
| Explicit temporary-bundle Launch Services smoke | PASS, one test passed; real helper and Launch Services, temporary accessory fixture only |

Logs live under the ignored `target/` directory:

- `target/s36-clippy.log`
- `target/s36-tests.log`
- `target/s36-deps.log`

Reproduce from `apps/desktop-v2`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-deps
cargo test -p kudzu-system relaunch::tests::launch_services_restarts_a_temporary_bundle_after_the_old_process_exits -- --ignored
```

The default system suite contains four intentionally ignored entries: three subprocess-only entry points and the explicit Launch Services smoke. The temporary-HOME and flock subprocess cases are invoked by their ordinary parent tests. The restart subprocess is invoked by the separately run smoke. They are not skipped acceptance checks.

The first workspace test run exposed a release race when another test forked a subprocess: closing the owner descriptor alone could temporarily leave the shared open-file description locked until the child exec. A deterministic duplicated-descriptor test failed before the fix and passed after `InstanceGuard::drop` explicitly issued `LOCK_UN`. The original failed run is retained in `target/s36-tests-first.log`; the final gate uses the corrected implementation.

## Bounds and integration notes

- Compressed/staged archive cap: 256 MiB. Extracted regular-file bytes: 1 GiB. Entries: 10,000. Encoded normalized TAR: 1 GiB + 10,000 × 1 KiB + 1 MiB. Info.plist: 1 MiB. Paths: at most 64 components. Link resolution: at most 64 hops.
- Archive input is normalized using system libarchive (`tar -cP --format=ustar @archive`), with absolute names preserved for rejection. Only the private, validated USTAR file is extracted. PAX/GNU metadata is normalized before validation. Unrepresentable USTAR entries, unsupported entry types, duplicate paths, case aliases and nonportable paths fail closed. Paths must be printable ASCII, contain no backslashes, and fit USTAR; ACLs, xattrs and file flags are not restored. S70 must produce a compatible archive with only `Kudzu.app` at top level and no AppleDouble siblings. Real embedded signature preservation is an S72 gate.
- The designated requirement uses an inline `=` prefix, Apple generic anchor, exact bundle id, leaf OU `V3HN8HXZYK`, and Developer ID intermediate/leaf OIDs. Production signing, trust insertion, notarization and positive Developer ID acceptance were not attempted.
- `auto-launch` 0.6 writes unescaped XML. This adapter explicitly refuses executable paths containing `<`, `>` or `&` (or control characters) before any registration rather than writing a corrupt plist. This is a current limitation for unusual installation/home paths.
- Socket cancellation interrupts headers, TLS and body I/O. Name resolution and a TCP connection still in progress are bounded by the request deadline, as documented by ureq; the atomic flag is checked again when those operations return.
- The existing `UpdatePackage.digest` field comment says SHA-256, while its module contract and S31 implementation say BLAKE3. This step follows D16 and the implementation. The consuming service owns that port file, so S36 leaves the stale field comment untouched.
- If rollback fails, the old app is deliberately retained at `<bundle parent>/.kudzu-update-<uuid>/previous.app`. Success and ordinary failure clean up their staging directories. Abrupt power-loss recovery is not claimed by the two-rename protocol.

## S72 live checklist

Use S70's notarized release candidate and real Developer ID team `V3HN8HXZYK` on Apple Silicon and Intel/Rosetta. With the owner's authorization and production-data backup:

1. Verify a real signed newer archive survives normalization/extraction and passes the pinned codesign requirement, then installs on actual Quit and on Restart. Check the new executable/version, same-team permission retention, preserved data and the 400 ms restart delay.
2. Test wrong team/id, tampered staged bytes, signed/embedded version mismatch and equal/lower running versions against the final app. Confirm a failed install leaves the app usable and never causes an automatic restart.
3. Launch from the DMG and a genuinely translocated path; confirm actionable needs-move state. Repeat from a real read-only volume and a writable installed location; confirm probes leave nothing behind.
4. Exercise real login enable/disable/login with `--hidden`, first-run override, separate production/Next identity, simultaneous second launches and actual main-window focus/Dock reopen. This implementation session did not register a real login item.
5. Run the product bundle's full shutdown/relaunch with an unsaved draft, engine/clipboard workers and an open agent socket. Confirm ordered persistence and shutdown precede process exit and relaunch acquires the instance lock.
6. Check Finder open/reveal against actual selected files, including spaces/non-ASCII names. Observe focus and selection; these were not driven against real user files here.
7. Complete S72's other lifecycle, iCloud, appearance and performance requirements; this adapter step makes no claim about them.

## Integration edits outside Owns

- `crates/kudzu-system/Cargo.toml`: workspace references for approved `auto-launch`, `ureq`, `blake3`, `semver`, `plist`, `uuid`, and macOS `objc2`, `objc2-foundation`, `objc2-app-kit` dependencies.
- `Cargo.lock`: generated by Cargo, including transitive packages; not hand-edited.
- `docs/migration/S36_REPORT.md`: this requested implementation report.

No edits to `apps/desktop`, existing workflows, root JavaScript workspace files, application ports/fakes, composition root, or another worktree. No agent branch change, push or PR.
