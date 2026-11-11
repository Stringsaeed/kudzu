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
- `cargo xtask set-version 0.3.0-beta.1`: stamp `version.txt` only.

`build.sh` delegates to xtask; its internal preflight/verification modes enforce
artwork, profile, certificate, entitlements, both architectures, minimum macOS,
Gatekeeper and stapling checks. `SKIP_NOTARIZE=1` skips only Apple submission,
stapling and Gatekeeper assessment. With `APPLE_SIGNING_IDENTITY=-` and a
throwaway wrapped updater key, it builds ad-hoc artifacts; those are not usable
by the production installer. `--dry-run` never produces artifacts.

Install both Rust Darwin targets and create-dmg **1.2.3** (the workflow pins its
source commit). The CI runner must have a logged-in Finder session for layout.
No fallback skips Finder layout. A failed build may leave `dist/`; move it aside
before retrying. All cargo invocations use this workspace's `target/`.

## Owner: real candidate evidence before S75

1. Review and merge S70 into the rebuild branch. Make the dispatch-only workflow
   available to GitHub Actions, select the rebuild branch, and verify its exact SHA.
   No workflow was dispatched by the implementation agent.
2. Confirm the existing CI secrets are configured, without printing their values:
   `APPLE_CERTIFICATE` (base64 P12), `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_P8`,
   `APPLE_PROVISIONING_PROFILE`, `TAURI_SIGNING_PRIVATE_KEY`, and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The last two map to `KUDZU_UPDATE_KEY`
   and `KUDZU_UPDATE_KEY_PASSWORD`. The updater key is base64-wrapped minisign
   key text, not a path. No new production key is generated.
3. Dispatch **Desktop v2 release candidate**. Require success for signing,
   notarization, stapling, `spctl` on both artifacts, both slices' `minos 14.0`,
   profile certificate fingerprint, and temporary keychain cleanup. Save the run
   URL and logs. The artifacts attach only to the run; the feed URL describes the
   future release location and is not live.
4. Download the run artifacts and run `shasum -a 256 -c SHA256SUMS.txt` in their
   directory. Mount the DMG and visually verify its background, 660×440 window,
   Kudzu at (170,220), Applications at (490,220), and drag installation.
5. For an interrupted runner, stop the release process before invoking
   `cargo xtask release cleanup`. Cleanup failure is a failed run, not evidence
   of successful keychain removal. A killed runner needs owner recovery before reuse.

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
