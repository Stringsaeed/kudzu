# Kudzu — agent guide

Kudzu is a macOS text expander. The app lives in `apps/desktop/`: a Tauri 2 shell, a Rust engine, and a Next.js + shadcn/ui interface. Read `apps/desktop/README.md` (layout and features), `apps/desktop/docs/TEMPLATE_SYNTAX.md` (the snippet format) and `apps/desktop/RELEASING.md` (releases, updates, iCloud sync) before changing behaviour.

## Layout

- The root is a pnpm workspace with Turborepo. `apps/desktop/` and `apps/website/` keep separate app configs; dependencies resolve through the root lockfile. Run installs at the root.
- `packages/oxlint-config` and `packages/typescript-config`: shared frontend checks. Use Oxlint for linting and TypeScript for type checks.
- `apps/website/`: the Next.js App Router marketing site for kudzu.site. See `apps/website/README.md` for its components and Vercel settings.
- `apps/desktop/src-tauri/crates/kudzu-core`: pure Rust, with no OS APIs. Types, the template parser and renderer, the trigger matcher, Espanso and JSON interchange, backups, dedupe, and the iCloud sync model (`sync.rs`). Anything that can be unit-tested goes here.
- `apps/desktop/src-tauri/crates/kudzu-engine`: system integration. The CGEventTap, insertion, permissions and the caret position.
- `apps/desktop/src-tauri/src`: the Tauri shell:
  - store and IPC commands
  - windows and tray
  - updater (`updater.rs`)
  - CloudKit layer (`cloud/`)
- `apps/desktop/app`, `components`, `lib`: the UI.
  - `components/ui` holds shadcn primitives, `components/app` holds Kudzu's own components.
  - `lib/mock.ts` is an in-memory backend for the browser preview (`pnpm dev`).

## Commands

```sh
pnpm install                # root workspace install
pnpm dev                    # desktop browser preview + website
pnpm dev:app                # full Tauri development app
pnpm check                  # type-check and lint both frontends
pnpm build                  # both frontend builds, cached by Turbo

cd apps/desktop
pnpm dev                    # UI in a browser, mock backend (?view=settings:about, ?light, ?update, …)
pnpm tauri dev              # full app
pnpm typecheck && pnpm lint
cd src-tauri && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) installs at the root, checks and builds both frontends through Turbo, then runs the design checks and Rust checks. Native Tauri builds and signing stay outside Turbo caching.

## Conventions

- **Commit messages** follow Conventional Commits (`feat:`, `fix:`, `perf:`, …). release-please reads them to version releases and write the changelog. Only commits touching `apps/desktop/` count.
- **Library JSON is a contract.** Make additive changes only: new fields are optional or defaulted, so old files and other Macs still decode them.
- **Sync.** Follow the rules in `kudzu_core::sync` and RELEASING.md:
  - new fields are free (they're JSON keys inside `payload`);
  - bump `SYNC_VERSION` only for changes older builds would misread;
  - CloudKit's Production schema is add-only.
- **Restricted entitlements.** Never add iCloud (or other restricted) entitlements to `tauri.conf.json`; they live in `tauri.icloud.conf.json` and only apply with a provisioning profile.
- **Never edit generated files:** `src-tauri/gen/` and the `AGENTS.md` block that `next dev` writes.
- **Tests:** Rust tests sit next to the code (`#[cfg(test)]`) or in `crates/*/tests`. Write them first for core logic.
- **UI:** shadcn components plus the depth tokens in `app/globals.css` (`shadow-card`, `shadow-raised`, `shadow-well`, …). Shortcuts render with `Shortcut`, which is built on shadcn `Kbd`.
- **Dependencies:** don't add third-party dependencies without asking.
