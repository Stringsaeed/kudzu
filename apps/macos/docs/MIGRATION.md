# Kudzu for macOS — migration plan (Tauri → native Swift)

This is the original UI/domain parity specification. The user subsequently approved
SwiftData with automatic CloudKit sync and a copy/verify/switch migration. For the
current persistence architecture and completion checklist, read
[`SWIFTDATA_MIGRATION.md`](SWIFTDATA_MIGRATION.md) and
[`SWIFTDATA_INTEGRATION.md`](SWIFTDATA_INTEGRATION.md). Their accepted replacement
supersedes the old DataStore/custom-sync tasks below. Current evidence and live
gates are in [`VERIFICATION.md`](VERIFICATION.md).

Read this guide and the Tauri sources it names for UI/domain parity. Edit the native
tree. `apps/desktop` stays until the final cutover. Parity notes in `docs/parity/`
cite that tree at tag `v0.6.0`.

## 1. Goal and non-negotiables

Rebuild Kudzu (`apps/desktop`, Tauri 2 + Next.js + Rust) as a **pure native macOS app** in
`apps/macos`, with the **same UI and the same behaviour**. Nothing is dropped, simplified or
"deferred": every gradient, shadow, animation, vibrancy/glass surface, drag interaction,
keyboard shortcut, window, setting and feature in the Tauri app exists in the native app and
behaves the same. When SwiftUI lacks a primitive, build it in AppKit; never cut the feature.

Parity rules:

- **Existing data is preserved during migration.** Keep bundle ids (`sh.saeed.kudzu`,
  dev `sh.saeed.kudzu.dev`), data directories and device-local state. Copy/verify the
  old Library into `Library.store` and retain originals/recovery evidence. JSON
  import/export remains compatible; it is no longer the runtime library authority.
  Automatic CloudKit uses the fresh `iCloud.sh.saeed.kudzu.library` destination;
  mixed-version sync with the old container is intentionally retired. Keep the
  updater feed, archive/signature format, MCP socket (`agent.sock`) and tools.
- **The TypeScript/Rust types are the contract.** `apps/desktop/lib/types.ts`,
  `kudzu-core/src/types.rs`, `clipboard.rs`, `settings.rs`, `updater.rs`, `cloud/mod.rs` define
  every JSON key. Swift `Codable` types use exactly those keys (camelCase; discriminators
  `state`, `phase`, `type`, `kind`). UUIDs are serialized **lowercase** (Rust style; CloudKit
  record names such as `snippet-<uuid>` are case-sensitive). Dates are RFC 3339 UTC with
  fractional seconds and parse any fraction length.
- **Logic is ported, not reinterpreted.** `kudzu-core` and `kudzu-engine` are the spec. Port
  function by function, keep doc comments, and port their tests (see §6).
- **Design tokens are ported as values.** `apps/desktop/app/globals.css` is the spec for colours
  (OKLCH → sRGB), radii, depth, motion and type. The *values and intent* are ported; the
  *mechanisms* are not (see the next rule).
- **Native first, never 1:1.** Parity is what the user sees and feels, not how it is built. Use
  the platform primitive whenever one exists and restyle it with the tokens: SwiftUI `Button` +
  `ButtonStyle`, `Toggle` (`.switch`/`.checkbox`), `Picker` (menu, segmented, radio) for selects,
  tabs and segmented controls, `Menu` and `.contextMenu` (real `NSMenu`s) for dropdown and context
  menus, `.popover`, `.sheet` / `.alert` / `.confirmationDialog`, `.help` tooltips, `List` /
  `Table` / `ScrollView` with native scrollbars, selection and keyboard navigation,
  `NavigationSplitView` with `.listStyle(.sidebar)` for the sidebar, `TextField` / `TextEditor` /
  `NSTextView`, `.searchable` where it fits, `.draggable` / `.dropDestination` / `Transferable`
  and `NSItemProvider` for drag and drop, `NSVisualEffectView` for glass, `NSStatusItem`,
  `NSPanel`, `.keyboardShortcut` / `FocusState`, native springs and `KeyframeAnimator`, SF Symbols
  for icons. Do not reimplement shadcn/Radix/cmdk components, do not emulate CSS mechanics (no
  pixel-exact `box-shadow` layer renderers, no `linear()` easing samplers, no hand-drawn
  scrollbars). Build custom views only where no primitive exists (template chips editor, command
  palette list, toast stack, selection pill, keycaps, trigger chips, insight bars) and compose
  them from native pieces. Where a native control dictates its own look (menus, alerts, switches),
  accept the native look over a hand-built lookalike.
- **Rust and TypeScript stay untouched** except for the fixture exporter in §6. The Tauri app keeps
  working until the native app ships.

## 2. Decisions

| Question | Decision | Why |
|---|---|---|
| Language | Swift 6 (Swift 6 language mode, strict concurrency), macOS 14+ | Same minimum as Tauri; `@Observable`, `CKSyncEngine`, SwiftUI `CustomAnimation` all need 14. |
| Rust core | **Ported to Swift**, no FFI | One toolchain, one language, "pure app". The Rust tests become the spec and golden fixtures check the port. |
| UI | **SwiftUI for views**, AppKit for the shell | SwiftUI gives gradients, springs, drag and drop and previews. AppKit owns `NSPanel`s, `NSVisualEffectView`, the status item, `CGEventTap`, and the chip editor (`NSTextView`). |
| Project | `Kudzu.xcodeproj` (thin app target) + local Swift package `Packages/KudzuKit` with all code | The pbxproj stays tiny and stable; modules build and test with `swift build` / `swift test` without Xcode. The project is generated from `project.yml` with XcodeGen and **checked in**, so Xcode works without XcodeGen. |
| Icons | **SF Symbols**, mapped one-to-one from the Lucide icons the web UI uses (a table in `KudzuDesign/Icons`) | Native iconography scales with text, weight and rendering modes. Lucide SVGs are bundled only for the handful of glyphs with no SF Symbol equivalent. |
| Fonts | SF Pro (system) for UI; Paper Mono bundled (`PaperMono-Variable.ttf`, derived from the repo's woff2) | Same as the web app on macOS. Figtree is the non-macOS fallback and is not needed. |
| Dependencies | **None** beyond Apple frameworks | Repo rule. Blake2b (for minisign) and OKLCH conversion are implemented in-tree. |

## 3. Layout

```
apps/macos/
├── Kudzu.xcodeproj/                generated from project.yml (checked in)
├── project.yml                     XcodeGen spec: one app target, two configurations
├── Kudzu/                          app target: entry point, Info.plist, entitlements, assets
│   ├── KudzuMain.swift             `@main`; handles `Kudzu --mcp [--dev]` before AppKit starts
│   ├── Info.plist                  LSUIElement, usage strings, ATSApplicationFontsPath
│   ├── Kudzu.entitlements          hardened runtime only (no iCloud)
│   ├── Kudzu.icloud.entitlements   iCloud (restricted) — release builds with a provisioning profile only
│   └── Assets.xcassets             app icon (from design/app-icon/Kudzu.icon), menu-bar glyphs, Lucide icons
├── Packages/KudzuKit/              Swift package; all code lives here
│   ├── Package.swift
│   ├── Sources/
│   │   ├── KudzuCore/              pure logic, Foundation only (port of kudzu-core + lib/*.ts pure helpers)
│   │   ├── KudzuEngine/            CGEventTap, insertion, pasteboard, permissions, apps, caret (port of kudzu-engine)
│   │   ├── KudzuServices/          disk stores, backups, clipboard history, hotkeys, login item, logging, updater, CloudKit, MCP socket
│   │   ├── KudzuDesign/            tokens, elevation, motion, materials, primitives (shadcn equivalents), Kbd/Shortcut, icons
│   │   ├── KudzuEditor/            NSTextView template editor with chips, slash menu, formatting
│   │   └── KudzuApp/               AppModel + Backend protocol, features (screens), shell (windows, panels, status item)
│   └── Tests/                      KudzuCoreTests, KudzuEngineTests, KudzuServicesTests, KudzuAppTests (+ Fixtures/)
├── scripts/                        build-app.sh (archive, sign, notarize, DMG, updater artifacts), fonts, icons
├── docs/MIGRATION.md               this file
└── README.md                       how to build, run, test; module map (written at the end)
```

Module dependency graph (acyclic):

```
KudzuCore ← KudzuEngine ← KudzuServices ← KudzuApp → KudzuDesign ← KudzuEditor
                                              └────────→ KudzuEditor
```

`KudzuDesign` and `KudzuEditor` depend on `KudzuCore` only for model types (chips need tag
tokens; `FormattedText` needs `FormatSpan`).

## 4. Architecture

### 4.1 KudzuCore (pure)

Port of `kudzu-core` plus the pure TypeScript helpers the UI needs. Foundation only, no AppKit,
no main-actor code. Namespaces (enums with static members or small types), one folder each:

| Folder | Port of | Notes |
|---|---|---|
| `Models/` | `types.rs`, `formatting.rs`, `settings.rs`, `clipboard.rs` types, `updater.rs`/`cloud` status enums, `types.ts` | `Snippet`, `Group`, `UsageRecord`, `Library`, `AppRule`, `Formatting`, `FormatSpan`, `FormatStyle`, `Settings`, `Theme`, `UpdateChannel`, `ClipEntry`, `ClipKind`, `ClipSource`, `ImageSize`, `SecretKind`, `EngineStatus`, `PermissionStatus`, `TapMode`, `AppInfo`, `ScreenRect`, `FormFieldSpec`, `FormFieldKind`, `FormRequest`, `Diagnostic`, `RenderResult`, `TriggerConflict`, `BackupInfo`, `ImportSummary`, `ImportWarning`, `EspansoDetection`, `SyncStatus`, `UpdateState`, `AgentSetup`, `ClipEvent`. `Codable`, `Sendable`, `Hashable` where the TS compares them. Defaults match `Settings::default()` and `Store::load_dev`. |
| `Template/` | `template/{mod,parser,render,icu}.rs`, `lib/template.ts` | `parse`, `serialize`, `typed`, `render`, `renderFormatted`, `analyze`, `formFields`, `formatICU`, `FormMemory`, `splitOptions`; plus `tokenize`, `chipLabel`, `chipTone`, `buildTag`, `plainSummary` from the TS. ICU date formatting goes through Foundation's `DateFormatter` (ICU underneath) with the exact fallbacks `icu.rs` implements; offsets ported from `icu.rs`. |
| `Matching/` | `matching/{mod,index,settings}.rs` | `MatchingConfig`, `KeyInput`, `TriggerIndex` (reversed suffix trie), `ExpansionMatcher`, `MatcherEvent`, `TriggerMatch`, `UndoAction`, `casePropagation`, `findConflicts`. Keep O(trigger length) per key. |
| `Formatting/` | `formatting.rs`, `lib/formatting.ts`, `lib/editor-formatting.ts` (pure parts) | `safeLink`, `safeImage`, `insertionText`, `insertionOffset`, `escapeHTML`, `html`, `sourceSpans`, `sameSpans`. |
| `Secrets/` | `secrets.rs` | `scan`, `detect`, `containsSecret`, `marker`, `redact`, `DETECT_LIMIT`. Use `Regex` literals or `NSRegularExpression`; keep every rule. |
| `Clipboard/` | `clipboard.rs`, pure parts of `lib/clipboard.ts` | `uti`, `classify`, `isPrivate`, `defaultIgnoredApps`, `fnv1a`, `fileURLToPath`, `isLink`, `parseColor`, `looksLikeCode`, `excerpt`, `History` (ingest/prune/remove/restore/clear/pin/markUsed/setSecret/rescanSecrets), `encodeItems`/`decodeItems`, `textOf`, `imageOf`, `imageMimeForPath`; and the UI helpers `parseQuery`, `searchClips`, `buildRows`, `describe`, `hasText`, `isSecret`, `FILTERS`, `inFilter`, colour helpers (HEX/RGB/HSL/OKLCH, contrast). |
| `Interchange/` | `interchange/*` | `KudzuJSON`, `Espanso` (importer/exporter), `Raycast`, `TextExpander` (plist + CSV), `Strftime`, `Dedupe`, `Coordinator`, `BackupStore`, `UniqueName`, `Stats` interchange. YAML: a small in-tree parser for the Espanso subset `yaml-rust2` handled (block mappings/sequences, scalars, quoted strings, block scalars `|`/`>`), tested against the same fixtures. |
| `Sync/` | `sync.rs` | `SYNC_VERSION`, zone/record names, `RemoteRecord`, `SyncIndex`, `LocalChanges`, `encode`, `localChanges`, `recordSaved`, `recordDeleted`, `applyRemote`, `initialMerge`. |
| `Agent/` | `agent/{mod,tools}.rs` | `Agent.handle(host:serverVersion:message:)`, `Host` protocol, `Access`, `RenderEnv`, `Offline`, `definitions`, `INSTRUCTIONS`, `SKILL` (loaded from the bundled `SKILL.md` resource), protocol versions. |
| `Search/` | `lib/search.ts` | `SearchIndex`, `SearchHit`, fuzzy scoring (fzf-style greedy with tightening), `#group` filters. |
| `Stats/` | `lib/stats.ts`, `lib/groups.ts` | `usageById`, `totals`, `formatCount`, `formatDuration`, `relativeTime`, `CHARS_PER_WORD`; `buildTree`, `flatten`, `descendantIds`, `recursiveCounts`, `groupPath`, `isGroupEnabled`. |
| `Keys/` | `lib/keys.ts` | Accelerator grammar: `keyLabels`, `acceleratorLabels`, `acceleratorFromEvent` (takes key code + modifiers), parsing `Control+Alt+Space` into Carbon key code + modifiers. |

### 4.2 KudzuEngine (system)

Port of `kudzu-engine`: same public contract as `kudzu-engine/src/lib.rs` (`Engine`, `EngineHost`,
`EngineLibrary`, `EngineStatus`, `FormRequest`, `ExpansionEvent`, `clipboard` and `system`
namespaces). Internals: `Tap` (CGEventTap on its own thread with a run loop), `Keys` (synthetic
events, markers), `Plan` (`InsertionPlan`, grapheme maths), `Hold` (hold/replay queue, 1 s limit),
`Status`, `Library` (engine-side index and rendering), `Apps` (running/installed apps, icons,
activation), `AX` (caret rect, Secure Input), `Pasteboard` (read/write items, own-change tracking,
paste with ⌘V, thumbnails). Threads mirror `macos/engine.rs` (tap, worker, poll). `EngineHost`
callbacks never run while engine locks are held.

### 4.3 KudzuServices

Everything the Tauri shell did outside the webviews, as plain Swift types (actors or
`@MainActor` classes as the threading demands):

- `DataStore` (port of `store.rs` + `settings.rs`): atomic writes (temp + rename), corrupt-library
  quarantine, legacy `sh.saeed.kudzu.desktop` migration, dev variant defaults, `noteAppVersion`,
  "Before update" backup, `recordUsage`, form memory, device id, `dailyBackup`. `library.json`
  is one document: the Kudzu JSON library plus the optional, Services-owned `_nativeSync` member
  (`StoredLibraryDocument`), so a library and the sync state it was accepted with are always
  replaced by the same rename. `editLibrary` carries the sync state unchanged; `editLibraryAndSync`
  changes both halves in one transaction and takes a commit-time fence; committed receipts
  (`awaitCommittedDocument`) report what is on disk, never the speculative snapshot. A document
  whose `_nativeSync` is not understood exactly (newer schema, unknown member, malformed value)
  stays readable and exportable but every ordinary write fails closed and the bytes are kept.
  The file is mode 0600 while it carries sync state, 0644 (byte-identical to
  `KudzuJSON.encodeLibrary`) otherwise.
- `ClipboardHistory` (port of `clipboard/{mod,store}.rs`): 300 ms change-count poll, classification,
  thumbnails (480 px), blobs ≤ 64 MiB, index saved ≤ once/s, retention every 10 min, events,
  media access (thumb/image/file/icon) as an in-process API instead of `kzclip://`.
- `HotkeyCenter`: Carbon `RegisterEventHotKey` for the palette and clipboard shortcuts (works
  without Accessibility), same slot semantics and error messages as `hotkey.rs`.
- `LoginItem`: `SMAppService.mainApp`, launched with `--hidden`.
- `Logging`: `os.Logger` + rotating file log at `~/Library/Logs/<bundle id>/Kudzu.log` (2 MB, keep 3).
- `Updater` (port of `updater.rs` without the plugin): stable/beta feeds, check 20 s after launch
  then every 6 h, quiet download, **minisign** verification (Ed25519 + Blake2b-512, pure Swift)
  against the embedded public key, install on quit or on "Restart to Update" (replace bundle,
  relaunch), "move to Applications" detection, dev build refusal.
- `CloudSync` (port of `cloud/{mod,apple}.rs`): `CKSyncEngine` on the private database, zone
  `Kudzu`, record types `KudzuSnippet`/`KudzuGroup`/`KudzuUsage`/`KudzuMeta`, fetch every 3 min,
  usage every 10 min, account changes, status publishing, dev/unsigned unavailability messages.
  Accepted state (the exact `Sync.SyncIndex`, the account and epoch, the engine checkpoint,
  the count of accepted inputs it covers, and optional `removedBaselines`) lives only in
  `library.json`'s `_nativeSync`; the type itself owns ephemeral work: a positively verified
  session (`SessionGate`: account status plus a nonempty user record name before any engine
  exists), one ordered FIFO of immutable inputs (`SyncInputQueue`, 64 MiB budget, 8 MiB chunks,
  one chunk admitted at a time, checkpoints coalesce and never cover an uncommitted earlier
  input), the engine, cached server records and status. Every engine callback, send builder
  and fetch completion is stamped with the session it was issued under and ignored otherwise.
  A library fence is an admission check under the store lock: a job already admitted finishes
  its rename, and a revocation before admission fails that coalesced lineage. The route joins
  the drain before a transition. `CKAccountChanged` invalidates the in-flight identity probe
  and revokes the current token before any stale result can publish a driver; the same account
  keeps its engine, and a different account releases it before the successor. A sign-out's
  record names are captured when the sign-out is observed and survive a failed transition write
  and a later same-account sign-in. One single-flight route (`requestRoute`) owns the engine's
  fetch/send until it returns. Queue pressure still retires and resumes from the stored
  checkpoint; one input larger than the budget, with nothing else queued, is refused instead
  so a restart does not refetch it in a loop. `removedBaselines` (schema 1, omitted when empty)
  stops a refetch from restoring a deduped or locally deleted record whose payload is unchanged.
  Side files under `<data>/sync/` are read once, by the read-only `LegacySyncMigration`, and
  only when their known keys match exactly. An unknown member, a progress account that
  disagrees with the scope or the journal, an index or progress phase whose merge flag
  disagrees with the index, or another account's journal (any phase) blocks without converting.
  A journaled batch after a finished first merge is not that disagreement.
  Converted files move to `sync/legacy/<id>/` and are marked (`sync/native-sync-converted`);
  after that they are never authority again, and a library rewritten by an older build
  (metadata gone, marker present) blocks sync. Tauri-era state without an account witness
  (`index.json` + `engine-state.data`, no `account-scope`) is preserved and blocked under the
  default `.conservative` policy; `.bindUnboundStateToVerifiedAccount` exists as the explicit
  product choice to treat it as a same-account resume and is not applied automatically.
- `AgentServer` (port of `agent.rs`): owner-only Unix socket `agent.sock`, line protocol, `Kudzu
  --mcp [--dev]` stdio bridge, 30 s reply timeout, skill install to `~/.claude/skills/kudzu/`,
  `AgentSetup` for the settings page.
- `PermissionGuide` (port of `permissions/{mod.rs,macos.m}`): the companion panel's native pieces —
  the draggable app-bundle icon (drag pasteboard holds the bundle URL), System Settings window
  bounds, click-through overlay animation, restart marker.

### 4.4 KudzuDesign

The design system: Kudzu's tokens applied to **native SwiftUI/AppKit primitives**. Spec:
`globals.css` and `components/ui/*` (for variants, sizes and states — not for implementation).

- `Tokens`: every colour token for light and dark as dynamic `NSColor`s (OKLCH → sRGB conversion
  in-tree, alpha preserved), exposed as `Color.kz.<token>` and `NSColor.kz.<token>`; the `vibrant`
  sidebar overrides; chip tones; the radius scale; the type scale (13 pt body / 20 pt leading,
  11/12 pt labels, 26 pt insight tiles, Paper Mono for triggers and templates, SF Pro = system).
- `Depth`: the `--elev-*` intent (card, raised, pressed, well, float, panel, accent, hairline…) as
  `.depth(_:)` modifiers built from native `.shadow`, `.background` materials and hairline strokes;
  one or two layers that read the same, not a port of the CSS layer lists.
- `Motion`: fast/moderate/slow; `springSoft`/`springPop` as native `.spring(duration:bounce:)`
  fitted to the CSS tables (bounce pinned by a test); `rise`, `pop`, `checkIn`, `barGrow`,
  `permissionConfirm` as transitions; `tick`/`starBurst` with `KeyframeAnimator`; `shimmer` as a
  repeating gradient offset; `.stagger()`; `.press()`; `sheen`/`noise` overlays; Reduce Motion via
  `accessibilityReduceMotion`.
- `Materials`: `VisualEffect` over `NSVisualEffectView` (sidebar follows window state, popover
  always active) with rounded masks.
- `Styles` (restyled natives): `ButtonStyle`s (default/secondary/ghost/outline/destructive/link;
  sizes), `TextFieldStyle` + input groups, `ToggleStyle` tints for `.switch`/`.checkbox`, `Picker`
  styling (menu/segmented/radio), `List` row styling (`Item` family as row content), `Menu`/`
  .contextMenu` conventions (shortcuts shown with `.keyboardShortcut`), `.popover`/`.sheet`/`.alert`
  usage helpers, `.help` tooltips, `Form` section styling for Settings, native scroll views.
- `Views` (custom only where nothing native exists): `Kbd` + `Shortcut`, `Badge`, `TriggerChip`,
  `SelectionPill`, `Segmented` only if `Picker(.segmented)` cannot carry the look, `Toast` stack
  (bottom-centre, action/cancel, Undo), `CommandList` (the ⌘K palette's filtered list), `Skeleton`,
  `FormattedText`, `AppIcon`, `Wordmark`, `Breadcrumbs`, `InsightBar`.
- `Icons`: `Icon.<name>` mapping every Lucide icon the web UI imports to an SF Symbol (table checked
  by a test for completeness); bundled SVG fallbacks only where no symbol fits.

### 4.5 KudzuEditor

`TemplateEditor`: `NSTextView` (TextKit 2) in an `NSViewRepresentable`. Behaviour of
`components/app/editor/template-editor.tsx` + `lib/editor-formatting.ts`: the stored source never
changes shape; every tag is an atomic inline chip (attachment with a custom view/cell, tone colours,
hover ring, click → `onChipClick(tag, rect)`); `/` opens the slash menu (`onSlashOpen`); insert /
replace / coordsAt API; placeholder; iris caret and selection colours; 13 px Paper Mono at 1.7
line height (sans in formatted mode); formatted mode with bold/italic/underline/link/image spans
and `⌘B/⌘I/⌘U`; image attachments with width; undo for formatting changes and image deletion;
`FormattingToolbar` and `ImageTools` live beside it.

### 4.6 KudzuApp

- `Model/AppModel.swift`: `@MainActor @Observable final class AppModel` — the port of
  `lib/store.ts` + `lib/clipboard.ts` store + the window-level state the webviews kept. Owns
  `library`, `settings`, `status`, `usage`, `conflicts`, `view`, `selectedId`, `filter`,
  `commandOpen`, `shortcutsOpen`, `focusNameOf`, `updateState`, `syncStatus`, `clipboard`
  (entries, diskUsage, canPaste), `isDev`, and every action in `backend.ts`'s `api` plus
  `store.ts`'s optimistic mutations. Features add to it only through extensions in their own
  file (`Model/AppModel+<Feature>.swift`).
- `Model/Backend.swift`: `protocol Backend: Sendable` with the `api` surface (async) and an
  `events: AsyncStream<BackendEvent>` (library, settings, status, update, sync, clipboard,
  navigate, newSnippet, formRequest, paletteShow, clipboardShow, permissionShow).
  `MockBackend` = port of `lib/mock.ts` (sample library, fake clipboard, pretend update, etc.) for
  previews and tests. `LiveBackend` wires the services.
- `Features/`: one folder per screen group — `Library/` (sidebar, snippet list, toolbar, status,
  command menu, shortcuts dialog, group settings, data actions), `Editor/` (snippet editor,
  behaviour, trigger input, tag tools, formatting toolbar, image tools, preview), `Palette/`,
  `Clipboard/` (view, picker, list, tile, preview, content, filters, secret), `Form/`,
  `Settings/` (General, Expansion, Clipboard, AI agents, Apps, Data, Shortcuts, About, hotkey
  recorder, update row, iCloud row), `Insights/`, `Onboarding/`, `Permissions/`.
- `Shell/`: `AppDelegate` (activation policy, reopen, `--hidden`, single instance, Cmd+Q keeps
  running, Quit from the menu exits), `MainWindow` (1180×760, min 900×560, transparent titlebar,
  hidden title, traffic lights at (18, 22), sidebar vibrancy following active state, content card
  with `rounded-2xl` + `shadow-card`), `PalettePanel` (760×480, radius 18, popover material,
  non-activating floating panel on all Spaces, positioned on the monitor under the mouse at
  y = 20 %), `ClipboardPanel` (800×520, y = 18 %), `FormPanel` (440 wide, radius 16, placed at the
  caret with the exact clamping in `windows.rs`, resizes to content 160–680), `PermissionsPanel`
  (420×660), `StatusItem` (four glyphs, update dot, menu items from `tray.rs`), `Theme`
  (`NSApp.appearance` from `settings.theme`; `system` = nil), `Hotkeys` wiring, `Prewarm`.

### 4.7 Conventions

- Swift 6 language mode. `@MainActor` for UI and AppModel; actors for stores with background I/O;
  `Sendable` models. No `@unchecked Sendable` without a comment saying why.
- Files ≤ ~400 lines; one type per file unless tiny. `// MARK:` sections.
- Doc comments carry over from the Rust/TS source; keep the file-header explanations.
- No force unwraps outside tests. Errors are typed (`enum … : Error`), messages match the
  Tauri strings the UI shows.
- Keep names from the source where they read well in Swift (`findConflicts`, `renderPreview`).
- Logging through `Logging.logger(category:)`; never `print` in app code.
- Tests use **Swift Testing** (`import Testing`). See §6.
- Commit messages: Conventional Commits with the `macos` scope, e.g. `feat(macos): …`.

## 5. Tasks

Each task names its inputs, its deliverable and its acceptance criteria. Tasks in the same phase
run in parallel in the **same checkout**; agents only create files inside their folders and extend
shared types through extension files. Every task ends with `swift build` and `swift test` for its
targets passing, and no warnings introduced.

### Phase 0 — scaffold and contracts (sequential)

**T0 Scaffold + models + contracts.**
Inputs: §2–4, `types.ts`, `types.rs`, `settings.rs`, `clipboard.rs` (types only), `updater.rs`
(`UpdateState`), `cloud/mod.rs` (`SyncStatus`), `agent.rs` (`AgentSetup`), `kudzu-engine/src/lib.rs`
(status/app types), `lib/mock.ts`, `lib/store.ts`, `lib/backend.ts`.
Deliverable: `project.yml` + generated `Kudzu.xcodeproj` (configurations: Debug = `Kudzu Dev`,
`sh.saeed.kudzu.dev`; Release = `Kudzu`, `sh.saeed.kudzu`), the app target with
`KudzuMain.swift`, `Info.plist`, both entitlements files, the asset catalog with the app icon and
menu-bar glyphs, Paper Mono TTF; `Package.swift` with all six modules and test targets;
`KudzuCore/Models` complete with `Codable` round-trip tests against JSON written by the Tauri app;
`KudzuApp/Model/{AppModel,Backend,MockBackend}.swift` with the full API (MockBackend ports
`lib/mock.ts`); placeholder `ContentView` so the app launches to a window; `scripts/dev.sh`
(build + run the Debug app). `.gitignore` entries for `.build/`, `DerivedData`, `xcuserdata`.
Acceptance: `xcodebuild -project apps/macos/Kudzu.xcodeproj -scheme Kudzu -configuration Debug build`
succeeds; `swift build && swift test` in the package succeed; the app launches as a menu-bar app
with a window; `Kudzu --mcp` exits cleanly with a TODO message.

### Phase 1 — foundations (parallel)

**T1 Core: template, matching, formatting, search, stats, keys.**
Inputs: `kudzu-core/src/{template,matching,formatting.rs}`, `lib/{template,search,stats,groups,keys,formatting}.ts`, `docs/TEMPLATE_SYNTAX.md`, tests `template_*.rs`, `matching_*.rs`, `formatting.rs`.
Deliverable: `KudzuCore/{Template,Matching,Formatting,Search,Stats,Keys}` + tests.
Acceptance: every Rust test in the listed files has a Swift counterpart with the same inputs and
expectations; the golden fixtures (§6) for render and matching pass; `swift test --filter KudzuCoreTests` green.

**T2 Core: interchange, secrets, clipboard model, sync, agent.**
Inputs: `kudzu-core/src/{interchange,secrets.rs,clipboard.rs,sync.rs,agent}`, tests `interchange_*.rs`, inline tests, `skills/kudzu/SKILL.md`, the README's AI agents section.
Deliverable: `KudzuCore/{Interchange,Secrets,Clipboard,Sync,Agent}` + tests; the in-tree YAML reader.
Acceptance: same as T1 for the listed tests; import fixtures (Espanso YAML, Raycast JSON,
TextExpander plist/CSV used by the Rust tests) produce identical libraries; the agent test suite
(fake host) passes.

**T3 Engine.**
Inputs: `kudzu-engine/src/**`, `docs/PERMISSIONS.md`, README "Expansion" and "Performance notes".
Deliverable: `KudzuEngine` with the full contract; `KudzuEngineTests` porting `plan.rs`, `hold.rs`,
`status.rs`, `library.rs` tests.
Acceptance: `swift test --filter KudzuEngineTests` green; a manual check in the Debug app (after
T6) expands `;kudzu` in TextEdit with typed and pasted insertion, whole-word and immediate
triggers, case propagation, Backspace undo, `{{cursor}}` placement, Secure Input detection.

**T4 Design system.**
Inputs: `app/globals.css`, `components/ui/*` (variants/states only), `components/app/{shortcut,segmented,selection-pill,trigger-chip,formatted-text,app-icon,wordmark,breadcrumbs}.tsx`, the Lucide import lists across `components/`, `design/`.
Deliverable: `KudzuDesign` (§4.4) with a `DesignGallery` preview showing every token, depth level,
motion and style in light and dark; the Lucide → SF Symbol table.
Acceptance: colours match `globals.css` within 1/255 per channel (tested against an independently
computed table); springs fit the CSS tables within 0.02 (tested, bounce pinned); every Lucide name
in use has a mapping (tested); styles are applied to native controls — a reviewer must find no
re-implemented menu, popover, tooltip, dialog, switch, select, tab bar or scrollbar.

**T5 Template editor.**
Inputs: `components/app/editor/{template-editor,formatting-toolbar,image-tools}.tsx`, `lib/{template,editor-formatting,formatting}.ts`, the CodeMirror CSS block in `globals.css`, README "Formatted snippets".
Deliverable: `KudzuEditor` (§4.5) with a preview harness.
Acceptance: typing round-trips byte-for-byte through `Template.parse`/`serialize`; chips are
atomic to the caret and to selection; slash menu and chip click callbacks fire with correct
rects; formatting spans survive edits exactly as `editor-formatting.ts` specifies (its tests in
`scripts/tests/formatting.test.mjs` are ported); undo works for formatting and images.

### Phase 2 — services and features (parallel; needs Phase 1)

**T6 Services + LiveBackend + shell.**
Inputs: `src-tauri/src/**` (all), `RELEASING.md`, `docs/PERMISSIONS.md`, §4.3, §4.6 Shell.
Deliverable: `KudzuServices` complete; `KudzuApp/Shell` complete; `LiveBackend`; `KudzuMain`
handles `--mcp`; `KudzuServicesTests` for `DataStore` (atomic write, quarantine, migration),
`Updater` (minisign + Blake2b vectors, feed parsing, channel logic), `CloudSync` (record
translation with a fake engine), `AgentServer` (socket round trip), `HotkeyCenter` (accelerator
parsing).
Acceptance: the Debug app runs on top of the real stores reading an existing Tauri data folder
unchanged; palette/clipboard/form panels open at the right place with the right material and
radius; status item glyphs and menu match `tray.rs`; `Kudzu --mcp` relays to the running app.

**T7 Library feature** (`Features/Library`): sidebar (sections, collapse memory, groups tree with
drag to move groups and snippets, context menus, counts, clipboard filters), snippet list
(virtualized rows 64 px, sort, filter, multi-select, keyboard, bulk actions, drag out), toolbar,
status indicator, command menu (⌘K), shortcuts dialog (⌘/), group settings dialog, data actions,
toasts with undo. Inputs: `components/app/main/*` except settings/insights/onboarding, `lib/store.ts`.

**T8 Editor feature** (`Features/Editor`): snippet editor (name, triggers with `TriggerInput`
chips, plain/formatted selector, content editor, Insert menu, live preview, collision warnings
linking to the other snippet, autosave, more menu), behaviour section (fire mode, case, insertion,
app rules with `AppPicker`), tag tools (chip configuration popovers with live example for
date/time/clipboard/snippet/field/choice), formatting toolbar and image tools wiring. Inputs:
`components/app/editor/*`, `components/app/app-picker.tsx`.

**T9 Palette + Form** (`Features/Palette`, `Features/Form`): quick search (scopes with Tab,
`#group` filter, highlighted hits, live preview, ↵ insert, ⌘↵ copy, ⌘E edit, ⌘1–9, create from
query, target app chip) and the fill-in form (fields/choices, Tab order, ↵ / ⌘↵, Esc, remembered
values, live preview, content-sized panel). Inputs: `components/app/palette/palette.tsx`, `components/app/form/form-panel.tsx`.

**T10 Clipboard feature** (`Features/Clipboard`): history view (search with `app:`/`is:`, kind
filters, day grouping, tiles with thumbnails, previews for every kind incl. code with line
numbers, colours with HEX/RGB/HSL/OKLCH + contrast, links, file lists with Finder icons, copy
timeline, secret masking with hover reveal and the two corrections, pin, save as snippet, open,
reveal, delete with undo, large preview on Space) and the picker panel (filters with Tab, ⌘1–9
paste, ⇧↵ plain, ⌘↵ copy only). Inputs: `components/app/clipboard/*`, `lib/clipboard.ts`.

**T11 Settings + Insights + Onboarding + Permissions** (`Features/Settings`, `Insights`,
`Onboarding`, `Permissions`): all eight settings sections with every row, hotkey recorder,
update row and About (release notes, Check now, beta channel), iCloud row with statuses, AI
agents page (commands, JSON config, skill install), Apps page, Data page (import/export/backups
restore), Shortcuts cheat sheet; Insights tiles and ranked bars; onboarding steps (welcome,
access, import, practice) with the atmosphere gradient and staggered motion; the permission
companion (`permission-center.tsx`, `permission-setup.tsx`) on top of `PermissionGuide`.
Inputs: `components/app/main/{settings,insights,onboarding,shortcuts}.tsx`, `components/app/permissions/*`, `docs/PERMISSIONS.md`.

Acceptance for T7–T11: every element, state, shortcut and animation in the named TSX files exists
and behaves the same, built on native primitives per §1 (sidebar = `NavigationSplitView` + sidebar
`List`, lists = `List` with native selection and keyboard navigation, menus = `Menu`/`.contextMenu`,
settings = `Form`, dialogs = `.sheet`/`.alert`, drag and drop = `Transferable`); SwiftUI previews with `MockBackend` cover each screen in light and dark;
`swift build` clean. The agent records a parity checklist (one line per TSX component) in
`docs/parity/<feature>.md` with anything it could not match and why.

### Phase 3 — integration and release tooling

**T12 Integration.** Wire features into the shell on `LiveBackend`, remove placeholders, run the
app end to end against a copy of the Tauri data folder, fix cross-feature issues, write
`apps/macos/README.md`, update the root `CLAUDE.md`, `.github/workflows/ci.yml` (build + test
the package and the app on macOS), and `scripts/build-app.sh` producing the same release files as
`RELEASING.md` (universal app, signed, notarized, DMG with the existing background, `.app.tar.gz` +
minisign signature, `latest.json`).

### Phase 4 — verification (parallel, different model from the author)

**V1–V5** One reviewer per area (core, engine+services, design+editor, features, shell+release),
each reading the Tauri source and the Swift port side by side, running the tests and the app, and
filing concrete defects with file:line and a reproduction. Fixes go back to an implementer, then
the reviewer re-checks.

## 6. Tests

Useful and scalable, not ceremonial:

- **Ported behaviour tests.** Every Rust `#[test]` in `kudzu-core` and the pure parts of
  `kudzu-engine` becomes a Swift Testing test with the same name (camelCase), inputs and
  expectations. ~330 tests. They are the specification of the port.
- **Golden fixtures.** `Packages/KudzuKit/Tests/KudzuCoreTests/Fixtures/*.json` are frozen at
  the 0.6.0 Rust export: template render cases (source, context, expected text/cursor/diagnostics),
  matcher traces (key sequence → events), import corpora → libraries, secrets corpus → findings,
  sync merges. The Swift tests replay them. The generator lived in the Rust tree and is not run
  from this app. Parity notes in `docs/parity/` cite that same Tauri tree at tag `v0.6.0`.
- **Contract tests.** `Codable` round trips against real files written by the Tauri app (library,
  settings, clipboard index, backups) and against the TS `types.ts` shapes.
- **Design data tests.** Colour conversion, elevation layers, spring sampling: tested as data,
  not as screenshots.
- **Service tests** with fakes: file store, updater verification vectors, CloudKit translation,
  agent socket round trip.
- **No UI snapshot tests.** Parity is verified by reviewers running the app with `MockBackend`
  previews and against the Tauri app side by side.

## 7. Commands

```sh
cd apps/macos/Packages/KudzuKit && swift build && swift test          # all modules
cd apps/macos && xcodebuild -project Kudzu.xcodeproj -scheme Kudzu -configuration Debug build
cd apps/macos && scripts/dev.sh                                        # build and launch Kudzu Dev
cd apps/macos && xcodegen generate                                     # only after editing project.yml
# Golden fixtures under Tests/KudzuCoreTests/Fixtures are frozen. Do not regenerate them here.
```

`xcodebuild` needs the full Xcode: `export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`
if `xcode-select` points at the Command Line Tools.

## 8. Integration notes (running list for T12 and reviewers)

Facts agents reported that later tasks must know; remove an entry when it is resolved.

- Naming in `KudzuCore`: the TS `View` is `Route`; `SnippetGroup` aliases `Group` (SwiftUI clash);
  SwiftUI files must write `KudzuCore.Settings` (clashes with the `Settings` scene); the engine's
  `FormRequest` keeps its Rust name and the TS `FormRequest` is `FormPayload`; `Rect` is `ScreenRect`.
- Duplicated constants: `Clipboard.TEXT_EXCERPT_LIMIT` etc. (T2) and `ClipEntry.textExcerptLimit`,
  `defaultIgnoredApps` … (T0) say the same thing twice. Unify on one home (T12).
- `TriggerConflict` is declared in `Matching/TriggerConflicts.swift` (T1) with its JSON contract in
  `Models/TriggerConflict+Codable.swift` (T0): keys `snippetId`/`otherSnippetId`.
- `Backend.events` creates a new subscription per read; `BackendEvent.command` is never emitted.
- `AppModel` is eventually consistent like the TS store: a stale backend event can briefly
  overwrite an optimistic change; tests wait for convergence.
- App target: ad-hoc signing leaves the hardened runtime flag off locally on purpose (dyld rejects
  the ad-hoc debug dylib otherwise); the release script signs with Developer ID. The `.icon`
  bundle compiles to `Kudzu.icns` through actool; `NSHostingView.sizingOptions = []` keeps the
  main window at 1180×760.
- `KudzuDate`: library records (`Snippet`, `UsageRecord`, `BackupInfo`) write whole seconds like
  Rust's `iso8601`; clipboard and status types write chrono style (`Z`, 0/3/6 fraction digits).
- Known representation limits (accepted): nanosecond render precision needs
  `RenderContext.nanosecond`; `plainSummary` cannot keep a lone UTF-16 surrogate (U+FFFD instead).
- Swift `String` equality is canonical equivalence (`é` == `e + ◌́`); Rust compares bytes. Any
  place Rust compared strings or keyed maps by string (form names, triggers, ids, dedupe) must
  compare by unicode scalars in Swift. V1a is auditing KudzuCore; reviewers check their modules.
- Engine live checks owed to T12 (after the app runs on `LiveBackend`): in TextEdit type `;kudzu`,
  `;Kudzu`, `;KUDZU` (case propagation, immediate Backspace undo); whole-word firing with Space and
  Return emitted once; typed, long pasted, formatted, image and `{{cursor}}` snippets in TextEdit and
  an Electron editor, typing another trigger mid-insertion (ordered replay); clipboard restored after
  400 ms and a newer copy preserved; form cancel leaves the trigger, submit replaces it in the
  original app; a target that quits during activation; Secure Input blocks expansion; Input
  Monitoring alone cannot insert; Kudzu's own windows suppress matching; quitting mid-insertion
  restores the clipboard and releases held input.
- Accepted limits from the core review: `[String: String]` form-value maps cannot hold two keys
  that differ only by Unicode normalization (Rust maps can); the renderer falls back to each
  field's default, so only a user submitting both spellings loses one. Swift matches a Rust bug
  where a quoted attribute containing `"`/`\` plus a combining mark serializes malformed. `safeLink`
  follows Rust where TS disagrees (U+FEFF, C1 controls). Search lowercasing lacks JS's final-sigma rule.
- Trigger matching, the trie, per-snippet trigger dedupe and conflicts are keyed by NFC on purpose
  (Rust `key_buffer::nfc`, `folded_key`); do not "fix" them to byte comparison.
