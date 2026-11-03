# Releasing Kudzu

Kudzu ships as a signed, notarized **universal DMG** on [GitHub Releases](https://github.com/Stringsaeed/kudzu/releases). Installed copies **update themselves** from the same release, and **Homebrew** gets a cask bump automatically:

```sh
brew install --cask stringsaeed/tap/kudzu
```

## How a release happens

```
feat:/fix: commits on main
        │
        ▼
release-please keeps a "chore: release x.y.z" PR open
(bumps desktop/package.json, writes desktop/CHANGELOG.md)
        │  you merge it
        ▼
tag vX.Y.Z + draft GitHub release
        │
        ▼
.github/workflows/build-macos.yml  (macos-15)
  build universal app ─► sign (Developer ID, hardened runtime) ─► notarize + staple app
  ─► DMG: sign, notarize, staple ─► updater archive + minisign signature ─► latest.json
        │
        ▼
upload assets ─► publish the release (now "latest") ─► bump Casks/kudzu.rb in homebrew-tap
```

The release stays a **draft until every asset is uploaded**, so the updater and Homebrew never see a half-built release.

### What users see

1. Kudzu checks about 20 seconds after launch, then every 6 hours. **Settings → About → Check now** checks straight away.
2. A new version **downloads quietly** and is verified against the public key built into the app.
3. Once it's ready:
   - the menu-bar glyph gets a dot, and the menu reads **"Restart to Update to x.y.z"**;
   - the main window shows a toast with **Restart**;
   - About shows the release notes.
4. The update **installs when the user quits Kudzu**, or right away on **Restart**. Kudzu never restarts on its own, because a text expander restarting mid-sentence drops the trigger being typed.
5. On the first launch of a new version, Kudzu saves a **"Before update" backup** of the library before anything can migrate it. It's listed under Settings → Backups & data.
6. If Kudzu runs straight from the DMG, or from a quarantined copy that was never moved, it says **"Move Kudzu to your Applications folder"** instead of failing silently.

Turning on **Get beta updates** switches to the beta feed. That feed always points at whichever is newer, the latest beta or the latest stable, so beta testers never fall behind. Switching back to stable leaves a beta install where it is until a stable release passes it, because the updater never downgrades.

### Cutting a beta

Run **Actions → Beta → Run workflow** with a version like `0.3.0-beta.1` and the branch or commit to build. CI:

1. tags that commit;
2. builds and notarizes it exactly like a stable release, with the version stamped into the build only (no commit);
3. publishes it as a **pre-release**, which stable users and Homebrew ignore;
4. points `beta.json` on the rolling **`update-feed`** release at it.

Use betas for anything risky, sync-format changes above all: run them on your own Macs first.

### Day to day

1. Write commit messages in [Conventional Commits](https://www.conventionalcommits.org) style. Only commits that touch `desktop/` count.

   | Commit | Effect before 1.0 | Effect after 1.0 |
   |---|---|---|
   | `fix: …`, `perf: …` | patch, 0.2.0 → 0.2.1 | patch |
   | `feat: …` | minor, 0.2.0 → 0.3.0 | minor |
   | `feat!: …` or `BREAKING CHANGE:` | minor | major |
   | `docs:`, `chore:`, `refactor:`, `test:`, `ci:` | no release, hidden from the changelog | same |

   With merge commits (the repo's current style), every commit in the PR is read. With squash merges, the PR title is what counts, so title PRs the same way.

2. When you're ready to ship, open the **"chore: release x.y.z"** PR. Read the changelog, edit it if you like, and merge.
3. Watch **Actions → Release**. A run takes about 20–30 minutes, most of it compiling two architectures and waiting on Apple's notary service. When it's green:
   - the release is public;
   - running copies find it within six hours, or straight away with **Settings → About → Check now**, or **Check for Updates…** in the menu-bar menu;
   - `brew upgrade` picks it up.

To force a specific version, add `Release-As: 1.0.0` to a commit body.

### Release files

| File | Used by |
|---|---|
| `Kudzu_X.Y.Z_universal.dmg` | people downloading and Homebrew (signed, notarized, stapled) |
| `Kudzu_X.Y.Z_universal.app.tar.gz` and `.sig` | the in-app updater; the archive is verified against the public key built into the app |
| `latest.json` | the updater feed at `releases/latest/download/latest.json` |
| `SHA256SUMS.txt` | checksums |

The rolling **`update-feed`** pre-release holds `beta.json`. It's machine-readable plumbing, not something to download. Don't delete it, or beta users stop getting updates.

## One-time setup

### 1. GitHub secrets

Run this once, after collecting the items below:

```sh
desktop/scripts/release/set-github-secrets.sh \
  --p12 ~/Desktop/DeveloperID.p12 \
  --p8 ~/Downloads/AuthKey_XXXXXXXXXX.p8 --key-id XXXXXXXXXX --issuer xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

| Secret | What it is | Where to get it |
|---|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | Developer ID Application certificate and private key | Keychain Access → My Certificates → right-click **Developer ID Application: Muhammed Saeed (V3HN8HXZYK)** → Export… as a `.p12` with a password. The script derives the identity name. |
| `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_P8` | App Store Connect API key, used to notarize | [App Store Connect → Users and Access → Integrations → App Store Connect API](https://appstoreconnect.apple.com/access/integrations/api) → **+**, role **Developer**. Download the `.p8` (only possible once) and note the Key ID and Issuer ID. |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | updater signing key | already generated: `~/.desktop/kudzu-updater.key`, with its password in the login keychain under "Kudzu updater signing key password". The script reads both. |
| `HOMEBREW_TAP_TOKEN` | lets CI push to the tap | [Fine-grained PAT](https://github.com/settings/personal-access-tokens/new): repository **Stringsaeed/homebrew-tap**, permission **Contents: Read and write** |
| `APPLE_PROVISIONING_PROFILE` | Developer ID provisioning profile for `sh.saeed.kudzu` with iCloud | see step 2; the script base64-encodes it |
| `RELEASE_PLEASE_TOKEN` (optional) | lets CI run on the release PR | fine-grained PAT on **Stringsaeed/kudzu** with Contents and Pull requests set to Read and write. Without it the release PR has no CI checks, because GitHub doesn't run workflows on PRs opened by `GITHUB_TOKEN`. |

> [!WARNING]
> **Back up the updater key and its password** (for example in 1Password). Every installed copy trusts only this key. If it's lost, existing users can't be updated and have to reinstall by hand.

### 2. iCloud sync (provisioning profile and schema)

iCloud is a restricted capability. A notarized DMG can use it, but only with an **embedded Developer ID provisioning profile**. macOS kills a signed app that claims iCloud without one, so the build script only adds the iCloud entitlements when the profile is present.

1. **Profile:** [developer.apple.com → Profiles → +](https://developer.apple.com/account/resources/profiles/add):
   - choose **macOS → Developer ID**;
   - App ID **sh.saeed.kudzu**, which already has iCloud with the `iCloud.sh.saeed.kudzu` container from the Swift app;
   - your Developer ID certificate.

   Download it, then pass it to `set-github-secrets.sh --profile …`, which uploads it as `APPLE_PROVISIONING_PROFILE`.
2. **Schema in Production:**
   - The record types are `KudzuSnippet`, `KudzuGroup`, `KudzuUsage` and `KudzuMeta`, each with a single `payload` string.
   - They already exist in the **Development** environment, created by a test build.
   - Deploy them once: [CloudKit Console](https://icloud.developer.apple.com) → `iCloud.sh.saeed.kudzu` → **Schema → Deploy Schema Changes…**.
   - If the Development schema is ever reset, `scripts/release/cloudkit-schema.sh` adds the types back without touching the Swift app's `CD_…` types.
   - Because every app field lives inside `payload`, **adding fields never needs another schema deploy**.
3. **Make releases require it:** Settings → Secrets and variables → Actions → **Variables** → `REQUIRE_ICLOUD` = `1`. From then on a release fails rather than shipping without sync.

#### Rules for changing sync

These are enforced in `kudzu_core::sync` and its tests. Keep them:

- **New snippet or group fields are free.** They become JSON keys. Older Macs keep keys they don't know, so they never erase data written by newer ones.
- **Bump `SYNC_VERSION` only for changes older builds would misread**, such as renaming or re-meaning a field. Macs on an older format then stop sending and show "Another Mac uses a newer Kudzu. Update to keep syncing". Ship such a change as a **beta** first, and make sure the stable release that bumps it is out before many people depend on it.
- **CloudKit schema changes are add-only in Production.** Record types and fields can never be removed or renamed. That's one more reason everything lives inside `payload`.

### 3. Repository settings

- **Settings → Actions → General → Workflow permissions:** "Read and write", plus "Allow GitHub Actions to create and approve pull requests". release-please needs both.
- The repo must be **public**: the updater and Homebrew download release assets without credentials.

## Building locally

`scripts/release/build.sh` is the same script CI runs. Use a certificate **SHA-1** for the identity: this Mac has several identically named Developer ID certificates, so `codesign` rejects the name as ambiguous.

```sh
cd desktop
export APPLE_SIGNING_IDENTITY=$(security find-identity -v -p codesigning | awk '/Developer ID Application/{print $2; exit}')
export TAURI_SIGNING_PRIVATE_KEY=~/.desktop/kudzu-updater.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(security find-generic-password -a kudzu-updater -s "Kudzu updater signing key password" -w)

SKIP_NOTARIZE=1 pnpm release:build            # signed, not notarized (quick check)

export APPLE_API_KEY=XXXXXXXXXX APPLE_API_ISSUER=xxxxxxxx-… APPLE_API_KEY_PATH=~/keys/AuthKey_XXXXXXXXXX.p8
pnpm release:build                            # the full thing, into desktop/dist/
```

## When something goes wrong

- **The build failed:** fix it on `main`, then **Actions → Release → Run workflow** with the tag (for example `v0.3.0`). The draft release is reused, and uploads use `--clobber`.
- **A bad version shipped:** don't delete the release. Installs that already updated keep the bad build, and deleting breaks `latest`. Ship a fix instead: commit `fix: …`, merge the release PR, and the next patch replaces it. To stop new updates immediately, delete only the `latest.json` asset from the release; copies then treat the check as failed and stay where they are.
- **Notarization was rejected:** run `xcrun notarytool log <submission-id> --key … --key-id … --issuer …` to see why. The usual cause is an unsigned binary, or a missing hardened runtime on a helper.

## Versions and identity

- **Version:** `desktop/package.json` is the single source. `tauri.conf.json` reads it (`"version": "../package.json"`). The Rust crate versions are internal and don't change.
- **Identifier:** `sh.saeed.kudzu`, the same as the original Swift app. Kudzu replaces it, which is why the Swift app is no longer released. Data lives in `~/Library/Application Support/sh.saeed.kudzu/`. On first launch, data from earlier pre-release builds (`sh.saeed.kudzu.desktop`) is copied over.
- **Privacy permissions:** Accessibility and Input Monitoring grants are tied to the bundle ID and the signing team. Updates signed by the same Developer ID keep them; switching from an ad-hoc local build to a signed build means granting them once more.
- **macOS:** 14 Sonoma or later (iCloud sync uses `CKSyncEngine`), universal (Apple Silicon and Intel).
- **Logs:** `~/Library/Logs/sh.saeed.kudzu/Kudzu.log` (rotated, about 2 MB × 3). Ask users for it when sync or updates misbehave.
