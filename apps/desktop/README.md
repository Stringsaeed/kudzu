# Kudzu desktop app

**For things worth repeating**

Clipboard history and text shortcuts for your Mac. Shared product copy lives in `design/brand.json`.

The Kudzu app: **Tauri 2 + Next.js 16 (static export) + Tailwind v4 + shadcn/ui**, with the text-expansion engine and all logic in Rust. Dark first, keyboard first.

```
apps/desktop/
├── app/                      Next.js routes, one per window
│   ├── page.tsx              main window (library, editor, insights, settings, onboarding)
│   ├── palette/page.tsx      quick-search tray (global shortcut)
│   ├── clipboard/page.tsx    clipboard history picker (global shortcut)
│   └── form/page.tsx         fill-in form shown at the caret
├── components/
│   ├── ui/                   shadcn/ui primitives (radix-nova)
│   └── app/                  Kudzu components (editor, palette, form, clipboard, main/*)
├── lib/                      client logic: IPC bridge, search ranking, template tokenizer, store
└── src-tauri/
    ├── src/                  Tauri shell: store, IPC commands, windows, tray, hotkey
    └── crates/
        ├── kudzu-core/       pure Rust: types, template parser/renderer, matcher, Espanso, backups, clipboard model
        └── kudzu-engine/     system-wide engine: CGEventTap, insertion, permissions, apps (macOS)
```

## Run

Install dependencies with `pnpm install` at the repository root. The app is the `kudzu-desktop` workspace; `pnpm dev:app` from the root starts Tauri.

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

The browser preview takes a few dev-only query params: `?view=insights`, `?view=clipboard`, `?view=settings:apps`, `?onboarding` (add `&espanso` to simulate a detected Espanso install, `&espanso=retry` for import recovery, or `&finish=retry` for a failed completion save; the Raycast and TextExpander rows import samples), `?light`, `?update` (pretend a new version exists), `/form?demo`, `/clipboard` (the picker), `?clips=5000` (fill clipboard history to its cap) and `?emptyclips`. `/permissions` previews the permission companion with controls for grant, revocation, pause, and Secure Input. Copying text in the preview adds it to the mock history.

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
| **Clipboard history** | Records every copy (text, code, links, colors, images, files and app-specific data) with the time and app it came from, and folds repeats into one item with a copy timeline. Kept for 1 week by default (1 day to 1 year, a custom number of days, or forever), pinned items never expire, and up to 5,000 items are kept. The sidebar's Clipboard section lists History, Pinned and a folder per kind (Text, Images, Links, Files, Code, Colors). Press <kbd>⌃⌥V</kbd> anywhere to search the history and paste into the app you were in. Previews for every kind: images, code with line numbers, colors with HEX/RGB/HSL/OKLCH values and contrast, links, and file lists with Finder icons. Passwords and anything marked concealed or transient are skipped, password managers are excluded, and the history never leaves the Mac. |
| **AI agents** | An MCP server, off by default, under **Settings ▸ AI agents**. It lets assistants like Claude Code search, read and expand snippets, create and edit them, and search clipboard history. Agents can't delete anything. Passwords, API keys, tokens and private keys are redacted before anything leaves the app. See [AI agents](#ai-agents-mcp). |
| **Insights** | Expansions, characters and time saved (at a WPM you set), most-used, recently used and never-used snippets. Usage is per device, and typed text is never stored. |
| **Data** | Import from Espanso (auto-detected), Raycast (`.json`), TextExpander (`.textexpander` or CSV) or a Kudzu JSON file, and export to Kudzu JSON or Espanso YAML. A backup is taken daily and before every import or restore; the last 14 are kept and can be restored in one click. |
| **Shell** | Menu-bar app with status, pause, quick search, "new snippet from selection", open at login, and an optional Dock icon while the window is open. Onboarding walks through a guided permission companion (draggable app icon, native Settings animation, and live detection), an import step (Espanso is detected; Raycast and TextExpander exports are chosen as files), and a practice preview. See [permission setup](docs/PERMISSIONS.md). |

## Formatted snippets

Choose **Formatted** in the **Plain text / Formatted** selector above the content editor. Select text to apply
bold, italic, underline, or a link. The toolbar also adds bulleted and numbered lines; their
markers remain visible in plain-text apps. **⌘B**, **⌘I**, and **⌘U** format a selection.
Template chips still work, including styling the value of a fill-in field.

Use **Image** to embed a PNG, JPEG, or WebP image, choose its display width, and give it
a text alternative. Images are converted to compact PNGs, with a maximum dimension of 512
pixels and up to 144 KiB of image data per image (384 KiB per snippet). They are stored inside
the snippet, so they work offline and travel with backups, Kudzu JSON exports, and iCloud sync.
Select an image and use **Remove formatting** to replace it with its text alternative, or
Backspace to delete it. Formatting changes and image deletion can be undone in the editor.

Formatted snippets paste HTML and plain text together. Apps choose the format they support;
an explicit **Typing** insertion rule uses plain text. Image alternatives appear as `[Label]`
in plain text. A formatted snippet also preserves formatting and images from nested snippets;
a plain-text snippet uses their plain-text versions.

The editor, quick-search palette, and fill-in forms preview formatting and images. JSON export
preserves them; Espanso export currently writes the plain-text template. Choose **Plain text**
in the format selector to remove formatting and retain image alternatives; undo restores it.

Image pastes use the target app's reported caret movement to distinguish real attachments from
plain-text alternatives. If the app does not report a matching result, Kudzu skips automatic
cursor repositioning and its immediate Backspace-to-trigger undo for that expansion. Normal
editing and the target app's own undo remain available.

Run `pnpm test:formatting` for editor-state, rendering, and native macOS HTML-import checks.
The native check uses AppKit without changing the system clipboard or opening user documents.

## AI agents (MCP)

Turn on **Settings ▸ AI agents ▸ Allow AI agents**. Agents then start `Kudzu --mcp`, a stdio MCP server that relays each JSON-RPC line to the running app over `agent.sock` in the data folder. The socket exists only while access is on and is owner-only. When the app can't be reached, the bridge still lists the tools and returns a message explaining how to turn access on. The settings page shows the `claude mcp add` command and the JSON configuration for this copy of the app. Development builds register as `kudzu-dev` and add `--dev`.

| Tool | Does |
|---|---|
| `list_snippets`, `get_snippet`, `list_groups` | Search and read the library |
| `expand_snippet` | Render a snippet now, with fill-in `values` |
| `create_snippet`, `update_snippet` | Write, when **Create and edit snippets** is on. Triggers can't collide, missing groups are created, and edits are saved, synced and backed up like any other. |
| `list_clipboard`, `get_clipboard_item` | Search clipboard history, when **Share clipboard history** is on |

**Secrets.** `kudzu_core::secrets` scans every value before it is sent. It looks for PEM keys, JWTs, vendor-prefixed tokens (`ghp_`, `sk-ant-`, `AKIA…`, `xoxb-`, …), labelled values (`password=`, `"api_key": …`, `Bearer …`), passwords in URLs, webhook URLs, and generated-looking words. Each match is replaced with `[redacted: …]`. Clipboard items that are entirely secret are withheld, and searches run on the redacted text, so a query can't confirm a secret. Content is never written back over a snippet that holds a secret. Short, plain passwords look like ordinary words, so detection is best effort.

The protocol and tools are in `crates/kudzu-core/src/agent/` and are tested with a fake host. The socket and bridge are in `src-tauri/src/agent.rs`. [`skills/kudzu/SKILL.md`](skills/kudzu/SKILL.md) is the Claude Code skill. **Install skill** in Settings copies it to `~/.claude/skills/kudzu/`.

## Keyboard

Every action has a shortcut. <kbd>⌘K</kbd> opens the command menu and <kbd>⌘/</kbd> shows the cheat sheet.

<kbd>⌘N</kbd> new · <kbd>⌘F</kbd> filter · <kbd>⌘L</kbd> list · <kbd>⌘E</kbd> editor · <kbd>Esc</kbd> back to list ·
<kbd>↑↓</kbd>/<kbd>J K</kbd> move · <kbd>⇧↑↓</kbd> extend selection · <kbd>⌘⌫</kbd> delete (undoable) · <kbd>⌘D</kbd> duplicate ·
<kbd>⌘⇧F</kbd> favorite · <kbd>⌘⇧E</kbd> on/off · <kbd>⌘⇧P</kbd> pause · <kbd>⌘1–5</kbd> views · <kbd>⌘,</kbd> settings.

<kbd>⌘Q</kbd> closes Kudzu's windows and keeps expansion, clipboard history, and the menu bar running. Reopen the library with **Open Kudzu…** in the menu bar. Choose **Quit Kudzu** there to exit completely.

Clipboard history: <kbd>↵</kbd> copy (in the picker, paste) · <kbd>⇧↵</kbd> plain text · <kbd>⌘↵</kbd> copy only (picker) · <kbd>⌘1–9</kbd> paste the nth item (picker) · <kbd>Space</kbd> large preview · <kbd>⌘P</kbd> pin · <kbd>⌘S</kbd> save as snippet · <kbd>⌘O</kbd> open · <kbd>⌘⌫</kbd> delete (undoable) · <kbd>⇥</kbd> filter by kind (picker). Search also accepts `app:slack` and `is:image`.

## Performance notes

- **Keystroke path (Rust):** reversed-trigger suffix tries, so the cost per key depends on the longest trigger, not the library size. Insertion runs on a worker thread, never in the tap callback.
- **Search:** with 5,000 snippets, each keystroke takes 1–4 ms and an index refresh after usage changes takes about 1 ms (folded fields are cached per edit stamp). `useDeferredValue` keeps typing responsive.
- **Windows:** the palette and form webviews are created hidden at launch, so the shortcut only shows an existing window. The palette-specific JavaScript is about 7 KB gzipped on top of the shared runtime.
- **List:** virtualized with TanStack Virtual. Row summaries are computed lazily and cached by content.
- **Clipboard:** a background thread reads the pasteboard's change count every 300 ms and only reads data when it changes. Thumbnails are made once at copy time, and media reaches the webviews through the `kzclip://` protocol with immutable caching instead of IPC. The index is written at most once a second. With 5,000 items (production build), a search keystroke costs about 10 ms of script and moving the selection about 3.5 ms; the list renders only its visible rows.
- **Disk:** writes are atomic (temp file + rename). A corrupt library is moved aside, never overwritten.

## Design

- **Theme:** dark first, with a light theme and System option. The palette is a shadcn theme (stone neutrals, blue primary) in `app/globals.css`; Kudzu's extra tokens (surfaces, hairlines, the `iris` accent) derive from it.
- **Accent:** one accent, *iris*, used only for focus, selection and tokens. It follows `primary` (the brighter `sidebar-primary` in dark mode).
- **Depth:** a small elevation scale (`shadow-card`, `shadow-raised`, `shadow-well`, …) that Tailwind's stock `shadow-*` and shadcn's components also map onto. On macOS the sidebar, palette and form use native vibrancy.
- **Type:** SF Pro (the macOS system face) for UI, Figtree as the non-macOS fallback, and Paper Mono for triggers and templates. Figtree and Paper Mono are bundled offline (OFL).
- **Motion:** short rise/pop animations that respect Reduce Motion.

## Frontend checks

Run `pnpm check` at the repository root for Oxlint and TypeScript across both apps. Shared rules live in `packages/oxlint-config/next.json`; the desktop config allows autofocus and custom ARIA widgets for the keyboard-driven native UI. Additional accessibility and compiler findings introduced by Oxlint are warnings in the desktop package, while the website uses the shared error-level rules. Native build and release commands remain in this directory.
