<img src="assets/icon.svg" width="64" height="64" alt="">

# Kudzu

A lightweight, native, free and open-source text expander for macOS.

Type `;sig` anywhere and get your full signature. Kudzu watches for triggers you define and replaces them with snippets — dates, clipboard contents, fill-in forms, nested snippets — in any app.

Kudzu exists because the good text expanders cost a subscription and the free ones make you edit YAML. It aims for Espanso's power with a real Mac UI.

## Status

Planning. See [`docs/PLAN.md`](docs/PLAN.md) for decisions and milestones, and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how it fits together.

## Highlights (planned v1)

- Native Swift / SwiftUI menu-bar app, Raycast-style main window
- Visual snippet editor with inline variable chips (`/` to insert) and live preview
- Reliable insertion: keystrokes for short text, clipboard-paste-and-restore for long text, per-app override
- Forms that appear at your cursor
- Fuzzy search palette (⌃⌥Space)
- App-specific rules, groups, multiple triggers per snippet
- iCloud sync between Macs, usage stats, daily local backups
- Import from Espanso; export to Kudzu JSON or Espanso YAML

## Requirements

macOS 15 Sequoia or later.

## License

[MIT](LICENSE)
