# Desktop v2 candidate pipeline

This pipeline is isolated until S75. It does not tag, push, publish a release,
change a feed, or update a tap. The production identity is `sh.saeed.kudzu`, team
`V3HN8HXZYK`; ordinary development remains Kudzu Next (`sh.saeed.kudzu.next`).

## Commands

- `cargo xtask release --dry-run`: describe stages without reading credentials or invoking them.
- `cargo xtask release`: universal production bundle, temporary CI keychain,
  hardened signing, notarized/stapled app and DMG, updater archive/signature,
  `latest.json`, and SHA256 checksums in a new `dist/` directory.
- `cargo xtask release cleanup`: recover temporary keychains after interruption;
  only run after the release process has stopped.
- `cargo xtask feed --release dist/latest.json [--current beta.json] --out beta.json`:
  advance only to a higher SemVer precedence; exit 3 and leave output untouched otherwise.
- `cargo xtask set-version 0.7.1-rc.1`: stamp `version.txt` only. Pre-releases are valid and
  order correctly (`0.7.0 < 0.7.1-rc.1 < 0.7.1-rc.2 < 0.7.1`) in the feed gate, S36's feed
  check and S36's installer. `version.txt` must be canonical (no `v` prefix); a release stops
  otherwise.

`build.sh` delegates to xtask; its internal preflight/verification modes enforce
artwork, profile, certificate, entitlements, both architectures, minimum macOS,
Gatekeeper and stapling checks. `SKIP_NOTARIZE=1` skips only Apple submission,
stapling and Gatekeeper assessment. With `APPLE_SIGNING_IDENTITY=-` and a
throwaway wrapped updater key, it builds ad-hoc artifacts; those are not usable
by the production installer. `--dry-run` never produces artifacts.

Install both Rust Darwin targets (`rust-toolchain.toml` lists them, so rustup adds them) and
create-dmg **1.2.3** (the workflow pins its source commit). The CI runner must have a logged-in
Finder session for layout. No fallback skips Finder layout. A failed build may leave `dist/`;
move it aside before retrying. All cargo invocations use this workspace's `target/` and
`--locked`.

### What a real run checks before it builds

- `version.txt` is canonical SemVer, and `dist/` does not exist.
- The updater key unlocks, and its public half is the production key (key id
  `6E52269F4241F007`, the key S36 compiles in). Only an ad-hoc build
  (`APPLE_SIGNING_IDENTITY=-`) may use another key. After signing, the archive is verified
  against that public key before the `.sig` is written.
- The profile names the production app id and CloudKit container.

### Where the secrets go

xtask reads the signing secrets once and removes them from its own environment; every cargo
child also has them stripped. The universal bundle is built before the certificate is imported,
so no build script or proc macro runs while a credential exists on the machine. The workflow
compiles xtask in an earlier step with no secrets, then runs the finished binary; checkout does
not persist the `GITHUB_TOKEN`. The P12 is re-wrapped under a random one-time password before
`security import`, and only `/usr/bin/codesign` may use the imported key. The `.p8` is written
only after the build, inside a private directory under `$RUNNER_TEMP` that is deleted at the
end. The provisioning profile is written to `$RUNNER_TEMP/kudzu-release/` and passed by
`KUDZU_PROVISIONING_PROFILE`. `notarytool` output is reduced to its `id:` and `status:` lines.

The job runs only for `refs/heads/t3/rune-migration-plan` and `refs/heads/main`, uses its own
concurrency group (`desktop-v2-release`, so it cannot block or cancel `release.yml` or
`beta.yml`), pins every action to a commit SHA, and keeps run artifacts for 7 days.

## Owner: real candidate evidence before S75

Run these in order. The implementation agent dispatched nothing.

1. **Stamp a pre-release candidate.** `version.txt` is `0.7.0`, the frozen app's current
   version, so an unstamped candidate would share file names with the real v0.7.0 assets and
   carry production signatures, and run artifacts are readable by anyone who can read the
   repository. On the rebuild branch run `cargo xtask set-version 0.7.1-rc.1` and commit it.
   S72 later needs a second, higher candidate (`0.7.1-rc.2`), because S36 installs only a
   strictly greater version.
2. **Put the workflow on `main` and check the secrets.** GitHub dispatches `workflow_dispatch`
   only from workflow files on the default branch, so `desktop-v2-release.yml` must land on
   `main` first. Then dispatch with
   `gh workflow run desktop-v2-release.yml --ref t3/rune-migration-plan`. Confirm the secret
   names, never their values, with `gh secret list`: `APPLE_CERTIFICATE` (base64 P12),
   `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`,
   `APPLE_API_ISSUER`, `APPLE_API_KEY_P8`, `APPLE_PROVISIONING_PROFILE`,
   `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The last two map to
   `KUDZU_UPDATE_KEY` and `KUDZU_UPDATE_KEY_PASSWORD`; the key is base64-wrapped minisign key
   text, not a path, and no new production key is generated. Recommended: move these secrets
   into a GitHub Environment with a required reviewer and add `environment:` to the job, so a
   dispatch needs a second click. The job already refuses any ref except
   `t3/rune-migration-plan` and `main`.
3. **Check the runner.** Confirm that the `xcode-27` runner has a logged-in Finder session for
   `create-dmg`, and whether it is self-hosted. A self-hosted runner shares its keychain search
   list with whoever uses it.
4. **Dispatch and watch.** Check that the run's SHA is the branch head and that every stage
   passes, including cleanup. On a self-hosted runner, confirm that
   `security list-keychains -d user` no longer lists the temporary keychain. Cleanup failure is
   a failed run, not evidence of keychain removal. Save the run URL and logs. The artifacts
   attach only to the run; the feed URL describes the future release location and is not live.
5. **Verify the artifacts.** Download them and, in their directory:
   - `shasum -a 256 -c SHA256SUMS.txt`
   - `tar -tzf Kudzu_*.app.tar.gz` lists only `Kudzu.app/…` and no `._` entries.
   - The base64-decoded `.sig` verifies:
     `minisign -Vm Kudzu_*.app.tar.gz -P RWQH8EFCnyZSbuM5BGaktuexN/70p9Q1ZPCwwNt5ilvPkDnn57keAIOC -x decoded.sig`,
     and shows the `file:Kudzu_<version>_universal.app.tar.gz` trusted comment.
   - `codesign -dvv` on the app and the DMG shows team `V3HN8HXZYK`.
   - `spctl` and `xcrun stapler validate` pass on the app and the DMG.
   - `xcrun vtool -arch arm64 -show-build` and `-arch x86_64` both report `minos 14.0`.
6. **Check the DMG by hand.** Mount it: the background shows, the window is 660×440, Kudzu is
   at (170,220) and Applications at (490,220). Install by dragging.
7. **Run S72 against that run.** Back up production data and quit the installed Kudzu first.
   Then build the second, higher candidate and test the update end to end from a local feed
   (see below).

For an interrupted runner, stop the release process before invoking
`cargo xtask release cleanup`. A killed runner needs owner recovery before reuse.

## S72: use that exact candidate

1. Record the run URL, commit, checksums, Mac/macOS/display and all pass/fail results
   in `docs/migration/NATIVE_SYSTEM.md`. Quit installed Kudzu and back up its production data
   before launching this production-identity candidate.
2. Install from the DMG; run natively on Apple Silicon, then run its executable
   with `arch -x86_64 /Applications/Kudzu.app/Contents/MacOS/Kudzu` under Rosetta.
3. Exercise first run, permission grants, hidden login start, Dock toggle,
   lifecycle and unsaved drafts, themes, vibrancy, motion, and iCloud on two Macs
   including an account change, as specified by S72.
4. Produce a second higher-version signed candidate using the same updater key.
   Follow S72's local-feed harness to prove new-app to new-app download, verify,
   install-on-quit, Restart, needs-move from DMG and tampered-archive rejection.
   The S36 installer must accept the real Developer ID requirement and BLAKE3
   staging check. Ad-hoc or throwaway-key artifacts cannot establish that result.
5. Measure every §1.7 budget against the captured baseline. Record unrun checks
   as unrun; a green local build is not evidence of native release acceptance.

## Artwork prerequisite (S03 handoff)

After changing `design/wordmark/kudzu-wordmark-echo-small.svg`, first regenerate
`design/wordmark/kudzu-wordmark-echo-small.png` from that SVG on the design Mac
at its SVG canvas aspect ratio, with transparency. Inspect it, then run
`scripts/update-design-assets.sh` to rebuild the DMG background and manifest.
Commit the SVG, bitmap, background and manifest together. The existing renderer
consumes this bitmap; it does not rasterize the SVG automatically.

## Owner-only maintenance

`set-github-secrets.sh --dry-run` lists secret names. `--apply` uploads explicit
shell environment values to `GITHUB_REPOSITORY`; it never discovers credentials
from local stores. `cloudkit-schema.sh` merges only missing record types into
Development; review and deploy Production schema through CloudKit Console.
`update-cask.sh VERSION SHA256 TAP_DIR` writes the cask into the supplied checkout,
with no WebKit or HTTPStorages zap entries. It never commits or pushes.
