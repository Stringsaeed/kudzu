# S70 review — release pipeline

Independent review of `05e75555` (`build(desktop-v2): build, sign and package releases from xtask`). No blockers; six majors. The candidate must not be dispatched until the majors are fixed.

Confirmed correct: the base64-wrapped minisign feed and prehashed signatures, the trusted comment `timestamp:<unix>\tfile:Kudzu_<v>_universal.app.tar.gz`, both darwin platforms, `feed.mjs` precedence (including exit 3), the codesign requirement S36 pins, the Dev variant never getting an identity, profile or entitlements, `set-version` writing only `version.txt`, `set-github-secrets.sh` passing values on stdin, and no frozen file touched.

## Major

1. **AppleDouble `._*` entries in the updater archive.** `xtask/src/release.rs:181-188` runs `/usr/bin/tar -czf`, which on macOS stores extended attributes (for example `com.apple.provenance`, carried over by `fs::copy`) as `._` entries. S36's `archive_path` (`crates/kudzu-system/src/update_install.rs`) rejects a top-level `._Kudzu.app`; a nested `Kudzu.app/Contents/._Info.plist` breaks `codesign --strict` after extraction. Fix: `.env("COPYFILE_DISABLE", "1")`, `--no-mac-metadata --no-xattrs --no-acls`, ideally `--format ustar`. Add a test that runs S36's archive validation on the archive xtask produces.
2. **Toolchain and installed targets disagree.** `.github/workflows/desktop-v2-release.yml:18-20` installs `stable` plus both darwin targets, but `rust-toolchain.toml` forces 1.99.1 with no `targets`, so `--target x86_64-apple-darwin` fails once stable moves on. Fix: install 1.99.1 as `desktop-v2.yml` does and add `targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]` to `rust-toolchain.toml`.
3. **Build scripts and proc macros can read the signing secrets.** The update key and its password, the Apple certificate and its password and the API key path stay in the environment while `bundle::build` runs `cargo build` for two targets; the keychain is already imported and unlocked; `actions/checkout` leaves `GITHUB_TOKEN` in `.git/config`. Fix: build the universal bundle first with no secrets in the environment (its own step, or `env_remove` on every cargo child); in xtask read the secrets then remove them from the process environment before any child starts; import the keychain only after the build; `persist-credentials: false` on checkout.
4. **`concurrency: group: release` is shared with the frozen pipeline.** GitHub keeps one running and one pending run per group and a newly queued run cancels the pending one even with `cancel-in-progress: false`, so a 90-minute candidate can block or cancel a real `release.yml` / `beta.yml` run (rule 1). Fix: `group: desktop-v2-release`.
5. **Actions are not pinned by SHA in a job holding production secrets.** `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `actions/upload-artifact@v4`. Fix: full commit SHAs with a `# vX.Y.Z` comment.
6. **Nothing checks that the updater key is the production key, and it is unlocked last.** A wrong key or password gives a green run every installed copy rejects, or fails only after ~30 minutes of notarization. Fix: in preflight, decode and unlock the key and require its public key to equal the production public key (key id `6E52269F4241F007`); after signing, verify the archive bytes against the public key before writing the `.sig`.

## Minor

1. The P12 password reaches argv via `security import -P` (`sign.rs:89-100`). Re-wrap the P12 with a random one-time password (`openssl pkcs12 -passin env:… -passout env:…`) so only a throwaway reaches argv.
2. `-T /usr/bin/security` on import (`sign.rs:98-99`) lets the key be exported. Keep `-T /usr/bin/codesign` only.
3. The DMG is signed without `--timestamp` (`release.rs:163-168`) and `build.sh:55` verifies it without a team requirement. Add `--timestamp` (not for ad-hoc) and `-R='=anchor apple generic and certificate leaf[subject.OU] = "V3HN8HXZYK"'`.
4. The version is stamped two ways: `release.rs:97` normalizes, `bundle.rs:94` writes raw `version.txt` into `Info.plist`. Fail if `raw != parsed.to_string()`.
5. The "`dist` already exists" check (`release.rs:149-152`) runs after app notarization. Move it, and the key unlock (major 6), into preflight.
6. `keychain.close()?` (`release.rs:223-226`) drops the original error when cleanup also fails. Report both.
7. All `notarytool` output is discarded (`release.rs:34-47`). Print only the `id:` and `status:` lines.
8. The `.p8` key goes under `target/` and the profile under `assets/` (`desktop-v2-release.yml:33-37, 46`). Write both to `$RUNNER_TEMP`, as `build-macos.yml` does.
9. Release builds run without `--locked` (`bundle.rs:54-61`, the `cargo xtask` alias). Pass `--locked`.
10. `version.txt` is `0.7.0`, the frozen app's current version, so a candidate's files share names with the real v0.7.0 assets and carry production signatures; run artifacts are public for 90 days. Runbook: stamp candidates with a pre-release version such as `0.7.1-rc.1`; set `retention-days: 7`.
11. The workflow can be dispatched from any ref with the production secrets. Guard on `github.ref`; the runbook recommends moving the secrets into a GitHub Environment with a required reviewer.
12. `update-cask.sh:32-38` `zap` lists Tauri's `~/Library/LaunchAgents/Kudzu.plist`; S36 writes `sh.saeed.kudzu.plist` (`login_item.rs`). List that and add `~/Library/Logs/sh.saeed.kudzu`.
13. The signing tests use a key from `minisign` 0.10, not Tauri, and never run the installer's validator on the archive. Commit a throwaway fixture key in Tauri's format and add the major-1 test.
14. Housekeeping: `dist/` is not gitignored; `assets/Kudzu.icloud.entitlements:5-6` still mentions `tauri.icloud.conf.json`; `build.sh:65` usage omits the internal modes.
15. `RELEASING.md:33-35` "make the workflow available" is vague. GitHub dispatches `workflow_dispatch` only from files on the default branch: say `desktop-v2-release.yml` must land on `main`, then `gh workflow run desktop-v2-release.yml --ref t3/rune-migration-plan`.

## Owner steps before the first real candidate (for RELEASING.md)

1. Land the major fixes; stamp a pre-release candidate with `cargo xtask set-version`. S72 later needs a second, higher candidate, because S36 installs only a strictly greater version.
2. Put `desktop-v2-release.yml` on `main`; confirm the secret names (not values) with `gh secret list`; optionally move them into an Environment that needs approval.
3. Confirm the `xcode-27` runner has a logged-in Finder session for `create-dmg`, and whether it is self-hosted.
4. Dispatch with `--ref t3/rune-migration-plan`; check the run's SHA is the branch head and every stage passes, including cleanup. On a self-hosted runner, confirm `security list-keychains -d user` no longer lists the temporary keychain.
5. Download the artifacts and check: `shasum -a 256 -c SHA256SUMS.txt`; `tar -tzf Kudzu_*.app.tar.gz` lists only `Kudzu.app/…` with no `._` entries; the base64-decoded `.sig` verifies with `minisign -Vm <archive> -P RWQH8EFCnyZSbuM5BGaktuexN/70p9Q1ZPCwwNt5ilvPkDnn57keAIOC -x <decoded.sig>` and shows the `file:` comment; `codesign -dvv` shows team `V3HN8HXZYK`; `spctl` and `stapler validate` pass on the app and the DMG; `vtool -show-build` reports `minos 14.0` for both slices.
6. Mount the DMG: background, 660×440 window, Kudzu at (170,220), Applications at (490,220); install by dragging.
7. Run S72 against that run: back up production data and quit the installed Kudzu first, then build a second, higher candidate and test the update end to end from a local feed.
