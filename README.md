<img src="desktop/design/icon.svg" width="64" height="64" alt="">

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

The app lives in [`desktop/`](desktop): Tauri 2 with a Rust engine and a Next.js + shadcn/ui interface. See [`desktop/README.md`](desktop/README.md) to build and run it, and [`desktop/RELEASING.md`](desktop/RELEASING.md) for how releases, updates and iCloud sync are shipped.

## License

[MIT](LICENSE)
