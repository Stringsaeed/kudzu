# Kudzu — agent guide

Kudzu is a native macOS text expander. The app lives in `apps/macos`. The marketing site lives in `apps/website`. Read `apps/macos/README.md`, `apps/macos/docs/TEMPLATE_SYNTAX.md`, and `apps/macos/RELEASING.md` before changing behaviour.

## Layout

- The root is a pnpm workspace with Turborepo. `apps/website` is a Next.js package. `apps/macos` is the Swift app and is not a pnpm package. Run `pnpm install` at the root.
- `packages/oxlint-config` and `packages/typescript-config`: shared website checks. Use Oxlint for linting and TypeScript for type checks.
- `apps/macos/Packages/KudzuKit`: the Swift package. `KudzuCore` holds types, the template parser, matching, interchange, and sync (`Sources/KudzuCore/Sync/Sync.swift`). `KudzuEngine` is keyboard input and insertion.
- `apps/macos/Kudzu`: the app target. The version is `Kudzu/Version.xcconfig`. `Kudzu.xcodeproj` is generated from `project.yml`.
- `apps/website`: the Next.js site for kudzu.site. See `apps/website/README.md`.

## Commands

```sh
pnpm install
pnpm dev:website            # http://localhost:3001
pnpm dev:app                # apps/macos/scripts/dev.sh (Kudzu Dev)
pnpm check                  # website lint and types
pnpm build:website

cd apps/macos/Packages/KudzuKit && swift test
```

CI (`.github/workflows/ci.yml`) checks the website, runs `swift test`, and compiles the Release app. Signing and notarization stay in `apps/macos/scripts/build-app.sh`.

## Conventions

- **Commit messages** follow Conventional Commits (`feat:`, `fix:`, `perf:`, …). release-please reads commits that touch `apps/macos/` and writes `apps/macos/CHANGELOG.md`. The manifest is `0.6.0`; put `Release-As: 0.7.0` in the migration commit body before release-please runs.
- **Library JSON is a contract.** Make additive changes only: new fields are optional or defaulted, so old files and other Macs still decode them.
- **Sync.** Follow `KudzuCore/Sync/Sync.swift` and `apps/macos/RELEASING.md`. New fields are free. Bump `SYNC_VERSION` only when an older build would misread the change. CloudKit Production is add-only.
- **Restricted entitlements.** They live only in `apps/macos/Kudzu/Kudzu.icloud.entitlements`. `scripts/build-app.sh` applies that file when `Kudzu.provisionprofile` matches this app. Do not set them as `CODE_SIGN_ENTITLEMENTS` in `project.yml`.
- **Generated project.** Do not hand-edit `Kudzu.xcodeproj`. After `project.yml` changes, run `xcodegen generate` in `apps/macos`.
- **Tests.** Swift Testing tests sit next to the modules. Golden fixtures under `Tests/KudzuCoreTests/Fixtures` stay frozen.
- **Dependencies.** Do not add third-party dependencies without asking.
