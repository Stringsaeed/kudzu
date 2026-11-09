# Releasing Kudzu

The shipping app is native. Bundle id `sh.saeed.kudzu`, name `Kudzu.app`, executable `Kudzu`, team `V3HN8HXZYK`, minimum macOS 14. Debug stays `Kudzu Dev` / `sh.saeed.kudzu.dev`. None of those change: TCC grants and the updater's designated requirement are tied to them.

`scripts/build-app.sh` is the packager. `UNSIGNED=1` makes an ad-hoc package on this Mac. CI calls the same script with Developer ID, notarization, and the updater key.

## What ships

```
tag vX.Y.Z
  → scripts/build-app.sh
      Kudzu.app          universal, hardened runtime, Developer ID
      Kudzu_X.Y.Z_universal.dmg
      Kudzu_X.Y.Z_universal.app.tar.gz
      Kudzu_X.Y.Z_universal.app.tar.gz.sig    base64 of a minisign file
      latest.json
  → GitHub release (draft, then published)
  → beta.json on the rolling update-feed release, when this version is newer
  → Homebrew cask, stable channel only
```

Feeds stay where installed copies already look:

- Stable: `https://github.com/Stringsaeed/kudzu/releases/latest/download/latest.json`
- Beta: `https://github.com/Stringsaeed/kudzu/releases/download/update-feed/beta.json`

Both platform keys (`darwin-aarch64`, `darwin-x86_64`) point at one universal archive. The archive contains `Kudzu.app/Contents/...` and nothing above that. The previous updater drops the first path component and treats the rest as the bundle. The native installer untars and takes the first `.app`. One `Kudzu.app` wrapper satisfies both.

The `.sig` file is the base64 encoding of a minisign text file, not a second signature. `latest.json`'s `signature` field is that text. The trusted comment is `timestamp:<unix>` then a tab then `file:<basename>`. The public key embedded in the app is unchanged (key id `6E52269F4241F007`). Do not rotate it.

## Version

Xcode reads `MARKETING_VERSION` from `Kudzu/Version.xcconfig`. release-please updates its marked line and the simple strategy's `version.txt` together; packaging refuses mismatched values. The manifest in `.release-please-manifest.json` is `apps/macos` at `0.6.0`, continued from the last shipping release.

A fix-only bump from that manifest would be `0.6.1`. Put a `Release-As: 0.7.0` footer in the migration commit body that touches `apps/macos` before release-please runs. Inspect the generated PR's manifest, version.txt, xcconfig, changelog and tag together. Changing an existing PR's title or body does not regenerate its version files. Signed packaging rejects native versions below `0.7.0` and prereleases on the stable channel; local ad-hoc builds may use the current development version.

Betas do not edit the file. The beta workflow sets `CHANNEL=beta` and `VERSION` from the tag, and `build-app.sh` stamps that build only.

`CFBundleVersion` is `CURRENT_PROJECT_VERSION`. CI sets it to the workflow run number. Local builds use `1`.

## Day to day

Commits that touch `apps/macos/` use Conventional Commits (`feat:`, `fix:`, `perf:`). release-please opens `chore: release x.y.z`. Merging it tags `vX.Y.Z` and opens a draft GitHub release. `.github/workflows/release.yml` builds, signs, notarizes, uploads, publishes, and bumps the cask.

The required GitHub check is the CI job `macos`. Rename the branch-protection rule that still says `desktop`.

## Cutover from the previous app

The native package and the previous package share the bundle id, the signer, the data folder, and the feed. The installer refuses an update whose `CFBundleExecutable` is `kudzu-desktop`, whose short version does not match the feed, or whose version is not strictly newer. The feed version and `CFBundleShortVersionString` are the same string. `0.7.0` is newer than the shipping `0.6.0`.

On the first launch, a missing `native-app` marker in Application Support writes a `preUpdate` backup even when the version string did not change, then writes the marker only if that backup succeeded. A same-version hop still backs up. The marker is not a substitute for the installer's version check.

`Contents/MacOS/kudzu-desktop` is a symlink to `Kudzu`, sealed in the signature. MCP clients registered as `kudzu-desktop --mcp` still start. New registrations use `Kudzu --mcp`. The skill is `Contents/Resources/skills/kudzu/SKILL.md`.

The old login item could leave a LaunchAgent. Removing it when the user toggles the login item is app behaviour, not something the packager does. Confirm that path before calling the cutover done.

An ad-hoc local build does not satisfy a Developer ID install. `codesign` on the ad-hoc app is not evidence of notarization.

## iCloud

Automatic SwiftData mirroring explicitly uses the fresh candidate
`iCloud.sh.saeed.kudzu.library`. It must never attach to the legacy custom schema
in `iCloud.sh.saeed.kudzu`. The candidate has not been provisioned by this work.
`Kudzu.icloud.entitlements`, profile validation and signed-package validation name
the new destination. No matching profile is supplied or fabricated. Unsigned and
Dev builds remain explicitly local-only.

The old `cloudkit-schema.sh` fails closed: its custom record definitions do not
belong in a SwiftData container. Initialize Development using an isolated signed
build with the actual SwiftData schema, test two Macs and accounts/assets, then
promote Development to Production through CloudKit Console. Production is add-only.

LibraryMigration copies the private directory and UI defaults, imports into a
separate local store, reopens and compares before writing `active-library.json`.
Original JSON, local state, blobs and old sync evidence remain in place. The receipt
is local. One primary Mac seeds. Secondary Macs explicitly use `--join-library`
on first cutover; this retains their originals but does not import their library.
Never use this option as a substitute for deciding which Mac seeds.

Settings changes to iCloud require restart because ModelConfiguration is selected
at open. “Check iCloud status” checks supported account state; it cannot force a
framework sync. Framework activity is not a guarantee of delivery to other Macs.
See docs/SWIFTDATA_INTEGRATION.md for verification limits and live gates.

## Design

`scripts/design/check-design-assets.py` runs at the start of `build-app.sh` and in CI. The DMG background is `design/dmg/background.png` (1320×880 at 144 dpi). The script copies that file into the image and compares the bytes. It does not re-encode it.

The window is 660×440, origin `{10, 60}` (bounds `{10, 60, 670, 500}`), icon size 128, text size 16, arrangement not arranged, toolbar and status bar hidden. `Kudzu.app` is at `(170, 220)` with its extension hidden. The Applications symlink is at `(490, 220)`. Finder writes this through AppleScript. A layout miss, a missing `.DS_Store`, a `SetFile` failure, or a `bless` failure fails the build. There is no skip flag. If a volume named Kudzu is already mounted, the script stops so Finder cannot edit the wrong disk.

## Local ad-hoc package

```sh
cd apps/macos
UNSIGNED=1 DERIVED_DATA=/private/tmp/kudzu-dd-release DIST=/private/tmp/kudzu-dd-release/dist bash scripts/build-app.sh
```

This does not read the updater key, does not call `notarytool`, and does not apply iCloud entitlements. The script says the result is ad-hoc. Publishing the stamped `0.6.0` archive would not install over the shipping app.

## Signed package

CI sets `APPLE_CERTIFICATE` (base64 p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_P8`, `APPLE_PROVISIONING_PROFILE`, and `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. `HOMEBREW_TAP_TOKEN` pushes the cask. The repo variable `REQUIRE_ICLOUD` gates a release without the profile.

The script imports the p12 into a throwaway keychain, signs nested code inside out with the hardened runtime and a timestamp, notarizes a zip of the app, staples the app, signs and notarizes the DMG, and checks `spctl`. `SKIP_NOTARIZE=1` stops after the Developer ID signature.

The workflow installs `minisign` with Homebrew when missing. Local releases need `brew install minisign` too. The private key remains `~/.tauri/kudzu-updater.key` for local secret setup (`scripts/release/set-github-secrets.sh`). `sign-update.sh` accepts that file path, native minisign text, or the existing Tauri base64 envelope, passes the password on stdin, and writes the base64 `.sig` next to the archive. It never rewrites the registered key.

`scripts/release/set-github-secrets.sh` uploads those secret names. It does not generate a new updater key. Do not run it as part of a build.

## When something goes wrong

- **Design check failed.** Fix the asset or the manifest under `design/`. Do not bypass the check.
- **Not universal.** `lipo -archs` on `Contents/MacOS/Kudzu` must list `arm64` and `x86_64`.
- **Executable is `kudzu-desktop`.** The installer will reject the update. The symlink may exist; `CFBundleExecutable` may not be that name.
- **Designated requirement mismatch.** The candidate must satisfy the installed app's requirement. An ad-hoc build, or a signature from a different team, will not install over the shipping app.
- **Profile certificate mismatch.** macOS will refuse to open an app whose restricted entitlements were signed by a certificate the profile does not list. Gatekeeper does not catch this. The script compares SHA-1 fingerprints.
- **Finder layout (`-1728`, `-1743`).** AppleScript could not talk to Finder, or Finder lost the disk. The app and the updater archive are already written. Detach a stuck Kudzu volume and run again. Do not add a skip.
- **`minisign` missing or the trusted comment rejected.** Install `minisign` on the runner. Keep the comment format above. Do not substitute another signature scheme.
- **Feed version equals the installed version.** The installer requires a strictly newer version. Use `Release-As: 0.7.0` for the first native release.
- **Legacy LaunchAgent still loaded.** That cleanup is in the app's login-item setting, not in this script. Check it on a machine that used the previous login item.
