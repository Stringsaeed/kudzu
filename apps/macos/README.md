# Kudzu for macOS (native)

The native Swift/SwiftUI port of Kudzu. The plan, layout and rules are in
[`docs/MIGRATION.md`](docs/MIGRATION.md); per-feature parity notes are in `docs/parity/`.

- `Packages/KudzuKit`: every module (`KudzuCore`, `KudzuEngine`, `KudzuServices`, `KudzuDesign`,
  `KudzuEditor`, `KudzuApp`) and their tests.
- `Kudzu/`: the app target's entry point, Info.plist, entitlements, icon and fonts.
- `Kudzu.xcodeproj` is generated from `project.yml`.

## Run Kudzu Dev

Requires the full Xcode (not only the Command Line Tools).

```sh
cd apps/macos
scripts/dev.sh            # builds the Debug app into build/ and opens it
scripts/dev.sh --hidden   # start in the menu bar only, as the login item does
```

Kudzu Dev (`sh.saeed.kudzu.dev`) can run next to the release Kudzu, because it keeps its own
copy of everything:

- data in `~/Library/Application Support/sh.saeed.kudzu.dev/`, logs in
  `~/Library/Logs/sh.saeed.kudzu.dev/` (it never reads or writes `sh.saeed.kudzu`);
- its own Accessibility and Input Monitoring grants;
- its own hotkeys (⌃⌥⇧Space search, ⌃⌥⇧V clipboard) and the MCP server name `kudzu-dev`.

The script quits a running Kudzu Dev before it opens the new build. When it is launched from a
terminal, macOS may leave the window behind the active app; `open -b sh.saeed.kudzu.dev` brings
it to the front.

## Test

```sh
cd apps/macos/Packages/KudzuKit
swift build
swift test                          # everything
swift test --filter KudzuAppTests   # one bundle
```

Golden fixtures in `Packages/KudzuKit/Tests/KudzuCoreTests/Fixtures` are frozen at the 0.6.0 library. The generator that wrote them ships with the Rust tree and is not part of this app.

## Release

`scripts/build-app.sh` builds a universal Release `Kudzu.app` (bundle id `sh.saeed.kudzu`, executable `Kudzu`). `UNSIGNED=1` is an ad-hoc local package. A signed run uses Developer ID, notarizes, and writes the updater archive. See [`RELEASING.md`](RELEASING.md).

The version is `Kudzu/Version.xcconfig`. Design sources are `design/`. `scripts/design/check-design-assets.py` checks them. The DMG background is `design/dmg/background.png` and is copied into the disk image byte for byte.

`Contents/Resources/skills/kudzu/SKILL.md` is the bundled agent skill. `Contents/MacOS/kudzu-desktop` is a symlink to `Kudzu`, so an MCP client registered against the previous executable name still starts. `Info.plist` keeps `CFBundleExecutable` as `Kudzu`.

Parity notes in `docs/parity/` cite the Tauri sources as they were at tag `v0.6.0`.
