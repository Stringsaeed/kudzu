<img src="apps/desktop/design/icon.svg" width="64" height="64" alt="">

# Kudzu

A fast, keyboard-first, free and open-source text expander for macOS.

Type `;sig` anywhere and get your full signature. Kudzu watches for triggers you define and replaces them with snippets — dates, clipboard contents, fill-in forms, nested snippets — in any app.

Kudzu exists because the good text expanders cost a subscription and the free ones make you edit YAML. It aims for Espanso's power with a real Mac UI.

## Install

```sh
brew install --cask stringsaeed/tap/kudzu
```

Or download the DMG from [Releases](https://github.com/Stringsaeed/kudzu/releases). Kudzu updates itself; Settings → About has a beta channel.

Requires macOS 14 Sonoma or later (Apple Silicon or Intel).

## Highlights

- Menu-bar app with a quick-search tray (⌃⌥Space) and a full library window
- Snippet editor with inline tag chips (`/` to insert) and a live preview
- Reliable insertion: typing for short text, paste-and-restore for long text, per-app overrides
- Fill-in forms that appear at the caret
- Groups, favorites, multiple triggers per snippet, app rules
- iCloud sync between Macs, usage stats, daily local backups
- Import from Espanso; export to Kudzu JSON or Espanso YAML
- No accounts, no analytics, no servers of its own

## Development

The repository is a pnpm workspace managed by Turborepo. Use Node.js 22.12 or later and the pnpm version pinned in the root `package.json`.

```sh
pnpm install             # install both applications from the root lockfile
pnpm dev                 # desktop browser preview :3000 + website :3001
pnpm dev:website         # website only
pnpm dev:desktop         # desktop browser preview only
pnpm dev:app             # native Tauri development app (macOS)
pnpm check               # types and lint for both applications
pnpm build               # desktop static export + website production build
pnpm test:rust           # native workspace tests (macOS)
pnpm lint:rust           # native workspace Clippy checks (macOS)
```

| Workspace | Purpose |
| --- | --- |
| [`apps/desktop/`](apps/desktop) | Tauri 2, Rust engine, Next.js + shadcn/ui interface |
| [`apps/website/`](apps/website) | Next.js App Router marketing site for kudzu.site |
| [`packages/oxlint-config/`](packages/oxlint-config) | Shared Oxlint rules for both apps |
| [`packages/typescript-config/`](packages/typescript-config) | Shared strict Next.js TypeScript configuration |

Both apps keep their own framework configuration and dependencies. Turborepo caches frontend tasks; native app builds and signing run through the existing Tauri scripts. There is one root `pnpm-lock.yaml` and one root `pnpm-workspace.yaml`. Run installs at the root and add an app dependency with `pnpm --filter kudzu-desktop add …` or `pnpm --filter kudzu-website add …`.

See [`apps/desktop/README.md`](apps/desktop/README.md) to build and run the app, [`apps/desktop/RELEASING.md`](apps/desktop/RELEASING.md) for releases, updates and iCloud sync, and [`apps/website/README.md`](apps/website/README.md) for the site and Vercel setup.

## License

[MIT](LICENSE)
