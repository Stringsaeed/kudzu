# Kudzu desktop app

The Kudzu app: **Tauri 2 + Next.js 16 (static export) + Tailwind v4 + shadcn/ui**, with the text-expansion engine and all logic in Rust. Dark first, keyboard first.

```
desktop/
├── app/                      Next.js routes, one per window
│   ├── page.tsx              main window (library, editor, insights, settings, onboarding)
│   ├── palette/page.tsx      quick-search tray (global shortcut)
│   └── form/page.tsx         fill-in form shown at the caret
├── components/
│   ├── ui/                   shadcn/ui primitives (radix-nova)
│   └── app/                  Kudzu components (editor, palette, form, main/*)
├── lib/                      client logic: IPC bridge, search ranking, template tokenizer, store
└── src-tauri/
    ├── src/                  Tauri shell: store, IPC commands, windows, tray, hotkey
    └── crates/
        ├── kudzu-core/       pure Rust: types, template parser/renderer, matcher, Espanso, backups
        └── kudzu-engine/     system-wide engine: CGEventTap, insertion, permissions, apps (macOS)
```

## Run

```sh
pnpm install
pnpm tauri dev          # Kudzu Dev (Rust + webviews, hot reload)
pnpm tauri:build:dev    # installable Kudzu Dev.app in src-tauri/target/release/bundle/macos/
pnpm dev                # UI only, in a browser, with an in-memory mock backend
pnpm tauri build        # production-identity .app + .dmg in src-tauri/target/release/bundle

cd src-tauri && cargo test --workspace   # core + engine tests
```

### macOS development variant

`pnpm tauri dev` and `pnpm tauri:build:dev` use `src-tauri/tauri.dev.conf.json` to build **Kudzu Dev** (`sh.saeed.kudzu.dev`). It can run beside the installed **Kudzu** (`sh.saeed.kudzu`): its library, settings, backups, logs, login item, and macOS permissions are separate. Dev data lives in `~/Library/Application Support/sh.saeed.kudzu.dev/`; logs are in `~/Library/Logs/sh.saeed.kudzu.dev/`.

A new dev library starts paused, with **⌃⌥⇧Space** for Quick Search instead of production's **⌃⌥Space**. Resume it when you want to test expansion; pause the other app first to avoid both expanding the same trigger. Dev never imports the old production library automatically, syncs to production iCloud, or installs production updates.

`pnpm tauri dev` selects the development variant through the package's Tauri wrapper. Calling `pnpm exec tauri dev` directly bypasses that wrapper and uses the production identity and data. `pnpm tauri build` and `pnpm release:build` retain the production identity.

The browser preview takes a few dev-only query params: `?view=insights`, `?view=settings:apps`, `?onboarding`, `?light`, `?update` (pretend a new version exists), and `/form?demo`.

## Release

Releases are cut by merging release-please's "chore: release x.y.z" PR. CI then builds, signs, notarizes and publishes a universal DMG with in-app auto-updates, and bumps the Homebrew cask. See [RELEASING.md](RELEASING.md) for the flow, one-time secrets, local builds and recovery.

## Features

| Area | What it does |
|---|---|
| **Expansion** | A global keyboard tap that matches in O(trigger length). Supports several triggers per snippet, instant or whole-word firing, case propagation (`;sig` / `;Sig` / `;SIG`), Backspace right after an expansion to undo it, and `{{cursor}}` placement. Short text is typed; long text is pasted and the clipboard is restored afterwards (marked transient/concealed). |
| **Templates** | Date/time (ICU formats, offsets, locale), clipboard (optional trim), cursor, nested snippets (cycle-safe, depth 8), fill-in fields and dropdowns. The stored format is defined in [`docs/TEMPLATE_SYNTAX.md`](docs/TEMPLATE_SYNTAX.md). |
| **Editor** | Tags appear as inline chips in a CodeMirror editor. Type `/` or use **Insert** to add variables; click a chip to configure it with a live example. Changes save automatically, a live preview shows the result, and collision warnings link to the conflicting snippet. |
| **Rules** | Per-snippet and per-group allow/deny app lists and insertion method, with inheritance (snippet ▸ group ▸ app setting ▸ default). Password managers are excluded by default, and terminals paste. |
| **Quick search** | Press <kbd>⌃⌥Space</kbd> anywhere. Fuzzy, usage-boosted ranking across name, trigger and content, with `#group` filters, Tab to change scope, a live preview, and <kbd>↵</kbd> insert, <kbd>⌘↵</kbd> copy, <kbd>⌘E</kbd> edit, <kbd>⌘1–9</kbd> pick. It can also create a snippet from your query. |
| **Forms** | A panel near the caret with a live preview. <kbd>Tab</kbd> moves between fields, <kbd>↵</kbd> inserts, and <kbd>Esc</kbd> cancels and leaves the trigger as typed. Fields remember their last values. |
| **Library** | Favorites, recently used, turned-off snippets and nested groups (drag to move, right-click to manage). The list is virtualized, supports multi-select bulk actions and type-to-filter, and deletes can be undone. |
| **Insights** | Expansions, characters and time saved (at a WPM you set), most-used, recently used and never-used snippets. Usage is per device, and typed text is never stored. |
| **Data** | Import from Espanso (auto-detected) or a Kudzu JSON file, and export to Kudzu JSON or Espanso YAML. A backup is taken daily and before every import or restore; the last 14 are kept and can be restored in one click. |
| **Shell** | Menu-bar app with status, pause, quick search, "new snippet from selection", open at login, and an optional Dock icon while the window is open. Onboarding walks through permissions (detected live), Espanso import, and a practice field. |

## Keyboard

Every action has a shortcut. <kbd>⌘K</kbd> opens the command menu and <kbd>⌘/</kbd> shows the cheat sheet.

<kbd>⌘N</kbd> new · <kbd>⌘F</kbd> filter · <kbd>⌘L</kbd> list · <kbd>⌘E</kbd> editor · <kbd>Esc</kbd> back to list ·
<kbd>↑↓</kbd>/<kbd>J K</kbd> move · <kbd>⇧↑↓</kbd> extend selection · <kbd>⌘⌫</kbd> delete (undoable) · <kbd>⌘D</kbd> duplicate ·
<kbd>⌘⇧F</kbd> favorite · <kbd>⌘⇧E</kbd> on/off · <kbd>⌘⇧P</kbd> pause · <kbd>⌘1–4</kbd> views · <kbd>⌘,</kbd> settings.

## Performance notes

- **Keystroke path (Rust):** reversed-trigger suffix tries, so the cost per key depends on the longest trigger, not the library size. Insertion runs on a worker thread, never in the tap callback.
- **Search:** with 5,000 snippets, each keystroke takes 1–4 ms and an index refresh after usage changes takes about 1 ms (folded fields are cached per edit stamp). `useDeferredValue` keeps typing responsive.
- **Windows:** the palette and form webviews are created hidden at launch, so the shortcut only shows an existing window. The palette-specific JavaScript is about 7 KB gzipped on top of the shared runtime.
- **List:** virtualized with TanStack Virtual. Row summaries are computed lazily and cached by content.
- **Disk:** writes are atomic (temp file + rename). A corrupt library is moved aside, never overwritten.

## Design

- **Theme:** dark first, with a light theme and System option. The palette is a shadcn theme (stone neutrals, blue primary) in `app/globals.css`; Kudzu's extra tokens (surfaces, hairlines, the `iris` accent) derive from it.
- **Accent:** one accent, *iris*, used only for focus, selection and tokens. It follows `primary` (the brighter `sidebar-primary` in dark mode).
- **Depth:** a small elevation scale (`shadow-card`, `shadow-raised`, `shadow-well`, …) that Tailwind's stock `shadow-*` and shadcn's components also map onto. On macOS the sidebar, palette and form use native vibrancy.
- **Type:** SF Pro (the macOS system face) for UI, Figtree as the non-macOS fallback, and Paper Mono for triggers and templates. Figtree and Paper Mono are bundled offline (OFL).
- **Motion:** short rise/pop animations that respect Reduce Motion.
