# Kudzu on Rune: build plan

Build Kudzu again as a pure Rust app on [Rune](https://github.com/anomalyco/rune), in its own directory, `apps/desktop-v2/`. It is a separate app, not a migration in place. When it reaches parity, `apps/desktop` (the Tauri app) is deleted and `apps/desktop-v2` is renamed to `apps/desktop`. After that, the repository's desktop app has no Tauri, webview, Next.js, React, shadcn, Tailwind or TypeScript.

- **Branch:** `t3/rune-migration-plan`. Commits only, no PRs.
- **Rune pin:** `dev` at `29aacb688883a9712bc27175bdafd99fc6edb421`. Rune paths below (`crates/rune/...`, `examples/...`) are in that repository at that rev. Clone it read-only before you use an API.
- **This plan and its inputs** live in `apps/desktop-v2/docs/migration/`: `PLAN.md`, `FEATURES.md` (parity checklist), `RUNE_FIT.md` (Rune fit), `CRATES.md` (crate research), `PLAN_REVIEW.md` (the review answered in §7). Paths inside `FEATURES.md` are relative to `apps/desktop/`, the Tauri app. Where this plan disagrees with the research files, this plan wins; the disagreements are in §2 and §3.
- **Priority order (owner):** every feature and its behaviour; same UI structure and quality; solid architecture; reuse maintained crates; keep it simple (no users, so no data migration).

## How to read this plan

**Paths.** In §1 and §4, a path that does not start with `apps/`, `.github/` or "root" is relative to `apps/desktop-v2/`.

**Check ids.** `FEATURES.md` has no ids. This plan names each checkbox by a domain prefix plus its 1-based position inside that domain's section:

| Prefix | Domain | Count | Prefix | Domain | Count |
|---|---|---:|---|---|---:|
| EXP | Expansion engine | 20 | MCP | AI agents / MCP | 10 |
| TPL | Templates | 17 | ACT | Activity | 5 |
| EDT | Snippet editor | 18 | INS | Insights | 4 |
| RUL | Rules | 9 | DAT | Data | 13 |
| QS | Quick Search palette | 8 | SET | Settings | 12 |
| FRM | Fill-in forms | 6 | ONB | Onboarding + permissions | 10 |
| LIB | Library | 15 | SHL | Shell | 12 |
| CLP | Clipboard history | 20 | SYN | iCloud sync | 7 |
| PKR | Clipboard picker | 6 | WIN | §2 Windows checkboxes | 3 |

`EDT-9` is the ninth checkbox under "Snippet editor". 195 checks in total.

**Rules for every step.** A step brief can add to these, never remove them.

1. **`apps/desktop` is frozen and untouched.** Until S75, change nothing under `apps/desktop/`, and nothing that keeps it building and releasing: `.github/workflows/{ci,release,build-macos,beta}.yml`, `release-please-config.json`, `.release-please-manifest.json`, root `package.json`, `pnpm-lock.yaml`, `turbo.json`, `apps/website/`. The Tauri app, its CI and its releases keep working exactly as they do today.
2. **`apps/desktop-v2` stands alone.** Nothing under it may depend on `apps/desktop`: no path dependency, no `#[path]`, no `include_str!` or `include_bytes!`, no `build.rs` or script that reads its files at build time. What the new app needs is **copied in** by S01 and evolves in `apps/desktop-v2` from then on. Reading the Tauri app to learn how it behaves is expected. A step may copy a named file from it when its brief says so. The one tool that reads `apps/desktop` at run time is the S02 reference capture, which is not part of the build and is deleted at cut-over.
3. **Ignore the earlier ports.** Do not read or reuse PR #40 / `t3/migrate-tauri-rune-ui` or `migration/native-macos`.
4. **Do not copy the Tauri shape.** No command enum or string-dispatched command table. No event names with payloads. No UI-side mirror of domain state. No module per former TSX file. No reimplementation of shadcn. The copied `kudzu-core` and `kudzu-engine` are a starting point: they are restructured into the layout in §1.2 and §1.5, not preserved because they were copied.
5. **Dependencies.** Use only the crates §2 lists as decided. Anything else: stop and report; the owner approves dependencies.
6. **Rune is pre-release.** Read the source at the pinned rev before you rely on an API; its README lags in places. When Rune lacks something, follow §3: file or comment on an upstream issue, then build the workaround in the module §3 names and mark it `// RUNE-GAP(#issue): <what to delete when fixed>`.
7. **Module hygiene.** One responsibility per module. Target 400 lines per file, hard limit 600 outside tests and data tables. No `utils.rs`, `helpers.rs` or `common.rs`. Keep `pub` narrow: a crate exports what another crate calls.
8. **Tests.** Core and application logic are written test-first. A step is done when, in `apps/desktop-v2`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo xtask check-deps` pass, plus the step's own acceptance list.
9. **Privacy invariants, by sink.** Only the local product stores (library, backups, clipboard history, form memory) hold user content. Logs, error causes written to the log, and Activity never contain typed text, inserted text, clipboard contents or snippet bodies; Activity stays in memory. Only the library syncs: clipboard history, settings and form memory never do. Secrets are redacted before anything reaches an agent.
10. **Commits and releases.** One commit per step with the message given (small follow-ups allowed). Before S75, a commit touches only `apps/desktop-v2/` (plus the two new `desktop-v2*.yml` workflow files, in S01, S03 and S70) and uses the scope `desktop-v2`. release-please's package path is `apps/desktop`, and it counts only commits that touch files under `apps/desktop/`, so these commits cannot open or change a release. As a second guard, every commit before S75 uses a type release-please hides (`refactor`, `build`, `test`, `docs`, `ci`, `chore`). S01 confirms both with a dry run. The rewrite gets one changelog entry, from S75.
11. **Report honestly.** If an acceptance item cannot be met, say which and why; do not weaken the test.

---

## 1. Target architecture

### 1.1 Where things live

`apps/desktop-v2` is one Cargo workspace. The root package is the binary. The app version lives in `version.txt`; crate versions are internal and never change.

```
apps/desktop-v2/
├── Cargo.toml            [package] kudzu (bin) + [workspace]
├── version.txt           the app version, the one place it is written
├── Cargo.lock
├── rust-toolchain.toml   1.99.1, the toolchain Rune pins
├── .cargo/config.toml    alias: xtask = "run -p xtask --"
├── src/                  composition root only (main.rs, cli.rs, identity.rs, logging.rs, compose.rs)
├── crates/
│   ├── kudzu-core/       pure domain: library model, templates, matching, formatting, interchange,
│   │                     clipboard model, secrets, sync model, activity, search, colour, key labels
│   ├── kudzu-app/        application layer: one service per domain, persistence, schedulers, ports
│   ├── kudzu-engine/     expansion engine: event tap, insertion, input sources, AX, pasteboard
│   ├── kudzu-system/     macOS adapters that implement kudzu-app's ports
│   ├── kudzu-appkit/     AppKit glue for Rune gaps, one module per gap
│   └── kudzu-ui/         Rune UI: theme, widgets, shell, windows, screens
├── xtask/                bundle, sign, DMG, updater feed, dependency and parity checks
├── assets/               fonts (TTF), icons, tray glyphs, Info.plist template, entitlements
├── design/ skills/ scripts/ cloudkit/     copied from apps/desktop by S01, then owned here
├── spikes/               throwaway spike binaries (S10–S15); deleted in S75
└── docs/                 TEMPLATE_SYNTAX.md, PERMISSIONS.md, migration/ (this plan)
```

**Two apps until cut-over.** `apps/desktop` stays as it is: the reference for behaviour and looks, and the app that keeps shipping. `apps/desktop-v2` shares nothing with it at build time. The duplication (core, engine, design assets) lasts until S75 and buys two things: the reference can never be broken by new work, and the copied code can be reshaped freely.

**What S01 copies in** (plain copies, then owned by `apps/desktop-v2`): `src-tauri/crates/kudzu-core` and `kudzu-engine` with their tests and fixtures; `skills/`; `design/` (brand.json, app icon, wordmark, menu-bar glyphs, DMG background, manifest); the design scripts and `scripts/tests/formatted-paste.swift`; `src-tauri/icons/`; the Paper Mono font and its licence; `Info.plist`; the iCloud entitlements; `src-tauri/cloudkit/`; `docs/TEMPLATE_SYNTAX.md` and `docs/PERMISSIONS.md`. S01 records the source commit in `COPIED_FROM.md`. If `apps/desktop` changes any copied file after that, the change is ported by hand before S75 (S75 checks).

**Dependency rules.** An arrow means "may depend on". Nothing else is allowed.

```
kudzu-core      (no kudzu crate)
kudzu-engine  → kudzu-core
kudzu-app     → kudzu-core
kudzu-system  → kudzu-app, kudzu-core, kudzu-engine
kudzu-appkit    (no kudzu crate)
kudzu-ui      → kudzu-app, kudzu-core, kudzu-appkit, rune, rune-kit
kudzu (bin)   → all of the above
xtask           (no kudzu crate)
```

| Crate | Must not contain |
|---|---|
| `kudzu-core` | Any OS, UI or async crate. File access only in the `interchange` importers and exporters, where walking a folder is part of the format. |
| `kudzu-app` | `objc2*`, `rune*`, `winit`, `tokio` anywhere in its dependency tree. (`core-foundation-sys` is allowed: `chrono`'s local clock brings it in on macOS.) |
| `kudzu-engine` | `rune*`, `kudzu-app` |
| `kudzu-system` | `rune*` |
| `kudzu-appkit` | any `kudzu-*` crate, `rune*` (it takes raw `NSWindow`/`NSView` pointers and closures) |
| `kudzu-ui` | `kudzu-engine`, `kudzu-system` (the UI reaches the OS only through `kudzu-app` and `kudzu-appkit`) |
| `kudzu` (bin) | logic. It parses arguments, resolves the identity, builds adapters and services, and starts Rune. Limit: 500 lines in `src/`. |

`cargo xtask check-deps` enforces this from `cargo metadata` and a scan of first-party files (workspace members and `scripts/`; registry and git dependencies are not scanned), and CI runs it. It fails on:

- a forbidden crate edge, or a forbidden crate in a tree;
- **any first-party path that leaves `apps/desktop-v2`**: a workspace member, a path dependency, a `#[path]`, or an `include_str!` / `include_bytes!` / `include!` that resolves outside the workspace root;
- a `build.rs` in a first-party crate (none is needed, and a build script is the one thing a scan cannot vouch for);
- the string `apps/desktop/` or `../desktop` in a first-party manifest, Rust source or `scripts/` file (`docs/migration/` is exempt).

The boundaries are enforced by the build, not by review.

**Why six crates.** Each split removes a way to cheat. The UI cannot call the engine behind the application layer because it cannot name it. Application logic is testable on any machine because it cannot link AppKit. Rune-gap glue is deletable because nothing of ours leaks into it.

### 1.2 Domain model and application layer

`kudzu-core` holds rules and pure algorithms. `kudzu-app` holds state, persistence and coordination. Both are grouped **by domain**.

**`kudzu-core` after restructuring (S20).** The copied crate is close to a domain layout already; what it carries from its past is an MCP server inside the domain crate, import orchestration mixed with file I/O, a flat `types.rs`, serde attributes that existed for IPC, and several oversized files.

| Module | Holds | Comes from |
|---|---|---|
| `library/` | `model` (snippet, group, usage, app rule), `json` (Kudzu JSON codec), `groups` (inheritance resolver, tree, paths, counts), `triggers` (validation), `conflicts`, `usage` (aggregation, stats), `dedupe`, `starter` (Getting Started snippets) | `types.rs`, `interchange/{kudzu_json,stats,dedupe}.rs`, `matching/{settings,validator,conflicts}.rs`, part of `interchange/coordinator.rs`, `lib/groups.ts`, `lib/stats.ts` |
| `template/` | parser, serializer, renderer, analysis, ICU formatter, form fields and form memory model, and the editor document (S23) | `template/`, `lib/template.ts`, `lib/formatting.ts`, `lib/editor-formatting.ts` |
| `matching/` | key buffer, trigger index, matcher, case propagation, undo, matching config and defaults | `matching/` |
| `formatting/` | spans and HTML output | `formatting.rs` |
| `interchange/` | Espanso, Raycast and TextExpander readers, the Espanso writer, rich-text readers, `detect` (format detection), `merge` (imported into existing), `backup` (file naming, reasons, daily rule and rotation as pure functions) | `interchange/`, with file orchestration removed |
| `clipboard/` | `classify`, `history`, `retention`, `blob`, `query`, `sections`, `facts` | `clipboard.rs`, `lib/clipboard.ts` |
| `secrets/`, `sync/`, `activity/` | unchanged in substance, split where a file is over the limit | same names |
| `search`, `color`, `keys`, `release_notes`, `fmt` | new pure modules | `lib/search.ts`, `clip-content.tsx`, `lib/keys.ts`, `lib/updates.ts`, label code in `lib/stats.ts` |
| `system` | shared vocabulary: `AppInfo`, `Rect`, `PermissionStatus`, `TapMode`, `EngineStatus` | `kudzu-engine/src/lib.rs` |

Leaves core: `agent/` (the MCP server is an inbound adapter; it moves to `kudzu-app::agents` in S32) and the file orchestration in `interchange::coordinator` and `BackupStore` (to `kudzu-app::data` in S29). Removed: serde derives and renames that existed only so a type could cross IPC. Kept byte-for-byte: every persisted format (library JSON, clipboard index, sync payload, form memory, backup names).

**`kudzu-app`: one module per domain, one public service per module.**

| Module (`crates/kudzu-app/src/…`) | Owns | Narrow interface the UI and adapters call |
|---|---|---|
| `library` | Snippets, groups, usage; `library.json`; trigger conflicts | `watch()`, `conflicts()`, `create_snippet`, `update_snippet`, `duplicate_snippet`, `delete_snippets → Deleted`, `restore(Deleted)`, `move_snippets`, `create_group`, `update_group`, `move_group`, `delete_group`, `record_expansion`, `transact(|lib| …) → Result<(T, Revision)>`, `saved()` |
| `settings` | Typed settings and variant defaults; `settings.json` | `watch()`, `update(|s| …)`, typed setters that validate (prefix length, retention range) |
| `expansion` | The engine port; index feed; pause; suppression; health; explicit insert; render and copy | `status()`, `set_paused`, `suppress() → SuppressionGuard`, `begin_quick_search() → QuickSearchSession`, `insert`, `copy_rendered`, `preview(source, context) → RenderResult` |
| `forms` | The pending form, its suppression, and `form-memory.json` | `pending() → Watch<Option<Arc<FormRequest>>>` (token, fields, initial values, target, caret, selection), `preview(token, values)`, `submit(token, values)`, `cancel(token)`, `panel_focused(token, bool)` |
| `activity` | The 100-entry in-memory ring; remedies | `watch()`, `clear()`, `remedy(entry) → Option<Remedy>`, `apply(Remedy)` |
| `clipboard` | History, capture pipeline, blobs, thumbnails, retention, trash, secret overrides, copy and paste | `watch()`, `query(&ClipQuery)` (pure, on a snapshot), `text(id)`, `media(id, MediaKind)`, `pin`, `delete → DeletedClips`, `restore`, `set_secret`, `clear`, `copy(id, Flavor)`, `begin_pick() → PickSession`, `PickSession::paste(id, Flavor)`, `usage()` |
| `data` | Import, export, backups, restore, Espanso detection, backup schedule | `detect_espanso()`, `import(source) → Task<ImportSummary>`, `export_json`, `export_espanso`, `backups()`, `backup_now`, `restore(backup)` |
| `sync` | Sync lifecycle and status over the `CloudStore` port | `status()`, `set_enabled`, `sync_now` |
| `updates` | Update phases, feed, download, verify, schedule, release notes | `state()`, `check(Trigger)`, `restart_to_update()`, `install_on_quit()` |
| `agents` | MCP protocol and tools, socket server, setup command, skill install, stdio bridge | `status()`, `apply(settings)`, `setup() → AgentSetup`, `install_skill()`, `bridge::run(stdin, stdout, identity)` |
| `apps` | Installed-app catalogue and icon cache | `installed() → Watch<…>`, `icon(bundle_id) → Task<Png>`, `name_of(bundle_id)` |
| `permissions` | Permission-setup state machine and the restart marker | `state()`, `begin(Kind)`, `refresh()`, `request_restart()`, `take_pending_restart()` |
| `onboarding` | First-run state machine (steps, import recovery, finish retry) | `Onboarding::{step, advance, back, skip, record_import, finish}` |
| `hotkeys` | The two shortcut slots: validation, collision, rollback | `HotkeySlots::set(slot, chord, &dyn HotkeyRegistrar) → Result` |

Supporting modules, each small and std-only: `watch` (below), `tasks` (one named thread per long operation with a `Task<T>` result slot; no pool), `schedule` (one timer thread), `persist` (atomic JSON files, one writer thread), `ports` (traits the outside implements), `fixtures` (in-memory fakes and sample data for tests and headless scenes), `assembly` (`Kudzu::new` builds the services in dependency order; `Kudzu::shutdown`). `Kudzu` exists for composition and shutdown only; windows receive the few services they use, never the whole set. Queries the UI fires per keystroke use `rune::Latest`'s workers, not anything here.

**State and change notification.** Each service owns its state behind its own lock and publishes immutable snapshots:

```rust
// crates/kudzu-app/src/watch.rs: the only cross-layer notification primitive.
pub struct Watch<T> { /* Arc<T>, revision counter, wakers */ }
impl<T> Watch<T> {
    pub fn get(&self) -> Arc<T>;              // current snapshot, cheap
    pub fn revision(&self) -> u64;            // changes iff the snapshot changed
    pub fn subscribe(&self, wake: impl Fn() + Send + Sync + 'static) -> Subscription; // drop = unsubscribe
}
```

A waker carries no payload. Whoever is woken reads the current snapshot. Wakers run after the service has released its lock and must only set a flag, enqueue work, or wake a window.

**Transactions.** Every library change is one transaction under the library's lock: validate, apply, commit, return the new `Revision`. The typed methods are transactions. `transact` runs a caller's pure closure on a working copy and commits only on `Ok`; agent edits (collision check, group-path creation and the snippet as one unit), import merges, restores and sync merges use it, so they apply to the current library, never to a stale snapshot. A closure does no I/O and calls no service.

**Accepted is not durable.** A commit changes memory and returns at once; the writer thread persists it. Each persisted service publishes `saved() → Watch<SaveState { durable: Revision, error }>`. The UI shows "Saved" when `durable` reaches the revision it committed and "Couldn't save" while `error` is set. A failed write loses nothing: the change is in memory, and the writer retries on the next change and on a timer. `shutdown()` writes synchronously.

**Cross-domain reactions.** A service that reacts to another takes that service's `Watch` in its constructor and does the work on its own thread. Reactions are state-based: the reactor reads the latest snapshot and works out what to do, so coalesced wake-ups lose nothing. This table is the complete list, and together the rows do what `AppCore::library_changed()` did:

| When this changes | This reacts |
|---|---|
| `library` or `settings` | `expansion` rebuilds the engine index (no-op when matching data is unchanged, EXP-19) |
| `library` | `sync` diffs the library against its index and queues uploads. A remote merge commits the library and the index in one job on the same sync thread, so it leaves no diff and nothing echoes. |
| `library` (snippets removed) | `forms` prunes form memory |
| `settings.clipboard_*` | `clipboard` starts or stops capture, re-prunes |
| `settings.agent_*` | `agents` opens or closes the socket |
| `settings.icloud_sync` | `sync` starts or stops |
| `settings.launch_at_login` | the `LoginItem` port is set (wired in `assembly`) |
| engine `expanded` event | `library.record_expansion` |
| engine `form_requested` event | `forms` publishes the pending `FormRequest` |

**Sessions replace window hand-off fields.** `palette_target`, `palette_selection`, `pending_form` and `clip_target` become values with a lifetime:

- `QuickSearchSession { target, selection, _suppress: SuppressionGuard }`: created when the palette is asked to open; captures the frontmost app and, only when a snippet uses `{{selection}}`, the selection, before the palette takes focus (TPL-6, QS-1). The palette model holds it in an `Option`, and every way of hiding goes through one `dismiss()` that takes it: a hidden Rune window keeps its state, so nothing may rely on a window being dropped.
- `PickSession`: the same for the clipboard picker, with `can_paste`.
- The pending form is owned by `forms`, not by a window. The engine's request becomes an immutable `FormRequest` with a token; `submit` and `cancel` take the token and act exactly once (a stale or repeated token does nothing); `forms` holds the suppression guard until then; `Shell::close_windows` cancels it.
- `SuppressionGuard`: reference-counted; while any guard lives the engine matches no typed trigger. No scattered `set_suppressed(true/false)` pairs. Suppression stops matching only. Whether a typed form was interrupted is a separate signal: the form window reports `panel_focused`, and the engine marks a typed pending form interrupted on any key-down or click while the panel is not key (FRM-4). The Tauri engine skips that check while suppressed, so its guarantee has a hole; §6 records this as a deliberate fix.

**Ports** (`crates/kudzu-app/src/ports/`), implemented outside: `ExpansionEngine` and `EngineEvents`, `Pasteboard`, `Thumbnailer`, `AppDirectory`, `SystemSettings`, `LoginItem`, `FileOpener`, `CloudStore`, `UpdateInstaller`, `Relauncher`, `Fetch` (HTTP), `Clock`, `HotkeyRegistrar` (implemented by the UI shell, because Rune owns global shortcuts). Every port has a fake in `fixtures`.

**How this differs from the IPC layer of the Tauri app.**

| Tauri shape | Target |
|---|---|
| About 70 string-named commands in one namespace, arguments and results as serde DTOs | Typed methods on 14 domain services. No serialization, no names to dispatch on. |
| `AppCore`: one struct, ten mutexes, `library_changed()` saves, emits, reloads and queues sync | Each service owns its state and file and takes the watches it reacts to. The reactions table above is the whole list. |
| MCP edits run a closure under the store lock | `library.transact`: the same atomicity, owned by the library |
| Nine emitted events carrying whole payloads; the frontend mirrors them in a store and patches optimistically | One source of truth. `Watch<T>` snapshots; the UI pulls. No mirror, so no rollback problem (FEATURES §7 "optimistic store mutations lack rollback" disappears). |
| Hand-offs through mutex fields plus an event | Session values owned by the window that uses them |
| `Result<T, String>` | A `thiserror` enum per domain; `Display` is the user-facing sentence |
| Saves log errors and return success | A commit returns a revision; `saved()` says which revision is on disk and whether the last write failed |

A UI-side **action registry** (§1.3) is not a command layer: it is typed, lives in `kudzu-ui`, and exists so one definition feeds key routing, the ⌘K menu, the cheat sheet and menu labels.

**Threading.**

| Thread | Runs | Rule |
|---|---|---|
| Main | Rune's event loop, all window state, every `kudzu-appkit` call | A handler or `poll` must not block: no file, network, pasteboard or AX call. Target under 2 ms; Rune's debug build warns at 8 ms. |
| Engine threads (tap run loop, worker, 1 s poll) | Inside `kudzu-engine`. They call `EngineEvents`. | Never call the UI. Never hold an engine lock across a callback (already true). |
| `tasks` threads (one per operation) | Import, export, backup, restore, selection capture, paste into target | Results commit through the owning service, then its `Watch` wakes the UI. |
| `schedule` (one thread, a timer heap) | Daily backup check (startup, then 6 h), clipboard prune (10 min), update check (20 s, then 6 h), sync tick (3 min) | Replaces one sleeping thread per loop. |
| Clipboard watcher | 300 ms `changeCount` poll | Reads the pasteboard only when the count changed and the change is not ours. |
| Persistence writer | Debounced atomic writes | The only thread that writes domain files. Publishes `saved()`. |
| CloudKit queue, MCP listener and client threads | Adapters | Call services like any other thread. |

Services are `Send + Sync`. A service never calls another service while holding its own lock; it commits, unlocks, then wakers run. That one rule removes lock-ordering as a concern.

Global shortcuts are registered and unregistered on the main thread only, and `shutdown()` joins no thread that could be waiting on the UI loop (`AppHandle::register_shortcut` blocks an off-thread caller until the loop answers).

**Shutdown has one order**, whatever starts the quit: (1) windows commit open drafts; (2) the agent socket closes; (3) the engine and the clipboard watcher stop; (4) every writer drains synchronously, then sync checkpoints; (5) a verified update installs; (6) exit. Steps 2–5 are `Kudzu::shutdown()`: synchronous and idempotent, so the same call serves the tray's Quit and Rune's `on_quit` hook, after which the system may end the process at once.

### 1.3 UI architecture on Rune

**Process and windows.** One winit loop, started like `examples/launcher/src/main.rs`: `rune::run_windows_with_args(RunOptions { accessory: true }, rune_args, factory)` (`crates/rune/src/tooling.rs`), each window a `WindowSpec::from_app(key, app)` with its own state type (`AnyWindowSpec`).

| Key | State | Rune configuration | Tauri equivalent |
|---|---|---|---|
| `main` | `MainState` | `App::new(spec).keep_running().standard_menus().chrome(Chrome { full_size_content: true, traffic_lights: Some((18.0, 22.0)), drag_regions: true })`, transparent, 1180×760, minimum 900×560; `.windowless()` when launched `--hidden` after first run | main webview |
| `palette` | `PaletteState` | `.windowless()` + `WindowOptions::launcher_panel()` with `corner_radius(18.0)`, 760×480 (`crates/rune/src/window/spec.rs`) | palette webview |
| `picker` | `PickerState` | launcher panel, 800×520, radius 18 | clipboard webview |
| `form` | `FormState` | launcher panel, width 440, radius 16; height follows content through `tx.resize_window`, 160..640 | form webview |
| `permissions` | `PermissionsState` | `.windowless().keep_running()`, borderless, transparent, always on top, all Spaces, 420×660, radius 18 | permissions webview |
| guide overlay | `GuideState` | `App::overlay` (click-through, non-activating); native fallback in §3 | `macos.m` panel |

Launcher panels are prewarmed hidden and take keys without activating the app (`crates/rune/src/platform/macos.rs`), which is what the palette, picker and form need. They hide on blur with `App::on_focus` (palette and picker only; the form has no blur dismissal, FRM-6).

**Window state.** Each window state has three parts:

```rust
pub struct PaletteState {
    svc: PaletteServices,          // the Arc<Service>s this window needs, nothing more
    shell: Shell,                  // window coordination (below)
    model: PaletteModel,           // seen revisions, snapshots, derived data, local UI state
}
```

Screens inside the main window follow the same split per screen module: `model.rs` (state and transitions, no `El`, unit-tested), `view.rs` (`fn(&Model, &Theme, …) -> El`, thin), `scenes.rs` (fixture states for headless runs). Widgets shared by two windows (clipboard rows and previews are used by `main` and `picker`) are generic over the window state and take an accessor, as rune-kit components do: `clip_list::view::<S>(model, |s: &mut S| &mut s.clips, theme)`.

**Domain state reaches views by pull.** `compose.rs` subscribes each window's `AppHandle::wake` (`crates/rune/src/app.rs`) to the watches that window reads. `AppSpec::poll` then calls `model.sync(&svc)`, which compares `Watch::revision()` values with the ones it saw, pulls changed snapshots, recomputes derived data, and returns `true` only when something visible changed. A hidden ordinary window runs `poll` but not its view, and builds once with the latest state when shown (Rune README, "App lifetime"). A hidden launcher panel is different: it redraws its cached frame when `poll` reports a change (`crates/rune/src/window/lifecycle.rs`, `poll_hidden`), so a panel's model reports a change while hidden only for what the panel shows at the moment it opens. An idle app draws nothing.

**User intent reaches the domain by direct call.** A handler (`.on_press(|s: &mut MainState| …)`) updates local UI state and calls a service method. Calls that mutate in-memory state are synchronous and cheap. Calls that do I/O return a `Task<T>` or flip a `Watch`; the handler never waits. Keystroke-rate queries (palette search, clipboard search, editor preview) use `rune::Latest<T>` (`crates/rune/src/latest.rs`): the request closure runs a pure function on a snapshot on a worker, and `poll` lands the newest answer.

**Background work feeds the UI the same way.** Engine status, expansions, clipboard captures, sync merges, update progress and agent edits all go: adapter or worker → service commit → `Watch` → `wake` → `poll` → view. There is no second path.

**Window coordination.** `crates/kudzu-ui/src/shell/` owns it. `Shell` holds every window's `AppHandle` (they are `Clone + Send + Sync`) and exposes `open_main(Route)`, `toggle_palette()`, `toggle_picker()`, `open_permissions(Kind)`, `close_windows()`, `quit()`. Rules:

- A window is told what to do with `handle.update(|s, tx| …)`, then shown. `Route` is a typed enum (`Snippet(id)`, `NewSnippet(Draft)`, `Settings(Pane)`, `Clipboard(Filter)`, …). This is what `kudzu://navigate` and `kudzu://new-snippet` did.
- Windows that react to domain state decide for themselves. The form window sees `forms.pending()` change in its `poll` and shows itself at the caret; the permissions window sees `permissions.take_pending_restart()`. The application layer never asks the UI to present something.
- Placement uses `AppHandle::place` and `set_position` (`app.rs`). Screen geometry comes from `kudzu-appkit::screens`, because Rune's display list is a startup snapshot (#146).
- Lifecycle: `main` closes to hidden (`keep_running`) and drops the Dock icon with `AppHandle::set_dock_icon`. `App::on_reopen` opens `main`. ⌘Q closes windows (§3, gap 2). Hiding does not run Rune's close hook (`AppHandle::hide`), so every path that hides `main` (close button, ⌘Q, `close_windows`) commits the editor's draft first. Quit follows the order in §1.2: the tray's Quit runs one `update` on `main` that commits drafts, calls `Kudzu::shutdown()`, then `tx.quit()`; `App::on_quit` on `main` does the first two synchronously for a system-initiated quit (Dock, logout), where Rune runs quit hooks before it closes windows.
- Global shortcuts: the palette and picker Apps register theirs with `AppHandle::register_shortcut` (`crates/rune/src/shortcuts/mod.rs`). `Shell` implements `HotkeyRegistrar`, so `kudzu_app::hotkeys` keeps the collision and rollback rules (SHL-6) testable without Rune.

**Actions.** `crates/kudzu-ui/src/actions/` defines one typed `Action` enum per window with a label, a `rune::Chord`, an enabled predicate and a run function. `AppSpec::on_key` routes through it; the ⌘K menu, the cheat sheet and menu-bar labels read it. Shortcuts are declared once (LIB-13, LIB-14).

**Focus.** Rune has no `Tx::focus(key)` (#173). Text fields take focus through `TextEdit::focused`. Lists use a window-level focus model (`crates/kudzu-ui/src/focus.rs`): `on_key` sends keys to the pane the model marks active, and the pane draws its own ring. This gives ⌘L, ⌘E and Escape their current behaviour without programmatic element focus.

**Lists.** `El::virtual_rows(count, estimate, key, build)` with ids as keys, `virtual_rows_version(model.version)`, and `rune::memo(id, row_revision, …)` inside the row builder (`crates/rune/src/el/virtual_rows.rs`). Handlers find their item by id, never by index. Hover is painted with `El::hover` and `hover_group`, never stored in state. Change-aware motion (FEATURES §3: more than 12 snippet or 10 clipboard changes stay still) is a census in the model that selects `Tx::instant()`.

**Headless and scenes.** `main.rs` forwards Rune's tooling flags and decides the launch mode before anything is constructed: `--mcp` → the bridge; `rune::is_headless()` → fixtures; otherwise the real app. `is_headless()` (`crates/rune/src/tooling/args.rs`) reads the process arguments, so it also covers `--list-windows` and `--list-scenes`. `headless_run()` cannot be used for this: Rune publishes it after the window factory has run. In headless mode `compose.rs` wires `kudzu_app::fixtures` and never calls `Ports::macos`: no engine, no CloudKit, no socket, no real directories. Fixtures fix the clock, time zone, locale and UUIDs, and models run their `Latest` queries inline, so a snapshot never races a worker. Each window's `AppSpec::scenes` names fixture states, so `kudzu --window main --scene settings-about --theme dark --snap out.png --tree out.json` is deterministic. `fixtures` carries the sample data of the Tauri app's browser preview (`lib/mock.ts`, captured by S02) so scenes match the reference screenshots. Scenes take over the job of the preview's `?view=` query params.

### 1.4 Design system

`crates/kudzu-ui/src/theme/` is hand-authored Rust, written once from `apps/desktop/app/globals.css`. No generator, no Node.

| File | Content | Source in `apps/desktop` |
|---|---|---|
| `palette.rs` | Every colour token, light and dark, as `rune::oklch(l, c, h)` with alpha; `iris`, `surface`, `hairline`, `faint`, chips, success, warning, favorite, chart ramp, vibrant sidebar alphas (.72 / .55) | `globals.css:179`, `:254`, `:247` |
| `depth.rs` | The 12 elevation tokens (`card`, `raised`, `raised_hover`, `pressed`, `well`, `knob`, `key`, `accent`, `selected`, `soft`, `float`, `panel`) as `&[rune::Shadow]` per theme, each CSS layer one `Shadow { x, y, blur, spread, color, inset }` | `globals.css:222`, `:298` |
| `radius.rs` | 6 / 8 / 10 / 14 / 18 / 22 / 26 and the component radii 5, 12, 16, 20, 24 | `globals.css:74` |
| `type_.rs` | Text styles: body 13/20, metadata 10–11.5, controls 12–13, preview 13.5, search 17, name 22, headline 23/28, stats 26, mono 13/1.7, chip 11.5 medium; tabular figures for stats | FEATURES §3 |
| `motion.rs` | Durations 80/160/240 ms. `SPRING_SOFT` and `SPRING_POP` twice: as `rune_motion::Easing::Piecewise` with the exact CSS `linear()` samples (timed tweens, `Beat::easing`), and as `Spring::new(duration, bounce)` fitted to those samples (interruptible layout and presence motion). Named sequences: rise, pop, check, tick, star, bar, shimmer, shake, arrive, vanish, settle. | `globals.css:116`, `:167` |
| `icons.rs` | The Lucide icons the app uses (about 90) as `static NAME: rune::Icon { name, view: 24.0, body }`, bodies copied from Lucide (ISC), stroke `currentColor` | imports of `lucide-react` |
| `fonts.rs` | `include_bytes!` of `assets/fonts/Figtree[wght].ttf` and `PaperMono-Variable.ttf`; families `("System Font, Figtree", "Paper Mono, ui-monospace")`. "System Font" is SF Pro (`crates/rune/src/text/system_fonts.rs`). Rune reads TTF/OTF, not WOFF2, so Paper Mono is converted once and committed. | `globals.css:13`, `:174` |
| `kit.rs` | `fn kit(theme) -> rune_kit::Kit`: maps the tokens above onto `Palette`, `Elevation`, `Type`, `Radius`, `Motion`, `Glyphs` (`crates/rune-kit/src/kit.rs`) | |
| `mod.rs` | `Theme { kit, c, depth, ty, motion }`; `Theme::resolve(preference, cx.system_dark())` | `lib/theme.ts` |

Windows set `font_smoothing(FontSmoothing::Off)` (the `-webkit-font-smoothing: antialiased` look) and keep Rune's default sRGB blending, which is how the CSS tokens were authored. A theme flip applies in every window in the same turn with `Tx::instant()` for colours (FEATURES §3: no colour transition on theme change).

**shadcn primitive → Rune.** Use the rune-kit component with Kudzu's `Kit`. Do not rebuild a component rune-kit has.

| shadcn (`components/ui`) | Backed by | Note |
|---|---|---|
| button, toggle, toggle-group | `Button`, `IconButton`, `SegmentedControl` | Heights 24/28/32/36 and the raised/pressed depth must match; see the policy below |
| switch, checkbox, radio-group | `Switch`, `Checkbox`, `RadioGroup` | Switch 32×18.4 and 24×14, knob stretch on press |
| input, textarea, input-group, label | `TextInput`, `TextArea`, `Field` | Well depth at rest, ring on focus |
| select | `Select` | |
| popover, tooltip | `Popover`, `Tooltip` | `Tooltip`, not `El::help`: the native tooltip does not match |
| dialog, alert-dialog | `Dialog` on `El::modal` | |
| dropdown-menu, context-menu | `Menu`, `context_menu` | |
| command | `PopupList` + `TextInput` in a modal layer | The same list drives the editor's slash menu |
| scroll-area | `.scroll()` + `Kit::scroll_bar` | One key per viewport (#552) |
| separator, badge, collapsible | `Divider`, `Badge`, `Collapsible` | |
| kbd | `Keybind` | Wrapped by `widgets::shortcut` for spoken labels |
| sonner | `Toast`, `ToastRegion` | |
| tabs | `shell::TabStrip` | |
| slider | none (#149) | `widgets::slider` on core's slider role and `on_step` |
| breadcrumb | none | `widgets::breadcrumbs` |
| item | none needed | plain `row()` composition |
| app frame | `rune_kit::shell::{SplitView, Sidebar, SidebarSection, SidebarRow, Toolbar, Page}` | Sidebar rows give depth, in-place rename, reorder and virtual rows |
| release notes | `rune_kit::Markdown` | after the clean-up pass in `kudzu_core::release_notes` |
| image zoom | `rune_kit::ImageZoom` | clipboard large preview |

**Kudzu compositions** (`crates/kudzu-ui/src/widgets/`, each small, each used in two or more places): `shortcut`, `trigger_chip`, `selection_pill`, `breadcrumbs`, `slider`, `stepper`, `toggle_button`, `app_icon`, `app_picker`, `formatted_text`, `secret_text`, `empty_state`, `settings_card`, `key_hints`, `wordmark`.

**When a rune-kit component cannot match.** S06 renders every primitive beside its reference and records the result in `docs/migration/PARITY_MATRIX.md`. For a mismatch: first theme it through `Kit`; then decorate the built `El`; if a metric still cannot be reached, file a focused upstream issue and add a wrapper in `crates/kudzu-ui/src/widgets/kit_ext/<component>.rs` built from Rune core, tagged `RUNE-GAP`. A wrapper needs a row in the matrix that names the unreachable metric. This is the only door to local component code.

### 1.5 Platform layer

**`kudzu-engine` after restructuring (S34).** It keeps everything on the keystroke and insertion path, and everything that shares its bookkeeping: `tap`, `hold`, `input`, `input_source`, `keys`, `plan`, `status`, `activity`, `ax` (permissions, caret, selection, Secure Input), `pasteboard` and `clipboard` (NSPasteboard read, write, change count and the own-change ring, which snippet pastes and history writes must share), `frontmost_app`, `activate_app`.

What changes, under the 73 copied tests plus the protocol tests S39 adds first:

- `macos/engine.rs` (1,212 lines) becomes `macos/engine/{mod,session,match_flow,insert,form_flow,explicit,activity}.rs`.
- The crate is macOS-only. `other.rs`, the stub that let the Tauri app compile elsewhere, is deleted, with the "API contract for the Tauri app" comment.
- Shared value types move to `kudzu_core::system`, so the application layer needs no mapping.
- Serde derives that existed only for IPC are dropped.
- The `system` grab-bag shrinks to expansion: installed-app enumeration, icon rasterising, image thumbnails, Settings deep links and locale move to `kudzu-system` (S35).
- S39 adds seams around three things only (posting events, the pasteboard, clock and focus observations) and tests that pin paste-and-restore, held-input replay, sequence cancellation and form interruption before any code moves. It does not turn every platform call into a trait.
- One process-wide singleton stays, on purpose: the pasteboard's pending-restore and own-change record in `clipboard`. The system pasteboard is shared by the whole process, and snippet pastes and history writes must see the same record. It gets one owner type with an explicit shutdown.

**`kudzu-system`**: one module per port. `Ports::macos(identity)` in `lib.rs` builds them all; S40 writes it, and it panics if called in a headless run.

| Module | Implements | Uses |
|---|---|---|
| `engine` | `ExpansionEngine`; forwards `EngineHost` callbacks to `EngineEvents` | `kudzu_engine::Engine` |
| `pasteboard` | `Pasteboard` (change count, read, write, paste into target) | `kudzu_engine::clipboard` |
| `apps`, `thumbnails` | `AppDirectory`, `Thumbnailer` | NSWorkspace, NSImage |
| `settings_panes`, `files` | `SystemSettings`, `FileOpener` (open, reveal) | NSWorkspace |
| `login_item` | `LoginItem` | `auto-launch` |
| `instance`, `relaunch` | single instance (lock file; a second launch asks Launch Services to reopen the bundle, then exits), `Relauncher` | |
| `update_install` | `UpdateInstaller` (extract, swap the bundle, translocation and writability checks) | `/usr/bin/tar` |
| `cloudkit/` | `CloudStore` over `CKSyncEngine` | `objc2-cloud-kit` |

**`kudzu-appkit`**: AppKit glue for Rune gaps. One file per gap, a safe API of plain types and closures, no knowledge of Kudzu. Each file's header names the upstream issue and what replaces it. §3 lists them.

### 1.6 Persistence, errors, logging, identity, packaging, updates, MCP, iCloud

**Files.** Formats stay, because they are tested and one of them (Kudzu JSON) is also the export and backup format. With no users there is nothing to migrate, so compatibility code is not written.

```
~/Library/Application Support/<bundle id>/
  library.json        Kudzu JSON, formatVersion 1 (additive fields only)
  settings.json       missing keys take the variant's defaults
  form-memory.json    ≤ 200 snippets
  ui-state.json       sidebar section folds (the Tauri app kept these in localStorage)
  device-id
  backups/            kudzu-backup-<local timestamp>-<reason>[.N].json, newest 14
  clipboard/          history.json, items/<uuid>.kzc (KZC1), thumbs/<uuid>.png, trash/   (0700)
  sync/               index.json, engine-state.data
  agent.sock          0600, only while agent access is on
  permission-setup-pending
~/Library/Logs/<bundle id>/Kudzu.log   2 MB × 3
```

Not carried over, on purpose: the copy from `sh.saeed.kudzu.desktop`, the `kudzu-theme` localStorage key (the theme is in `settings.json`), the `kzclip://` protocol (the UI reads media bytes through `clipboard.media`, with the same checks: known entry, allowed path, 80 MiB image limit, 64 px icons).

`persist::JsonFile<T>` does every write: unique temp name, `fsync`, rename; a file that fails to parse is moved aside (`*-corrupt-<ts>.json`) before defaults are used, for the library, settings and form memory alike; if it cannot be moved aside, nothing is written over it. Write failures surface through `saved()` (§1.2) instead of being logged and forgotten (FEATURES §7 "disk success is weaker than API success"). Backups prune to 14 at the same points as in the Tauri app (daily and manual).

**Errors.** One `thiserror` enum per domain in `kudzu-app`. `Display` is the sentence the user sees (the strings the Tauri commands return are the baseline); `source()` carries the cause for the log. No `anyhow` in library crates. The UI maps an error to a toast or inline message in one place per screen.

**Logging.** The `log` facade stays (core and engine already use it). `src/logging.rs` installs the sinks: `~/Library/Logs/<bundle id>/Kudzu.log` rotated at 2 MB keeping 3, plus stderr; `Info` in release, `Debug` in debug (SHL-12).

**Identity.** `src/identity.rs` resolves one `Identity` at startup and passes it down; nothing else reads the bundle id.

| | Production | Dev |
|---|---|---|
| Chosen by | `CFBundleIdentifier == sh.saeed.kudzu` | any other bundle id, **or no bundle** (`cargo run`) |
| Name, MCP name | Kudzu, `kudzu` | see below |
| New-data defaults | not paused, ⌃⌥Space, ⌃⌥V | paused, ⌃⌥⇧Space, ⌃⌥⇧V |
| iCloud, updates | allowed when entitled | never |

An unbundled binary is Dev, so a stray `cargo run` can never touch production data. **Until cut-over the Dev bundle is "Kudzu Next" (`sh.saeed.kudzu.next`, MCP name `kudzu-next`)**, so it runs beside both the installed Kudzu and the Tauri app's Kudzu Dev with its own data, permission grants and login item. S75 renames it to Kudzu Dev (`sh.saeed.kudzu.dev`, `kudzu-dev`). `cargo xtask bundle --variant dev|prod` writes the bundle id. `Kudzu --mcp --dev` keeps its meaning.

**Packaging and signing.** `xtask` does what the Tauri CLI and the two `.mjs` scripts do for the Tauri app:

- `cargo xtask bundle --variant … [--universal]`: builds, `lipo`s, lays out the `.app` (executable `Contents/MacOS/Kudzu`; `Info.plist` with `LSUIElement`, version from `version.txt`, minimum macOS 14; icon; `Resources/skills/kudzu/SKILL.md`), embeds the provisioning profile and iCloud entitlements only when the profile is present.
- `cargo xtask run`: build, bundle the Dev variant, sign with `KUDZU_DEV_SIGN` when set (a stable identity keeps Accessibility grants across rebuilds), launch.
- `cargo xtask release`: in CI, import the signing certificate into a temporary keychain and delete it on exit, success or failure (the Tauri bundler did this for the old pipeline); codesign with the hardened runtime, notarize and staple the app, build the DMG with the current background and icon positions, sign and notarize it, write `Kudzu_<v>_universal.app.tar.gz` and its minisign `.sig`, `latest.json`, `SHA256SUMS.txt`; `cargo xtask feed` decides the beta feed (the semver comparison from `feed.mjs`, in Rust with tests).
- `scripts/release/build.sh` (a new file in `apps/desktop-v2`) is the entry point CI will call: the checks of today's script, then `cargo xtask release`.

**Version.** `version.txt` is the app version: `xtask` stamps it into `Info.plist`, the binary reads it with `include_str!`, and `cargo xtask set-version` writes it for betas. Crate versions stay internal, as in the Tauri app. This matches release-please's `simple` strategy, which updates `version.txt` and the changelog and nothing else; its `rust` strategy would re-version every workspace member. S75 sets the file to the value in `.release-please-manifest.json` and switches release-please to `simple`.

**Updates.** `kudzu_app::updates` owns the state machine (`Idle`, `Checking`, `UpToDate`, `Downloading`, `Ready`, `Restarting`, `Error { needs_move }`), the feed (`latest.json`, `beta.json`, unchanged shape), download through `Fetch` (time limits and a size cap), minisign verification against the public key compiled in, and the schedule. `UpdateInstaller` swaps the bundle on quit or on Restart, after checking the archive (one top-level `Kudzu.app`, no absolute or `..` entries) and the extracted bundle (bundle id, a version that is not lower, a valid code signature). Kudzu never restarts on its own. The existing updater key and GitHub secrets are reused: the Tauri key file is a base64-wrapped minisign key, so `xtask` decodes and signs with it; the workflows map the existing secret names to `KUDZU_UPDATE_KEY` and `KUDZU_UPDATE_KEY_PASSWORD`.

**MCP.** The server moves from `kudzu_core::agent` to `kudzu_app::agents`. The tools call `library`, `clipboard` and `expansion` directly; the `Host` trait and the lock-held closure go. The protocol stays synchronous JSON-RPC over a 0600 Unix socket; `Kudzu --mcp` stays a stdio relay with offline tool listing, a single reconnect and a 30 s timeout. No tokio.

**iCloud.** No record changes: `KudzuSnippet`, `KudzuGroup`, `KudzuUsage`, `KudzuMeta`, one `payload` string each, `SYNC_VERSION` 1, zone `Kudzu`, container `iCloud.sh.saeed.kudzu`. Production schema is add-only and nothing here needs to add. `kudzu_core::sync` keeps its rules. The CloudKit layer is rebuilt as an object: `cloudkit::Store` owns the engine, delegate, record cache and index (in the Tauri app they are process statics), implements `CloudStore`, and talks to `library` through `library.transact`.

### 1.7 Testing strategy

| Layer | What | Where | Gate |
|---|---|---|---|
| Core | The 377 copied tests keep passing; S20 changes only their `use` paths. New pure modules get tests plus **TS parity fixtures**: inputs and outputs dumped from the TypeScript implementation in S02 (search ranking, clipboard query and grouping, stats, chip labels and summaries, key labels, release notes, colour math, editor formatting transactions). | `crates/kudzu-core/tests` | CI |
| Engine | The 73 copied tests keep passing through S34. They are mostly pure planning and queue tests, so S39 first adds protocol tests behind seams (a controlled poster, pasteboard, clock and focus): paste and restore, held-input replay, sequence cancellation, form interruption, unknown image delivery. | `crates/kudzu-engine` | CI |
| Application | Each service against fakes: behaviours of the Tauri commands (duplicate naming, cycle refusal, undo, form memory eviction, retention), reactions, persistence round-trips, failure paths. The 10 Tauri shell tests are ported. | `crates/kudzu-app/tests` | CI |
| Screen models | `model.rs` transitions: selection anchors, scopes, sort, keyboard tables, autosave gates, onboarding recovery. No Rune needed. | `crates/kudzu-ui/src/**/model.rs` | CI |
| Element trees | The built binary run headless: `--window W --scene S --tree`. Assert roles, labels, order, focusability, key landmark rects, and accessibility names. Scripts (`--script`) drive keys and presses, then assert the tree. | `tests/ui_*.rs` (root package) | CI |
| Goldens | `--snap` PNGs for the component gallery and one scene per window per theme. Rendered with bundled fonts (`KUDZU_FONTS=bundled`) so they do not depend on the OS's SF Pro. Few on purpose: they catch regressions in tokens and components. | `tests/golden/` | CI on macOS |
| Motion | `rune::motion_gate` on list row arrival, selection pill, sidebar collapse, palette open. | `tests/motion_*.rs` | CI |
| Native | S71 and S72: live expansion, paste, undo, IME, forms at the caret, picker paste, permissions, Spaces, displays; then the release candidate, lifecycle, iCloud, updates and budgets. Needs a Mac with grants. | checklists in S71, S72 | manual, recorded |

**CI.** S01 adds `.github/workflows/desktop-v2.yml`: it runs on every push and pull request that touches `apps/desktop-v2/**`, on `xcode-27`, and runs the gate of rule 8 plus the headless tests. The existing workflows are not touched until S75, when this job replaces the `desktop` job.

**UI parity against the Tauri app.**

1. **Capture early (S02).** Run the Tauri app's browser preview (`pnpm dev` in `apps/desktop`) and capture, at 2× and at each window's real size, every state its query params reach: `?view=insights`, `?view=clipboard`, `?view=settings:<pane>` for all eight panes, `&emptyactivity`, `?onboarding` with `&espanso`, `&espanso=retry`, `&finish=retry`, `?update`, `/form?demo`, `/clipboard`, `?clips=5000`, `?emptyclips`, `/permissions`, `/palette`, each with and without `?light`; plus scripted states (command menu, cheat sheet, group dialog, context menus, chip popovers for every tag, formatting toolbar, bulk bars, empty states, toasts). Save `docs/migration/reference/<window>/<state>.<theme>.png` and `reference/metrics.json` (landmark rects, computed colours, font sizes).
2. **Same data.** `kudzu_app::fixtures` reproduces the preview's sample library, clipboard and statuses, and scenes carry the same names as the reference states.
3. **Compare (per screen step, and everything again in S73).** `cargo xtask parity` (built in S06) renders each scene on a Mac with the system font, as the reference was captured, then writes a side-by-side sheet and a landmark report. Gates: every landmark rect within 2 pt of the reference; token colours equal. Pixel equality is not a gate: WebKit and Rune rasterise text differently. Bundled-font goldens are regression tests only and prove nothing about parity. A difference that stays needs the owner's approval, not a reviewer's.
4. **What screenshots cannot show** is checked by hand in S72, with the Tauri app running beside the new one: vibrancy, native shadows, window corner clipping, real motion, focus behaviour across apps.

**Performance budgets.** S02 records the baseline from the installed release build of the Tauri app; S72 measures a release build of the new app on the same Mac. Both notes record the machine, macOS version and display. The Rune app must meet the budget and not be slower than the baseline. Launch and panel numbers come from marks the app writes to the file named by `KUDZU_TIMINGS` (`src/timing.rs`, S03): `services_ready`, `main_first_frame`, `shortcut_received`, `panel_frame`. Rune's `RUNE_READY_DIR` is not used: it fires when the first window is ready, which here is a hidden panel. A budget missed because of a Rune defect at the pin is recorded with its upstream issue and goes to the owner in S74.

| Metric | Budget | Measured by |
|---|---|---|
| Cold start to services running, hidden launch | ≤ 250 ms | `services_ready`; median of 7 warm launches after one discarded |
| Launch to main window content | ≤ 400 ms warm | `main_first_frame` (the first `Drawn` frame of `main`); same procedure |
| Hotkey to palette or picker frame encoded | ≤ 16 ms p95 handler-to-encode; ≤ 80 ms to visible | `shortcut_received` to `panel_frame` over 100 opens; screen recording for visible. #335 applies. |
| Picker open with 5,000 clips | same as above | same, `--scene clips-5000` |
| Trigger completed to first synthetic event | ≤ 15 ms p95; tap callback ≤ 0.2 ms p99 | engine timing test on S39's poster seam; a debug timing log in S72 |
| Palette search, 5,000 snippets | ≤ 2 ms p95 per keystroke | `cargo test --release` timing test in core |
| Clipboard search, 5,000 clips | ≤ 5 ms p95 | same |
| Idle CPU, windows hidden | ≤ 0.3%, 0 frames presented in 20 s | `on_frame_encoded` counter per window (hidden panels counted separately), `ps` sampling; no caret or shimmer on screen |
| Idle, main window visible | 0 presents in 20 s | same |
| Memory footprint, hidden with panels prewarmed | ≤ 150 MB and below the Tauri baseline | `footprint` after 60 s. #161 and #162 apply. |
| Virtual list scroll, 5,000 rows | 0 view commits during scroll; frame CPU ≤ 4 ms p95 | headless scroll script: per-frame CPU from the run's timing output, commits from `RUNE_COMMIT_BUDGET_MS`; Rune's `scripts/app-audit --strict` on `kudzu-ui` |

---

## 2. Dependency decisions

The test for adopting a crate is the owner's: it gives **the same result** and lets us **move faster**. Where a `CRATES.md` recommendation fails that test it is overruled here, with the reason. Versions were checked on crates.io on 2026-11-11; re-check with `cargo info` when adding.

**Approval** column: *approved* = a new direct dependency or tool, approved by the owner on 2026-11-11; *existing* = already used by the Tauri app's Rust code; *none* = no dependency.

| # | Concern | Decision | Crate@version | Reason | Approval |
|---|---|---|---|---|---|
| 1 | UI framework | adopt | `rune` (feature `native-file-dialog`) and `rune-kit` (feature `markdown`), git, pinned rev | The goal. Brings `rfd` 0.17 and a Markdown parser transitively. | given by the goal |
| 2 | Secret detection | **keep ours** | — | The candidates (`secrets_scanner` 0.2.3, `ripsecrets` 0.1.11, `redact-secret` beta) each have under 20k downloads and are built for scanning source files. None gives the same result: no lone-password detection (the main clipboard case), no labelled-value parser with placeholder exclusions, no whole-text flag, no kind-tagged ranges for our markers. The 742-line module exists and is pinned by tests, so a swap would be slower and change behaviour. Confirmed by the owner; `secrets_scanner` is trialled as an extra token source after the audit (§6). | none |
| 3 | MCP server | **keep ours**, moved to `kudzu-app::agents` | — | `rmcp` 3.9.0 is the maintained SDK, but it does not pass the test here: the protocol layer we own is about 200 lines and already tested; the tools (where the logic is) would be rewritten as handlers either way; and it brings tokio into an app that is otherwise threads. (Its transports could carry our stdio-to-socket bridge; topology is not the obstacle.) Spec drift is covered by replaying recorded client transcripts as fixtures. Revisit if we need streamable HTTP, OAuth or elicitation. | none |
| 4 | Fuzzy search and ranking | **keep ours** (port `lib/search.ts` to `kudzu_core::search`) | — | Overrules `CRATES.md` (frizbee). Behaviour must be kept: frizbee's Smith-Waterman is typo-tolerant, so it matches queries the current subsequence matcher rejects and reorders near ties. The port is 209 lines, pinned by TS parity fixtures, and already meets the budget. | none |
| 5 | Global hotkeys, shortcut recorder | Rune | `App::global_shortcut`, `AppHandle::register_shortcut`, `rune_kit::ShortcutRecorder` | Same Carbon mechanism as the Tauri plugin. Overrules `global-hotkey`. | none |
| 6 | Login item | adopt | `auto-launch@0.6.0` | The LaunchAgent route the Tauri plugin uses, so `--hidden` at login behaves the same. SMAppService (`objc2-service-management`) passes no launch argument and would change that; it can follow later. | approved |
| 7 | Single instance | keep ours | — | A lock file plus a Launch Services reopen; about 40 lines. `rune::single_instance` is a no-op on macOS. | none |
| 8 | Update check and download | adopt | `ureq@3.5` | Blocking HTTP with rustls; fits threads. | approved |
| 9 | Update signature verification | adopt | `minisign-verify@0.3.0` | What the Tauri updater uses; no dependencies. | approved |
| 10 | Update signing (xtask only) | adopt | `minisign@0.10.0` | Signs with the existing updater key; never linked into the app. | approved |
| 11 | Version comparison | adopt | `semver@1` | Stands in for the hand-written comparison in `feed.mjs`; used by `updates` and `xtask`. | approved |
| 12 | Update extraction | system tool | `/usr/bin/tar` | Always present on macOS; no crate. | none |
| 13 | Logging | keep `log`; adopt a sink | `flexi_logger@0.31` | Size rotation at 2,000,000 bytes keeping 3 files plus stderr, on the facade core and engine already use. Overrules `CRATES.md` (tracing): `tracing-appender` rotates by time only, so it cannot reproduce the current log files, and `tracing-oslog` adds a behaviour we do not have. | approved |
| 14 | Date and time formatting | **keep ours** | — | Overrules `CRATES.md` (icu_datetime) for now. ICU4X changes output for non-English locales (digits, abbreviations) and sits on the render path. It is a product improvement, not parity; propose it after S74. | none |
| 15 | Colour card (parse, HSL, OKLCH, contrast) | adopt | `csscolorparser@0.9.1` | Real CSS parsing in place of the canvas trick. OKLCH and WCAG contrast are computed from its OKLab and linear RGB in a few lines, so `palette` is not needed. `parse_color` stays the classifier. | approved |
| 16 | Editor image compaction | adopt as a direct dependency | `image@0.25` | Already compiled through Rune at this version. Decode PNG/JPEG/WebP, resize, encode PNG in `kudzu-app`. Clipboard thumbnails keep AppKit (HEIC, TIFF, PDF). | approved |
| 17 | Base64 | keep, and use it in core too | `base64@0.22` | Used by the Tauri shell. S20 replaces core's hand-written codec in `rich/image.rs` with it, provided the copied image tests pass unchanged. | existing |
| 18 | File dialogs | Rune | `Tx::open_file`, `save_file`, `open_folder` | Overrules a direct `rfd` dependency. One path per call is enough for every import and export dialog. | none |
| 19 | Open URL, open or reveal file | Rune + ours | `Tx::open_url`; NSWorkspace in `kudzu-system::files` | Overrules `open`. Rune refuses `file:` (#147). | none |
| 20 | App directories | Rune | `rune::dirs::AppDirs` in the bin | Overrules `dirs`. Paths are passed into `kudzu-app`. | none |
| 21 | Async runtime | no tokio | threads, `std::sync::mpsc`, `parking_lot@0.12` | Nothing needs async. | existing |
| 22 | YAML (Espanso) | keep | `yaml-rust2@0.11` | Works; bumping to 0.13 is unrelated to this work. | existing |
| 23 | RTF, HTML and Markdown readers; HTML output | keep ours | — | Their job is bounded conversion to spans with named losses, not parsing. | none |
| 24 | Clipboard access | keep ours | `objc2-app-kit@0.3` | Multi-item raw snapshot and restore, private markers, own-change tracking. | existing |
| 25 | Persistence | keep JSON files | `serde_json@1` | Human-readable, tested, and the export format. | existing |
| 26 | CloudKit | keep | `objc2-cloud-kit@0.3`, `block2@0.6` | The only binding. | existing |
| 27 | AX, CGEventTap, TIS FFI | keep ours | `core-foundation@0.10`, `objc2@0.6` | Works and is tested. Swapping the hand-declared CGEvent and AX externs for `core-graphics` and `accessibility-sys` would be risk in the most delicate crate for no change in behaviour; not now. Rune links `objc2` 0.5 internally; both versions coexist, and their types are never cast across. | existing |
| 28 | AppKit glue for Rune gaps | keep | `objc2-app-kit@0.3`, `objc2-foundation@0.3` (more features) | Status item, menu, visual effect view, appearance, screens. | existing |
| 29 | Info.plist and LaunchAgent files | keep | `plist@1` | Used by the engine; also by `xtask`. | existing |
| 30 | Errors, ids, time, Unicode | keep | `thiserror@2`, `uuid@1`, `chrono@0.4`, `unicode-segmentation`, `unicode-normalization` | | existing |
| 31 | Syntax highlighting, file watching | nothing | — | Not features today. | none |
| 32 | DMG layout (build tool) | adopt | `create-dmg` (Homebrew) | The script Tauri's bundler runs, so the DMG looks the same. | approved |
| 33 | Reference capture (S02 only) | temporary tool | Playwright through `pnpm dlx`, not added to any `package.json` | Lives in `docs/migration/reference/`; deleted in S75. | approved |

**All approved by the owner on 2026-11-11:** `auto-launch` 0.6, `ureq` 3.5, `minisign-verify` 0.3, `minisign` 0.10 (xtask only), `semver` 1, `flexi_logger` 0.31, `csscolorparser` 0.9.1, `image` 0.25 (direct), `create-dmg` (Homebrew), Playwright through `pnpm dlx` (S02 only). The scripts that call `create-dmg` and Playwright name the version they expect. Anything not in this table still needs approval (rule 5).

---

## 3. Rune gaps

**Filed 2026-11-11 by the orchestrator:** new issues #561–#564 (gaps 2, 3, 4, 13), and comments with our case on #141, #350, #128, #357, #147. Steps don't file issues themselves: report new gaps (title + 2-line body) in the step report and the orchestrator files them.

**Procedure.** Search the issue list first (`gh issue list -R anomalyco/rune --search …`). If an issue exists, add a comment with Kudzu's concrete case. If not, open one with the title and body below. Rune takes issues, not pull requests. Then build the workaround in the named module with a `RUNE-GAP(#n)` header. Workarounds stay behind the smallest API that serves Kudzu, so deleting one touches one module and its call sites. Module paths are under `crates/`.

| # | Gap | Upstream | Local workaround | Module |
|---|---|---|---|---|
| 1 | No menu-bar status item on macOS (`rune::Tray` returns `Unsupported`, `crates/rune/src/tray.rs`) | **#141** (and #305 for runtime updates). Comment: Kudzu needs a template image, a title ("Dev"), a tooltip, per-item enable and title updates, and key-equivalent hints. | `NSStatusItem` + `NSMenu`; item callbacks run closures on the main thread, which call `Shell`. Glyph variants and the update dot are drawn from the committed PNGs. | `kudzu-appkit/src/status_item.rs` |
| 2 | ⌘Q always terminates: Rune's `applicationShouldTerminate:` returns `NSTerminateNow` (`crates/rune/src/quit/mac.rs`) and the app menu Rune builds has Quit ⌘Q (`native_menu/default_macos.rs`) | **#561** (filed). Title: "macOS: let an app re-key or replace the app menu's Quit item". Body: "A menu-bar app wants ⌘Q to close its windows and keep running, with Quit as a separate item without a shortcut. `MenuItem` cannot take ⌘Q while the built-in Quit holds it, and `on_quit` cannot cancel. Related: #201." | Once the menu bar exists (first update), insert "Close Windows ⌘Q" into the first menu even while it is empty: Rune fills the app menu only on the first Dock promotion. After each promotion (queued from inside an update, never by timer), move it directly above `terminate:`, clear Quit's key equivalent, retitle it "Quit Kudzu", and remove Show All. Idempotent. Launcher panels also map ⌘Q to Close Windows in `on_key`. See the review findings in `docs/migration/spikes/S10-menubar.md`. Dock Quit and logout still terminate through Rune, so `on_quit` runs. No terminate veto, no swizzling. | `kudzu-appkit/src/app_menu.rs` |
| 3 | No Sidebar material: `SystemMaterial` is `Glass` or `Popover` (`system_material/mod.rs`) | **#562** (filed). Title: "System materials: a Sidebar material that follows the window's active state". Body: "`NSVisualEffectMaterialSidebar`, behind-window, `followsWindowActiveState`, for a transparent full-size-content window's sidebar." | Use `SystemMaterial::Popover` on the sidebar rect with the translucent fills (.72 light, .55 dark). S12 produces the side-by-side with the Tauri app and the owner decides from it (§6); if rejected, add an `NSVisualEffectView` under the content view, framed from the sidebar's drawn rect. | `kudzu-ui` sidebar; fallback `kudzu-appkit/src/sidebar_material.rs` |
| 4 | No API to force a window's appearance (Dark, Light, System), so native materials, traffic lights and menus follow the system, not the app setting | **#563** (filed). Title: "macOS: set the app or window appearance (force light or dark)". Body: "An app with its own Appearance setting needs `NSApp.appearance` or `NSWindow.appearance` set so system materials and native menus match the theme it draws." | Set `NSApp.appearance` from the theme preference. The theme itself is resolved from the preference, not from `Cx::system_dark()`, except for System. | `kudzu-appkit/src/appearance.rs` |
| 5 | No inline image in editable text: `Mark` has no image, and a field's pill icon is an `Icon`, not a bitmap | **#350.** Comment with the snippet-editor case (atomic inline image with width and alt text). | An image is a pill labelled with its alt text and an image glyph. Selecting it opens a popover with the real image, width and alt controls. The preview pane and the form and palette previews draw the real image. Accepted by the owner until Rune ships inline images; if it has not by then, S76 builds them in this module (§6). | `kudzu-ui/src/main/editor/images.rs` |
| 6 | No press handler on a pill (#128) and no text hit-testing (#353) | **#128, #353.** Comment: chip click opens its configuration. | Requirement: one click on a chip opens its configuration, as in the Tauri app. Mechanism: a click that leaves the selection on exactly one pill opens the popover, anchored at `Cx::caret(key)`; Return or Space on a selected chip opens it too. S13 showed a single click can't select a chip at 29aacb6. The owner chose a double-click or Return/Space instead (D15). | `kudzu-ui/src/main/editor/chips.rs` |
| 7 | No drop targets inside the app for non-file payloads | **#357.** Comment: dragging snippets and groups onto sidebar groups. | Rows use `El::on_drag`; drop zones register their drawn rects (`on_drawn_layout_tx` with `live_only`); the gesture hit-tests them and draws a ghost on a live offset. Stable hover without flicker (LIB-11). | `kudzu-ui/src/dnd.rs` |
| 8 | Display list is a startup snapshot | **#146.** Comment: placing panels under the pointer after a display change. | Read `NSScreen` on demand: screen under the pointer, visible frames, scale. | `kudzu-appkit/src/screens.rs` |
| 9 | Launcher panels take about 280 ms to appear | **#335.** | Prewarm at launch (already how launcher panels work). If S11 measures over budget, set the panel's animation behaviour to none. | `kudzu-appkit/src/panel_tuning.rs` (only if needed) |
| 10 | No `Tx::focus(key)` | **#173.** Comment: keyboard-first pane switching. | Window-level focus model; fields focus through `TextEdit::focused`. | `kudzu-ui/src/focus.rs` |
| 11 | rune-kit has no Slider | **#149.** | A slider composed from core's slider role, `on_drag` and `on_step`. | `kudzu-ui/src/widgets/slider.rs` |
| 12 | `Tx::open_url` refuses `file:`; no reveal in Finder | **#147.** Comment: clipboard history opens and reveals files. | NSWorkspace open and `activateFileViewerSelectingURLs`. | `kudzu-system/src/files.rs` |
| 13 | Pasteboard change count is private (`clipboard/native.rs`) | **#564** (filed). Title: "Clipboard: a public change counter or `on_clipboard_change`". Body: "A clipboard manager needs to know when the pasteboard changed without reading it." | Not a UI concern: the engine's NSPasteboard poll stays the source. Nothing to delete later unless we choose to. | `kudzu-engine` (`clipboard`) |
| 14 | No chart widget | none (not worth an issue) | Bars are `el()` with a width spring; count-up is a `LiveText`. | `kudzu-ui/src/main/insights/` |
| 15 | No date formatter in `rune-i18n` | none | Dates are formatted by `kudzu-core` (`template::icu`, `fmt`) and passed in as strings. | `kudzu-core` |
| 16 | Location of the System Settings window (for the permission companion) is outside any UI framework | none | `CGWindowListCopyWindowInfo` metadata only: no screen capture, no Accessibility. | `kudzu-appkit/src/settings_window.rs` |
| 17 | Kit component metrics may not reach Kudzu's (button heights, switch sizes) | **#573** (filed 2026-11-11, consolidated from the S06 matrix); shadows on hover/press in **#360** (commented). Title pattern: "rune-kit `<Component>`: take `<metric>` from `Kit`". | Wrapper per component, only when the matrix proves the metric unreachable, and only for buttons, inputs and switches; elsewhere rune-kit's metrics are accepted (§6). | `kudzu-ui/src/widgets/kit_ext/` |
| 18 | Colour output: at 29aacb6 `MetalSurface::configure` drops the layer's sRGB tag (#540), so colours can read more saturated on P3 displays; no Display P3 surface | **#540** (commented 2026-11-11, with the P3 request). | None in-app. Tokens are mapped to sRGB as CSS does (S04 fix). Check colour accuracy on a P3 display in S71/S73, and bump the Rune pin once #540 lands. | — |
| 19 | An overlay in a windowless accessory app comes up as a plain winit window, not the click-through, all-Spaces overlay panel (found by S15, confirmed by its review) | **#574** (filed 2026-11-11). | **Default:** the permission guide is a native click-through `NSPanel` (`ignoresMouseEvents`, level 3, `CanJoinAllSpaces \| FullScreenAuxiliary`, spanning the union canvas) that hosts the guide drawing. Requirements are in the S15 notes, "Review findings". | `kudzu-appkit/src/guide_panel.rs` |

**Not gaps** (corrections to `RUNE_FIT.md`): the permission companion's draggable app icon is `El::drag_out` with `DragItem::file(bundle)` (Rune README, "Dragging out to other apps": an app bundle can be dropped into System Settings' privacy lists); fullscreen toolbar inset is `Cx::titlebar_inset()` (0 in full screen); window corner rounding with a correct shadow is `MacWindowOptions::corner_radius`; read-only previews can draw bitmaps inline with `El::inline_icon(InlineIcon::image(..))` or as block elements; caret-anchored popups use `Cx::caret(key)`.

---

## 4. Steps

61 steps (S76 is conditional). Each is sized for one agent session. **Owns** lists the paths a step may create or edit; a step touches nothing else, which is what lets parallel steps run in separate worktrees. Paths are relative to `apps/desktop-v2/` unless they start with `apps/`, `.github/` or "root".

**Sizes:** S = under 300 changed lines, M = 300–900, L = 900–2,000. Nothing is XL; larger work is already split. Sizes count hand-written source and tests, not copied code, fixtures or assets.

**Models.** Implementer and reviewer always come from different providers. Claude is kept for the hardest work and for reviews where judgement matters. OpenCode Go models run at their default effort. If a named model is unavailable, the orchestrator uses the next model of the same provider at the same or higher effort, keeps the reviewer on a different provider, and says so in the step report.

**Order.** Steps in one wave can run in parallel once their dependencies have merged.

| Wave | Steps |
|---|---|
| 1 | S01, S02 |
| 2 | S03, S04, S05, S11, S13, S14, S20 |
| 3 | S06, S10, S12, S15, S21, S22, S23, S24, S25, S39 |
| 4 | S07, S26, S31, S34, S38 |
| 5 | S27, S28, S29, S30, S33, S36 |
| 6 | S32, S35, S37, S70 |
| 7 | S40 |
| 8 | S41, S56, S57, S66 |
| 9 | S42, S43, S50, S55, S58, S64, S65 |
| 10 | S51, S59, S61 |
| 11 | S52, S60, S62, S63 |
| 12 | S53, S54 |
| 13 | S71, S72, S73 |
| 14 | S74 |
| 15 | S75 |
| 16 | S76 |

Wave 7 is one step on purpose: S40 is the first point where every service and adapter exists, and it composes them.

**Merge rules for the orchestrator.**

- A step that creates a tree for later steps pre-declares their modules as documented empty files: S01 for every crate's `lib.rs`, S20 for `kudzu-core`, S41 for the main window's screens, S61 for the settings panes, S52 for the editor's tag forms and format bar. Later steps fill files they own and do not edit the parent module. If a step still needs a new module line, such conflicts resolve by union.
- S01 puts every crate the owner has approved in `[workspace.dependencies]`; later steps reference `workspace = true`.
- Append-only registries (`scenes.rs`, `actions/*.rs`, `tests/golden/`) hold one entry per line. Conflicts resolve by union, then `cargo fmt`.
- `ports/` and `fixtures/` in `kudzu-app` hold one file per port. The step that implements the service consuming a port may extend that port's two files; no other step touches them.
- A step passes the gate in its own checkout, on top of its declared dependencies only, before it is merged.
- `Cargo.lock` conflicts: take either side, run `cargo check`, commit the result. Never hand-edit it.
- A screen step builds against `kudzu_app::fixtures`, so it does not wait for real adapters.
- The orchestrator re-runs the gate of rule 8 after every merge.
- **Integration edits outside Owns are allowed** (added 2026-11-11 after S03): `<crate> = { workspace = true }` lines for approved dependencies in any crate's `Cargo.toml`; `Cargo.lock` as cargo generates it; one-line `mod` declarations, scene or window registrations, and subcommand dispatch entries in a parent module (`lib.rs`, `mod.rs`, `xtask/src/main.rs`, `src/compose.rs`). Nothing more. The step report lists each such edit. They resolve by union at merge.

**Step fields.** *Owns* is the scope. *Deliverables* is given where it adds something; where a step has none, the deliverable is the modules in *Owns*, behaving as *Accept* lists. *Accept* lists what must be true beyond the gate. *Closes* names the FEATURES checks the step satisfies (a check split across steps appears in each; S74 confirms them all).

### Foundations

#### S01 · The new app's workspace — M
- **Goal:** `apps/desktop-v2` exists as a self-contained Cargo workspace: core, engine and assets copied in, every crate present with its final module tree, a Rune window that opens, the isolation check, and its own CI job.
- **Owns:** `apps/desktop-v2/**` (not `docs/migration/`, which is already there), `.github/workflows/desktop-v2.yml` (a new file; the existing workflows are not edited).
- **Deliverables:**
  - Root manifest: `kudzu` bin; `version.txt` with the version in `.release-please-manifest.json`; members `crates/*`, `xtask`, `spikes/*` (spikes outside `default-members`); the release profile of `apps/desktop/src-tauri/Cargo.toml`; `[workspace.dependencies]` with the pinned `rune` and `rune-kit` and every approved crate. `rust-toolchain.toml`, `.cargo/config.toml`, `.gitignore`.
  - The copies listed in §1.1, with `COPIED_FROM.md` naming the source commit. Copied Rust is changed only where a path must change (the `include_str!` of `SKILL.md`).
  - `kudzu-app`, `kudzu-system`, `kudzu-appkit`, `kudzu-ui` with the module trees of §1 as documented empty modules.
  - `src/main.rs`: `rune::prewarm_gpu()`, one placeholder window, Rune tooling flags forwarded.
  - `cargo xtask check-deps` with every kind of failure in §1.1.
  - Launch-mode selection in `main.rs` through `rune::is_headless()` (§1.3); the headless branch builds no adapter.
  - `.github/workflows/desktop-v2.yml`: on `push` (any branch) and `pull_request`, filtered to `apps/desktop-v2/**` and the workflow file; `xcode-27`; fmt, clippy, tests, check-deps, a headless `--tree` smoke test; Rust cache keyed on `apps/desktop-v2`.
  - `docs/migration/spikes/S01-notes.md`: whether `--tree` and `--snap` run on the CI runner; clean build time; the result of a release-please dry run showing that a commit touching only `apps/desktop-v2/` is not counted, and that the `simple` strategy on a copy of this layout changes only `version.txt` and the changelog.
- **Accept:** `cargo test --workspace` runs the same 377 core and 73 engine tests as `apps/desktop/src-tauri` (compare counts). `git diff --stat` for the step shows nothing under `apps/desktop/`, the existing workflows, or the root workspace files. `check-deps` tests fail on a forbidden edge, on a path dependency to `../desktop/…`, on a `#[path]` that leaves the workspace and on a first-party `build.rs`; the real resolved graph passes. `kudzu --tree out.json` works in CI. Each of `--tree`, `--snap`, `--list-windows` and `--list-scenes`, run with `RUNE_DIRS_ROOT` set to a temp folder, reaches no native adapter constructor and writes nothing outside that folder. `pnpm install --frozen-lockfile` at the root still passes.
- **Closes:** EXP-1–5, TPL-1–4, TPL-7, TPL-8, TPL-11–16, RUL-1–5, CLP-2–4, SET-7, DAT-1, DAT-3–7, SYN-2, SYN-3 (core checks whose tests now run in the new workspace).
- **Depends:** none · **Parallel with:** S02
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `opencode-go/glm-5.3`
- **Commit:** `build(desktop-v2): start the Rune app as its own workspace`

#### S02 · Reference capture — L
- **Goal:** Record how the Tauri app looks and how its TypeScript logic behaves, as the yardstick for the new app.
- **Owns:** `docs/migration/reference/**` only. It runs and reads `apps/desktop` (rule 2's one exception) and changes nothing there.
- **Screenshots outside git (D12):** the PNGs live at `$KUDZU_REFERENCE_DIR` (default `~/.local/share/kudzu-migration/reference/`), not in the repo; see `reference/README.md`. Tools that compare pixels (S06 `xtask parity`, S73) read them from there and skip, with a clear message, when the folder is missing (CI).
- **Deliverables:** `reference/capture.mjs` (Playwright through `pnpm dlx`, against `pnpm dev` in `apps/desktop`): PNGs at 2× for every state in §1.7 step 1, light and dark, as `reference/<window>/<state>.<theme>.png`; `reference/metrics.json` (landmark rects, computed colours, font sizes); `reference/STATES.md` (each state and the scene name it maps to). `reference/fixtures.mjs` (Node's test runner with `apps/desktop/scripts/tests/typescript-loader.mjs`): JSON fixtures for search (the preview's library plus a seeded 5,000-snippet library: ordered ids, scores, highlight ranges), clipboard (query parsing, filters, sections, match ranges, labels), stats, group tree, template tokenizer (chip labels, tones, summaries, presets, canonical tags), key labels and recorder rules, release notes, colour conversions and contrast, editor formatting transactions. `reference/fixtures/mock-state.json`: the preview's library, usage, clipboard, apps, statuses, backups, update and sync states. `reference/BASELINE.md`: launch time, palette-open time, memory and idle CPU of the installed release build of the Tauri app on the owner's Mac, with the machine, macOS version and display.
- **Accept:** Every listed state has both themes at the window's real size. Fixtures regenerate byte-identically (fixed clock and seed). `git status` shows changes only under `apps/desktop-v2/docs/migration/reference/` (the orchestrator's D12 note in this file is the one exception), and no PNG is tracked.
- **Closes:** none (it is the yardstick for S06, S21–S24, S73).
- **Depends:** none · **Parallel with:** S01
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `docs(desktop-v2): capture reference screenshots and behaviour fixtures from the Tauri app`

#### S03 · Dev bundle, identity, logging, design assets — M
- **Goal:** `cargo xtask run` builds and launches a signed Dev bundle; the binary knows which variant it is and where to log; the design-asset check works in the new layout.
- **Owns:** `xtask/src/{bundle.rs,run.rs}`, `assets/{Info.plist.in,icons/**,tray/**}`, `src/{identity.rs,logging.rs,cli.rs,timing.rs}` (and their hook-up in `src/main.rs`), `.github/workflows/desktop-v2.yml` (one added step: the design check), `scripts/{check-design-assets.py,update-design-assets.sh,brand-assets.py,design-assets.swift}`, `scripts/tests/test_design_assets.py`, `design/assets-manifest.json`.
- **Deliverables:** `cargo xtask bundle --variant dev|prod [--universal]`, `cargo xtask run`, optional signing with `KUDZU_DEV_SIGN`. `Identity` per §1.6, with the Dev bundle named Kudzu Next (`sh.saeed.kudzu.next`). Log sinks per §1.6. CLI: `--mcp [--dev]`, `--hidden`, Rune flags passed through. The `KUDZU_TIMINGS` marks of §1.7. The design-asset pipeline pointed at the new outputs (`assets/icons/`, `assets/tray/`): the icon set is produced with `sips` and `iconutil` instead of the Tauri CLI, the manifest drops that source and lists the new outputs, and the CI workflow from S01 runs the check.
- **Accept:** Tests: identity table (production id, another id, no bundle → Dev) and Dev defaults; `Info.plist` has `LSUIElement`, the bundle id, the version from `version.txt` and minimum macOS 14; the log rotates at its size limit and keeps 3 files. `Kudzu Next.app` launches the placeholder window. `python3 scripts/check-design-assets.py` and its unit test pass.
- **Closes:** SHL-11, SHL-12 (log files).
- **Depends:** S01 · **Parallel with:** S04, S05, S11, S13, S14, S20
- **Implementer:** `gpt-6.1-terra` (high) · **Reviewer:** `opencode-go/deepseek-v4-pro`
- **Commit:** `build(desktop-v2): bundle and run the Dev app from xtask`

#### S04 · Theme tokens — M
- **Goal:** The Tauri app's `app/globals.css` as a hand-authored Rust theme.
- **Owns:** `crates/kudzu-ui/src/theme/{mod,palette,depth,radius,type_,motion,fonts}.rs`, `assets/fonts/**`, `crates/kudzu-ui/src/gallery/tokens.rs`.
- **Deliverables:** Everything in the §1.4 table except `icons.rs` and `kit.rs`. Paper Mono converted once from the WOFF2 that S01 copied (record the command in the commit body); Figtree's variable TTF added with its OFL text. A `gallery` window (headless and debug builds only) with a `tokens` scene: swatches, the 12 depth samples, the type ramp.
- **Accept:** Table tests: every colour token equals its CSS value in both themes; every depth token has the CSS layer count and numbers; the `Easing::Piecewise` arrays equal the two CSS `linear()` lists; the fitted springs stay within 0.02 of the samples. Goldens `gallery-tokens.{light,dark}.png` rendered with bundled fonts.
- **Closes:** none directly (FEATURES §3 tokens; verified again in S73).
- **Depends:** S01 · **Parallel with:** S03, S05, S11, S13, S14, S20
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `refactor(desktop-v2): add the Kudzu theme for Rune`

#### S05 · Icon set — S
- **Goal:** The Lucide icons the Tauri UI uses, as `rune::Icon` statics.
- **Owns:** `crates/kudzu-ui/src/theme/icons.rs`, `crates/kudzu-ui/src/gallery/icons.rs`.
- **Deliverables:** One static per icon imported from `lucide-react` anywhere under `apps/desktop/components/` (list them with `rg`; multi-line imports count). Bodies copied from Lucide with its ISC notice in the file header. An `icons` gallery scene.
- **Accept:** A test compares the static list with the import list recorded in the file header. Every icon rasterises non-blank at 16 pt in the gallery golden.
- **Closes:** none.
- **Depends:** S01 · **Parallel with:** S03, S04, S11, S13, S14, S20
- **Implementer:** `gpt-6-luna` (max) · **Reviewer:** `opencode-go/glm-5.3-flash`
- **Commit:** `refactor(desktop-v2): add the icon set`

#### S06 · Kit mapping, gallery, parity matrix — L
- **Goal:** rune-kit components wearing Kudzu's theme, with written evidence of where they match the Tauri UI and where they cannot.
- **Owns:** `crates/kudzu-ui/src/theme/kit.rs`, `crates/kudzu-ui/src/gallery/components/**`, `crates/kudzu-ui/src/widgets/kit_ext/**`, `xtask/src/parity.rs`, `docs/migration/PARITY_MATRIX.md`, `tests/golden/gallery-*`.
- **Deliverables:** `kit(theme)`. `cargo xtask parity`: the landmark extractor and comparison runner of §1.7, which every screen step then uses. One gallery scene per shadcn primitive the Tauri app renders, in the variants, sizes and states that appear in S02's captures; primitives it never renders are listed and skipped. `PARITY_MATRIX.md`: per primitive, the rune-kit component, measured height, radius, padding, type and shadow against `reference/metrics.json`, and a verdict (match, themed, decorated, wrapper). Exact metrics are required for buttons, inputs and switches; elsewhere the themed rune-kit component is accepted as it is (§6). For each wrapper: the upstream issue filed, and the wrapper itself.
- **From the S05 review:** add `gallery-icons.{light,dark}` goldens, rendering the icon scene with the theme foreground on the theme background (it draws white on black today). Also add a test that a group-inherited stroke (the `lucide!` wrapper `<g>`) covers the same pixels as an inline-attribute copy of one icon, for example `X`.
- **Accept:** The matrix covers every primitive in use. "Match" rows are within 2 pt of the reference. No wrapper exists without a matrix row and an issue link. Goldens in both themes.
- **Closes:** none directly (gap 17).
- **Depends:** S02, S04, S05 · **Parallel with:** S10, S12, S15, S21, S22, S23, S24, S25, S39
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `claude-opus-5-5` (medium)
- **Commit:** `refactor(desktop-v2): map the theme onto rune-kit and record component parity`

#### S07 · Shared widgets — M
- **Goal:** The Kudzu compositions rune-kit has no equivalent for.
- **Owns:** `crates/kudzu-ui/src/widgets/{shortcut,trigger_chip,selection_pill,breadcrumbs,slider,stepper,toggle_button,formatted_text,secret_text,empty_state,settings_card,key_hints,wordmark}.rs`, `crates/kudzu-ui/src/motion.rs`, gallery scenes for each.
- **Deliverables:** As named. `formatted_text` draws spans, links, inline images, the cursor marker, and key and pause badges from a `RenderResult`. `motion.rs` holds shake, land, the change census and presence helpers.
- **Added by D6 (owner, after S06):** exact-metric wrappers or decorations for the select trigger (32 pt), menu, dialog and toast radii (10 for menus, 14–18 for dialogs, matching the Tauri components), hover and pressed shadows on raised buttons plus the switch knob's 1.2× stretch while held, and the checkbox's checked primary colours. Tag each `RUNE-GAP(#573)` or `RUNE-GAP(#360)`, add each to `PARITY_MATRIX.md` and the gallery goldens, and run `cargo xtask parity` again.
- **Accept:** Tree tests: spoken shortcut labels; slider role, value and keyboard steps; breadcrumbs collapse past 4 to first plus last two; `trigger_chip` shows `␣` for spaces; `secret_text` exposes no content in the tree until revealed. Motion gate: selection pill slide and shake. Goldens.
- **Closes:** TPL-17 (UI part), CLP-17 (concealment widget).
- **Depends:** S06 · **Parallel with:** S26, S31, S34, S38
- **Implementer:** `grok-4.7` (xhigh) · **Reviewer:** `gpt-6.1-terra` (high)
- **Commit:** `refactor(desktop-v2): add Kudzu's shared widgets`

### Spikes

A spike answers questions with a throwaway binary under `spikes/<name>/` and a findings note `docs/migration/spikes/<id>-<name>.md` (question, answer, numbers, decision, upstream issue links). It ships no production code. It files or comments on the upstream issues in §3. Each note ends with a verdict: go, go with the fallback §3 names, or no-go. A no-go stops the steps that depend on the spike and goes to the owner.

#### S10 · Spike: menu-bar lifecycle — M
- **Goal:** Prove the app can live in the menu bar the way the Tauri app does.
- **Owns:** `spikes/menubar/**`, `docs/migration/spikes/S10-menubar.md`.
- **Questions:** Does an `NSStatusItem` created from an `AppHandle::update` work with a windowless accessory app, with a template image swapped at run time and a title? Does the §3 gap 2 approach give ⌘Q "Close Windows" with the Dock icon on, with it off, and from a launcher panel? Do Dock Quit and logout still quit and run `on_quit`? Does `on_quit` run when no window is open? Does `on_reopen` open the main window? Do Edit shortcuts work in fields with `standard_menus()`?
- **Accept:** Each question answered yes or no with evidence; the app-menu item order achieved; issues filed for gap 2 and a comment on #141.
- **Depends:** S03 · **Parallel with:** S06, S12, S15, S21, S22, S23, S24, S25, S39
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `test(desktop-v2): spike the menu-bar lifecycle on Rune`

#### S11 · Spike: floating panels — M
- **Goal:** Prove the palette, picker and form behave like the Tauri app's floating windows, and measure them.
- **Owns:** `spikes/panels/**`, `docs/migration/spikes/S11-panels.md`.
- **Questions:** Hotkey-to-visible latency of a prewarmed launcher panel over 20 runs (#335). Does it take keys without activating the app, and does the previous app stay frontmost? Hide on blur. Placement on the screen under the pointer at 20% and 18% of its height. A panel at an arbitrary caret rect with above/below flip and content-driven height. Popover material with radius 18 and the native shadow. Focus in the search field on show. Returning focus to the target app. Behaviour over a fullscreen app and across Spaces. First-mouse clicks (#388). Does `App::on_focus` report a panel gaining and losing key status (FRM-4 depends on it)? Does a state change made in the same turn as `show()` appear in the first visible frame, or do cached pixels flash?
- **Accept:** Numbers against the §1.7 budget; a decision on gap 9.
- **Depends:** S01 · **Parallel with:** S03, S04, S05, S13, S14, S20
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `test(desktop-v2): spike launcher panels on Rune`

#### S12 · Spike: main window chrome — S
- **Goal:** Decide the sidebar material and how the app forces its appearance.
- **Owns:** `docs/migration/spikes/S12/` (evidence), `spikes/chrome/**`, `docs/migration/spikes/S12-chrome.md`.
- **Questions:** A transparent full-size-content window with traffic lights at (18, 22) and drag regions. The sidebar with `SystemMaterial::Popover` and Kudzu's translucent fills, photographed beside the Tauri app in light and dark, active and inactive. Does setting `NSApp.appearance` make materials, traffic lights and menus follow the app theme, and what does `Cx::system_dark()` report then? Toolbar inset in full screen. `SplitView` collapse.
- **Accept:** Side-by-side images in the note; the images the owner decides from (§6 D3); issues filed for gaps 3 and 4.
- **Depends:** S03 · **Parallel with:** S06, S10, S15, S21, S22, S23, S24, S25, S39
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `opencode-go/deepseek-v4-pro`
- **Commit:** `test(desktop-v2): spike the main window chrome and sidebar material`

#### S13 · Spike: template editor on `TextEdit` — L
- **Goal:** Prove the snippet editor can be built on Rune's rich text field, and fix the editor document contract for S23.
- **Owns:** `spikes/editor/**`, `docs/migration/spikes/S13-editor.md`.
- **Questions:** Tags as `Mark::Pill` chips built from a parsed template. One click on a chip must open its configuration (gap 6): what a click on a pill selects, and how the popover anchors (`Cx::caret`). Spell checking in the field. A slash menu (`PopupList`) at a word start. Inserting a pill at the selection. Undo grouping across a pill insert and a mark change. Bold, italic, underline and links with `allow_marks`, and no block formats (`allow_blocks([])`), since Kudzu's lists are literal text. An image as a pill (gap 5), and whether a bitmap can be drawn over a reserved range of the field (for S76). IME composition next to pills. Mono for plain snippets, sans for formatted. Typing cost on a 2,000-character snippet (#535). A prototype codec that round-trips the `template_grammar` corpus.
- **Accept:** Every question answered; one-click chip configuration demonstrated, or a no-go reported; the `EditorDoc` shape proposed; comments filed on #128, #350, #353, #354, #535 as they apply.
- **Depends:** S01 · **Parallel with:** S03, S04, S05, S11, S14, S20
- **Implementer:** `claude-fable-5-1` (high) · **Reviewer:** `gpt-6-astra` (high)
- **Commit:** `test(desktop-v2): spike the template editor on Rune's rich text field`

#### S14 · Spike: virtual lists and in-app drag — M
- **Goal:** Prove the two hot lists meet their budgets, and settle the drag-and-drop workaround.
- **Owns:** `spikes/lists/**`, `docs/migration/spikes/S14-lists.md`.
- **Questions:** 5,000 rows with section headers, 64 pt rows, memoised rows keyed by id. Multi-select with Shift and ⌘. Keyboard moves that reveal unbuilt rows (#546, #549). Filtering by replacing the slice (#203). Dragging selected rows onto a sidebar drop zone: ghost with a count, hover highlight, rejection, and `on_drag` beside `on_press` (#551). A scroll trace against the budget.
- **Accept:** Trace numbers; the API for `dnd.rs`; a comment on #357.
- **Depends:** S01 · **Parallel with:** S03, S04, S05, S11, S13, S20
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `test(desktop-v2): spike virtual lists and in-app drag on Rune`

#### S15 · Spike: permission companion — M
- **Goal:** Prove the guided permission flow works without an Objective-C bridge.
- **Owns:** `spikes/permissions/**`, `docs/migration/spikes/S15-permissions.md`.
- **Questions:** A borderless always-on-top window that stays visible while System Settings is active. `drag_out` of the running `.app` into a privacy list; a cancelled drag; click to reveal. Finding the System Settings window frame from window metadata. Placing the companion beside it. The guide (outline plus ghost icon) as a click-through Rune overlay, or a native panel if the overlay cannot join that Space.
- **Accept:** Each answered on a signed Dev bundle; overlay or native decided.
- **Depends:** S03 · **Parallel with:** S06, S10, S12, S21, S22, S23, S24, S25, S39
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `test(desktop-v2): spike the permission companion on Rune`

### Domain: core

S20 reshapes the copied crate. S21–S24 add the logic that runs in TypeScript in the Tauri app; the fixtures from S02 are their specification. A step copies the fixtures it needs into `crates/kudzu-core/tests/fixtures/ts-parity/`.

#### S20 · Core: restructure into the domain layout — L
- **Goal:** The copied `kudzu-core` arranged as §1.2 describes, with nothing left of its IPC past, behaving exactly as before.
- **Owns:** `crates/kudzu-core/**`; in `crates/kudzu-engine/**`, only the `use` paths and type names the moves change, so the workspace builds at this commit.
- **Deliverables:** The module moves in the §1.2 table (`library/`, `interchange/{detect,merge,backup}`, `template/forms`, `matching` config, `formatting/`, `clipboard/{classify,history,retention,blob}` from the 1,023-line `clipboard.rs`, `secrets/`, `sync/`, `activity/`). `system.rs` with the shared vocabulary. Serde derives and renames that existed only for IPC removed. Files over the limit split, or their inline tests moved to `tests/`. `lib.rs` declaring the final tree, including empty modules for S21–S24. A short `crates/kudzu-core/README.md` with the module map. The hand-written Base64 codec in `interchange/rich/image.rs` replaced by the `base64` crate if the copied image tests pass unchanged; otherwise kept, with the failing case named. Two things stay where they are, marked for their steps: `agent/` (moves in S32) and the file orchestration of `interchange::coordinator` and `BackupStore` (moves in S29).
- **Accept:** Before moving anything, add byte-exact golden tests for the persisted formats (a fixture library as Kudzu JSON, a clipboard index, a sync payload, form memory); they pass before and after. All 377 core tests and the 73 engine tests pass with assertions untouched: only `use` paths change, and the commit body says so with the test-file diff stat. No file over 600 lines outside tests and tables. `cargo doc` has no broken links. `rg -i "tauri|ipc|webview" crates/kudzu-core` finds nothing.
- **Closes:** none (the checks S01 closed stay closed).
- **Depends:** S01 · **Parallel with:** S03, S04, S05, S11, S13, S14
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): restructure kudzu-core by domain`

#### S21 · Core: search, group tree, stats — M
- **Goal:** The ranking, tree and stats logic from `lib/search.ts`, `lib/groups.ts` and `lib/stats.ts`.
- **Owns:** `crates/kudzu-core/src/search.rs`, `crates/kudzu-core/src/library/{groups,usage}.rs` (new functions only: the tree, and aggregation across devices; the code S20 moved there is not reshaped), their tests and fixtures.
- **Deliverables:** `search::{index, rank, highlight}` with the exact scoring (exact trigger 10000, prefix 5000, weights 1 / .85 / .6, usage and recency boosts, disabled −15, limits). Group tree, flatten, recursive counts, paths, descendant sets (cycle-safe). Usage aggregation across devices, totals, time saved.
- **Accept:** Fixture tests: same ordered ids and highlight ranges for every search query, scores within 1e-6; group and stats fixtures equal. A release-mode timing test: 5,000 snippets ranked in ≤ 2 ms p95. Non-ASCII behaviour follows the fixtures (FEATURES §7 "Unicode differences"); note each place the TS behaviour was kept on purpose.
- **Closes:** QS-2 (ranking), INS-2 (aggregation).
- **Depends:** S02, S20 · **Parallel with:** S06, S10, S12, S15, S22, S23, S24, S25, S39
- **Implementer:** `gpt-6.1-terra` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `refactor(desktop-v2): add search ranking, the group tree and stats to core`

#### S22 · Core: clipboard query and presentation — M
- **Goal:** The filter, search, grouping and labelling logic from `lib/clipboard.ts`.
- **Owns:** `crates/kudzu-core/src/clipboard/{query,sections,facts}.rs`, their tests and fixtures.
- **Deliverables:** `query.rs` (`app:` and `is:` aliases, AND of contiguous terms, match ranges merged and capped at 64), `sections.rs` (pinned first, Today, Yesterday, weekday, date), `facts.rs` (characters, words, lines, sizes, expiry, labels, spoken names).
- **Accept:** The 19 copied clipboard tests still pass. Fixture tests for query, sections and labels. 5,000 clips searched in ≤ 5 ms p95.
- **Closes:** CLP-9 (logic), CLP-16 (facts).
- **Depends:** S02, S20 · **Parallel with:** S06, S10, S12, S15, S21, S23, S24, S25, S39
- **Implementer:** `opencode-go/deepseek-v4-pro` · **Reviewer:** `gpt-6.1-terra` (medium)
- **Commit:** `refactor(desktop-v2): add clipboard query and presentation logic to core`

#### S23 · Core: template editor document — L
- **Goal:** The pure mapping between a stored snippet (template source plus `formatting`) and what an editor edits (text, atomic chips, marks, images). It stands in for `lib/template.ts`, `lib/formatting.ts` and `lib/editor-formatting.ts`.
- **Owns:** `crates/kudzu-core/src/template/{doc,chips,presets,summary,lists,images}.rs`, tests and fixtures.
- **Deliverables:** `EditorDoc { text, pills, marks, images }` as agreed in S13. `EditorDoc::from_template(source, formatting)` and `to_template() -> (source, Option<Formatting>)`. UTF-8 byte ↔ UTF-16 offset mapping. Escapes. Unknown and verbatim tags kept byte-exact. Chip labels and tones for all 12 tags and for unknown tags. The 11 date presets. 140-unit plain summaries. List-marker toggling (`• `, `1. `) as text operations. Image rules: data URL ≤ 196,608 characters, ≤ 524,288 per snippet, width clamps, alt text sanitising.
- **From S21:** search takes the template summariser as a function. Once `plain_summary` exists (at 2,000 UTF-16 units, like `plainSummary(content, undefined, 2000)`), switch `tests/ts_parity_search.rs` from the injected `plain-summaries.json` to the real function and keep it green.
- **From S21 review:** add a long-summary search benchmark (summaries near 2,000 UTF-16 units on the seeded 5,000-snippet library) with the real summariser, and keep p95 within the §1.7 budget.
- **Accept:** Property tests on the existing grammar generators: `to_template(from_template(s))` parses to the same nodes and renders the same text; canonical sources round-trip byte-identically; spans round-trip, including styles over chips and beside images. Fixture tests for labels, summaries, presets and list toggles.
- **Closes:** EDT-5 (model), EDT-13 and EDT-14 (span and list logic), EDT-15 (budgets).
- **Depends:** S02, S13, S20 · **Parallel with:** S06, S10, S12, S15, S21, S22, S24, S25, S39
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `refactor(desktop-v2): add the template editor document to core`

#### S24 · Core: colour, key labels, release notes, formatting helpers — M
- **Goal:** The remaining pure helpers from `clip-content.tsx`, `lib/keys.ts`, `lib/updates.ts` and the label code in `lib/stats.ts`.
- **Owns:** `crates/kudzu-core/src/{color,keys,release_notes,fmt}.rs`, tests and fixtures.
- **Deliverables:** `color`: parse any CSS colour, HEX/RGB/HSL/OKLCH strings with the current rounding, contrast against white and black. `keys`: accelerator grammar, symbol and spoken labels, recorder acceptance (modifier plus key, or a bare F-key; lone modifiers and Shift-only refused). `release_notes`: headings, bullets, link, hash and bold clean-up. `fmt`: counts (k, M), durations, relative dates, byte sizes, clock (time today, date before).
- **Accept:** Fixture tests for each.
- **Closes:** CLP-15 (colour math), SET-3 (recorder rules), SHL-10 (notes parsing), INS-4 (number and date formats).
- **Depends:** S02, S20 · **Parallel with:** S06, S10, S12, S15, S21, S22, S23, S25, S39
- **Implementer:** `opencode-go/glm-5.3` · **Reviewer:** `gpt-6-luna` (max)
- **Commit:** `refactor(desktop-v2): add colour, key label and release-note helpers to core`

### Domain: application layer

Every service is tested against the fakes in `kudzu_app::fixtures`. The behaviour to match is the Tauri shell's: `apps/desktop/src-tauri/src/{commands,store,state,host}.rs` and the module each step names. Read them to learn what happens, then write the service for §1.2. Do not translate the commands.

#### S25 · Application foundation — L
- **Goal:** The primitives every service uses, the port traits, and the fakes.
- **Owns:** `crates/kudzu-app/src/{watch,tasks,schedule,persist,paths,error}.rs`, `crates/kudzu-app/src/ports/**`, `crates/kudzu-app/src/fixtures/**` (one file per port in each).
- **Deliverables:** As §1.2. The `Ports` struct (one `Arc` per port). `fixtures`: a fake for every port; a fixed clock, time zone, locale and UUID source; sample data loaded from a copy of `reference/fixtures/mock-state.json`. `Kudzu` is not built here: it needs every service, so it comes with S40.
- **Accept:** Tests: `Watch` (revision changes only with the snapshot; wakers run after unlock, proven by a test that would deadlock otherwise; a change between `subscribe` and the first `get` is never missed; drop unsubscribes); `schedule` on a fake clock; `JsonFile` and its writer (atomic write, unique temp name, corrupt file moved aside and never overwritten when that fails, `saved()` through a delayed write, a failed write and its retry, a synchronous drain); a `Task` whose owner is dropped. No log line or error cause contains file contents. `check-deps` passes for `kudzu-app`.
- **Closes:** none.
- **Depends:** S20 · **Parallel with:** S06, S10, S12, S15, S21, S22, S23, S24, S39
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `refactor(desktop-v2): add the application layer's foundation and ports`

#### S26 · Settings and library services — L
- **Goal:** The two services everything else reads.
- **Owns:** `crates/kudzu-app/src/{settings,library}/**`.
- **Deliverables:** `settings`: every field and default of the Tauri app's `settings.rs`, Dev defaults, missing keys take the variant's defaults. `library`: the interface in §1.2, every change a transaction, `transact` for callers' closures, `saved()`; conflicts recomputed off the caller's thread, debounced 250 ms.
- **Accept:** Tests for each behaviour: create (new id and timestamps, inserted first), update (keeps `createdAt`, bumps `updatedAt`, drops blank triggers), duplicate (" copy", each trigger gets the first free numeric suffix from 2, inserted after the source), delete and restore (restores only missing ids), move snippets and groups (sort index; "A group can't be moved inside itself."), delete group keeping or deleting snippets (recursive), usage per snippet and device, first-run Getting Started seed only when the file is absent, corrupt library moved aside. Transactions: two threads creating the same trigger through `transact` give one success; a closure that fails after creating groups leaves no group behind; a closure sees every edit committed before it started. The two settings tests from `store.rs` ported.
- **Closes:** LIB-6, LIB-7 (domain part), SET-12, DAT-13 (seed and quarantine; the legacy-directory copy is left out on purpose), INS-1, RUL-7 (cycle rule).
- **Depends:** S25 · **Parallel with:** S07, S31, S34, S38
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the settings and library services`

#### S27 · Expansion, forms and activity services — L
- **Goal:** Everything the app does with the engine, behind sessions and guards.
- **Owns:** `crates/kudzu-app/src/{expansion,forms,activity}/**`.
- **Deliverables:** As §1.2: engine feed, pause, `SuppressionGuard`, `QuickSearchSession`, explicit insert, copy (plain plus HTML), preview through core's renderer with clipboard, history, selection and form values; the pending `FormRequest` with token-based `submit` and `cancel`, `panel_focused`, and form memory; the activity ring and remedies; health wording for the sidebar and tray.
- **From S26:** react to library snapshots and skip index rebuilds when only usage changed. Prune form memory after deletes. Route a successful insertion to `library.record_expansion`. Forms go through the `FormChannel` port (split from `ExpansionEngine`). Publish with `Committed<T>`.
- **Accept:** With a fake engine: the index reloads only when matching data changed, not on usage-only changes; pause persists and reaches the engine; guards count; a quick-search session captures the selection only when a snippet uses `{{selection}}`, and before the caller is told to show; initial form values are remembered → default → first choice → empty; form memory caps at 200 with smallest-UUID eviction and prunes on delete; submit records memory, then submits, and a failed memory write does not block the insert; a stale or repeated token does nothing; cancel reactivates the target; `forms` releases its guard on submit, cancel, replacement and `close_windows`; `panel_focused` reaches the engine port; the ring holds 100 newest-first with monotonic ids and is never written anywhere; every outcome has its remedy, and a remedy reports "fixed since" when state changed; explicit insertion keeps the current eligibility rule (RUL-9).
- **Closes:** TPL-6, QS-8, FRM-1, FRM-3, FRM-5 (application part), ACT-1, ACT-2, ACT-4 (logic), RUL-9, EXP-20 (usage after insertion).
- **Depends:** S26 · **Parallel with:** S28, S29, S30, S33, S36
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `claude-sonnet-5-5` (high)
- **Commit:** `refactor(desktop-v2): add the expansion, forms and activity services`

#### S28 · Clipboard service — L
- **Goal:** Clipboard history as one service: capture, store, retention, undo, secrets, copy and paste.
- **Owns:** `crates/kudzu-app/src/clipboard/**`.
- **Deliverables:** As §1.2; the behaviour of `apps/desktop/src-tauri/src/clipboard/{mod,store,protocol}.rs`. Media bytes are served by `media(id, kind)` with the protocol's checks.
- **From S22:** `ClipSearch` borrows an immutable snapshot and caches haystacks; rebuild it when the snapshot changes. Agent results still go through secret redaction.
- **From S26:** react to clipboard settings. Target capture and activation go through the `TargetApp` port (split from `AppDirectory`).
- **Accept:** With a fake pasteboard: launch contents, own writes, recording off, ignored apps and private markers are skipped; recopy folds; excerpt ≤ 2,048 scalars; blobs over 64 MiB are metadata-only and copying one fails with the explicit message; thumbnails ≤ 480 px through the port; the index is written at most every 900 ms and retried after an I/O failure; a corrupt index is moved aside; secrets are rescanned when the rules change; trash and orphans are collected at launch; undo restores from trash; clear with and without pinned; prune at start, every 10 minutes and on a retention change; the folder is 0700; `{{clipboard history=N}}` alignment (skip the newest only when it equals the current clipboard); copy back keeps flavours or forces plain; paste re-copies first and returns `Copied` when posting is blocked; the target-gone message; media limits (80 MiB image file, 64 px icon). Also: a private marker on any pasteboard item skips the copy; pausing expansion does not pause recording; a single image file over 40 MiB gets no thumbnail; a secret override survives a recopy and a relaunch; the paste path never restores the older clipboard and keeps the 800 ms activation wait and 40 ms settle. The two clipboard-store tests ported.
- **Closes:** CLP-1, CLP-5, CLP-6, CLP-7, CLP-11 and CLP-12 (logic), CLP-18 (persistence), PKR-5 (application part), TPL-5.
- **Depends:** S22, S26 · **Parallel with:** S27, S29, S30, S33, S36
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `refactor(desktop-v2): add the clipboard history service`

#### S29 · Data service: import, export, backups — M
- **Goal:** One place that orchestrates core's interchange code with the library, and owns the files.
- **Owns:** `crates/kudzu-app/src/data/**`; in `crates/kudzu-core/src/interchange/`, the removal of `coordinator.rs`'s file orchestration and of `BackupStore`'s I/O (their pure parts stay in `merge`, `detect` and `backup`). Until this step nothing outside core calls either, so no caller breaks.
- **From S26:** finish required backups before any mutation. Import and restore run against the current state inside `library.transact`.
- **Accept:** Imports parse before they mutate, and merge inside `library.transact`; the pre-import backup must succeed first; the summary carries counts, warnings and samples; Espanso detection reports only folders with snippets; JSON export is named `kudzu-YYYY-MM-DD.json`; Espanso export returns the file count; the backup list is newest first; backups prune to 14 where the Tauri app prunes (daily and manual); restore takes a pre-restore backup first; the daily backup runs at startup and every 6 hours on `schedule`; the "Before update" backup is taken once per new version (the `store.rs` test ported). Core tests that exercised orchestration move to `crates/kudzu-app/tests/` with their assertions unchanged; the total test count does not drop.
- **Closes:** DAT-2, DAT-8, DAT-10, DAT-11.
- **Depends:** S26 · **Parallel with:** S27, S28, S30, S33, S36
- **Implementer:** `gpt-6.1-terra` (high) · **Reviewer:** `opencode-go/deepseek-v4-pro`
- **Commit:** `refactor(desktop-v2): add the import, export and backup service`

#### S30 · Apps, permissions, onboarding and hotkey state machines — M
- **Goal:** Four pieces of flow logic that live in React components in the Tauri app, as tested state machines (`rune::machine` style: an enum state, a transition that returns effects; no Rune dependency).
- **Owns:** `crates/kudzu-app/src/{apps,permissions,onboarding,hotkeys}/**`.
- **From S26:** persist hotkeys only after a registration succeeds. Use `AppCatalog` (installed apps and icons) separately from `TargetApp`.
- **Accept:** `apps`: one enumeration, cached; icons cached; a missing app falls back to its bundle id. `permissions`: states checking, opening, waiting, allowed, connecting, revoked, error; ready means Accessibility and posting access and an attached tap; a drop or a Settings launch never counts as a grant; restart is offered after 5 s of grant without a listener; the restart marker is written and consumed once; the macOS 27 name "Device Control and Data Access". `onboarding`: Welcome → Access → Import → Try it with back and skip; detection states; a retry skips folders already imported and keeps partial counts; net-new counts; Continue only after net additions; a failed finish stays recoverable. `hotkeys`: normalised chords; the two slots cannot collide; a failed registration restores the old chord and does not persist; the clipboard slot is unregistered while recording is off.
- **Closes:** ONB-2, ONB-3 and ONB-4 (logic), ONB-9, ONB-10 (marker), SHL-6 (logic), RUL-8 (catalogue).
- **Depends:** S26 · **Parallel with:** S27, S28, S29, S33, S36
- **Implementer:** `grok-4.7` (xhigh) · **Reviewer:** `gpt-6.1-terra` (high)
- **Commit:** `refactor(desktop-v2): add the apps, permissions, onboarding and hotkey state machines`

#### S31 · Updates service — M
- **Goal:** The updater's behaviour, without a framework plugin.
- **Owns:** `crates/kudzu-app/src/updates/**`.
- **Accept:** With fake `Fetch`, `UpdateInstaller` and `Clock`: the seven phases; one check or download at a time; the schedule (20 s, then 6 h) runs only when enabled, in a release build, for the production identity (the Dev-gate test ported); stable and beta feeds; never a downgrade; bytes are verified before `Ready`, and a bad signature drops them; a quiet check error keeps the previous state, a quiet download error returns to `Idle`, a user-initiated error is reported; `needs_move` from the installer's probe; install on quit; restart 400 ms after install. Downloads have a time limit and a size cap. A committed fixture signed outside this code (with the `minisign` CLI) verifies, so signer and verifier are not only tested against each other.
- **Closes:** SHL-8, SHL-9 (application part).
- **Depends:** S25 · **Parallel with:** S07, S26, S34, S38
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `refactor(desktop-v2): add the update service`

#### S32 · Agents service (MCP) — M
- **Goal:** The MCP server as an inbound adapter of the application layer.
- **Owns:** `crates/kudzu-app/src/agents/**`; the removal of `crates/kudzu-core/src/agent/**` (moved here, `tools.rs` split by tool group) with its lines in core's `lib.rs` and `README.md`.
- **From S26:** collision checks, group creation and agent edits share one `library.transact`. Redact agent output.
- **Accept:** The 16 MCP tests pass against in-memory services; the two tests from the Tauri app's `agent.rs` are ported. Socket: 0600, refuses a live duplicate, removes a stale socket, one thread per client, stops answering when access is turned off, removed at shutdown. Bridge: newline framing, no reply to notifications, offline tool listing with the actionable message, one reconnect, a 30 s timeout without replay. Write and clipboard tools are gated independently; clipboard also needs recording on. Create and update run inside `library.transact`, so the collision check, group creation and the write are one unit (MCP-5), and an agent edit racing a UI edit either lands on top of it or fails cleanly. Recorded client transcripts (initialize, tools/list, tools/call) replay. Setup: the quoted executable path, `claude mcp add --scope user`, the name by identity, the JSON config. Skill install states: missing, current, outdated. The resource `kudzu://skill/SKILL.md` is listed and readable.
- **Closes:** MCP-1 through MCP-9.
- **Depends:** S26, S27, S28 · **Parallel with:** S35, S37, S70
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `opencode-go/glm-5.3`
- **Commit:** `refactor(desktop-v2): move the MCP server into the application layer`

#### S33 · Sync service — M
- **Goal:** Sync lifecycle and status over a port, so the CloudKit layer only translates.
- **Owns:** `crates/kudzu-app/src/sync/**`.
- **From S26:** merge inside `library.transact`, then commit the sync index in the same serialised sync job.
- **Accept:** With a fake `CloudStore`: all eight statuses; Dev and unentitled builds report Unavailable with the current wording (the gate test ported); edits queue without usage; usage is batched (nominal 10 minutes on the 3-minute tick: keep the 12-minute crossing); a remote merge runs inside `library.transact` and commits the sync index in the same job, so nothing is echoed and a local edit made during the merge survives; each start has a generation, and a callback or timer from an earlier one (after stop, restart or an account change) is ignored; a restart after a failed checkpoint resumes without loss; no upload of starter data before the first full fetch; the one-time initial dedupe; NeedsUpdate stops sends and keeps local data; an account change resets server knowledge and keeps the library; Sync now fetches and sends.
- **Closes:** SYN-1, SYN-4 and SYN-6 (application parts), SYN-5.
- **Depends:** S26 · **Parallel with:** S27, S28, S29, S30, S36
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `refactor(desktop-v2): add the sync service`

### Platform

#### S39 · Engine: seams and protocol tests — L
- **Goal:** Pin the engine's safety-critical protocols with tests before S34 moves any of its code. (Numbered out of order: it runs before S34.)
- **Owns:** `crates/kudzu-engine/**`.
- **Deliverables:** Seams in the existing insertion coordinator around three things and nothing else: posting events; the pasteboard (one owner type for the pending restore and the own-change record); observations (clock, frontmost app, form-panel focus). Fakes for them in the crate's tests. The `form_panel_focused` input and the interruption rule of §1.2.
- **Accept:** Tests on the otherwise unchanged coordinator, each asserting the full sequence of posted events and the state left behind. Paste and restore: a newer copy is not overwritten; an unsafe snapshot refuses the paste; consecutive pastes finish the earlier restore first; failure and shutdown restore (EXP-10). Held input: FIFO replay including key-up; input typed during replay; a second trigger completed by replay; release on cancellation, tap loss, shutdown and deadline (EXP-14). Sequences: stop on click, app change, pause, Secure Input, lost posting access and a stale generation, leaving inserted text alone (EXP-15). Images: an unconfirmed delivery skips the cursor move and the Backspace undo (EXP-13). Listener: active and listen-only attach, and an upgrade without a second listener (EXP-18). Forms: a typed form whose panel lost focus and then saw a key-down or click is not submitted; a stale token does nothing; a cancelled form leaves the trigger intact (FRM-4). A usage-only reload keeps an armed undo and a partial trigger (EXP-19). The trigger-latency test of §1.7 runs on the poster seam. The 73 copied tests still pass.
- **Closes:** FRM-4; EXP-10, EXP-13, EXP-14, EXP-15, EXP-18 at protocol level (live confirmation in S71).
- **Depends:** S20 · **Parallel with:** S06, S10, S12, S15, S21, S22, S23, S24, S25
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `test(desktop-v2): pin the engine's insertion protocols behind seams`

#### S34 · Engine: restructure — L
- **Goal:** The copied `kudzu-engine` as §1.5 describes: a small documented surface, no 1,200-line file, macOS-only, behaving exactly as before.
- **Owns:** `crates/kudzu-engine/**`.
- **Deliverables:** Every change listed in §1.5. The engine uses `kudzu_core::system`. App enumeration, icons, thumbnails, Settings links and locale leave the public API; until S35 moves their code, they stay `pub(crate)` or behind a re-export marked for S35 to delete.
- **Accept:** All 73 copied tests and S39's protocol tests pass; none deleted or weakened (list the test names before and after in the commit body). No file over 600 lines. `lib.rs` documents the public API and its threading contract. No serde derive remains that existed only for IPC. `rg -i "tauri" crates/kudzu-engine` finds nothing.
- **Closes:** none (EXP-6–19 keep their tests; live confirmation in S71).
- **Depends:** S39 · **Parallel with:** S07, S26, S31, S38
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): restructure the expansion engine`

#### S35 · System adapters: engine, pasteboard, apps — M
- **Goal:** The ports that touch the expansion path and app metadata, on macOS.
- **Owns:** `crates/kudzu-system/src/{engine,pasteboard,apps,thumbnails,settings_panes,locale}.rs`; in `crates/kudzu-engine`, the removal of the code that moves here.
- **Accept:** Each adapter passes the same contract test suite as its fake (the suite is generic over the port; the real adapter runs on macOS for cases that need no permission). The engine tests that moved with the code (`apps`, thumbnails) pass here. The commit body lists what S71 must check by hand.
- **Closes:** none alone (enables EXP, CLP and PKR live checks).
- **Depends:** S27, S28, S30, S34 · **Parallel with:** S32, S37, S70
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the engine, pasteboard and app adapters`

#### S36 · System adapters: login item, files, instance, update install — M
- **Goal:** The remaining ports: start at login, open and reveal files, keep one running instance, relaunch, and install an update.
- **Owns:** `crates/kudzu-system/src/{login_item,files,instance,relaunch,update_install}.rs`.
- **From S31:** the `ureq` `Fetch` adapter implements `get(&FetchRequest, &dyn Fn(u64, Option<u64>))`. It honours `timeout` and `max_bytes` while reading, reports bytes so far with `Some(total)` from Content-Length (or `None`), and watches `request.cancel`, returning `PortError::Cancelled`. Feed requests use 30 s / 64 KiB and archive requests use 10 min / 256 MiB.
- **From the S31 review (installer contract):**
  - The extracted `CFBundleShortVersionString` equals the **signed** version from `UpdatePackage`, and it must be strictly greater than the **running** bundle's version.
  - `CFBundleIdentifier` equals the expected bundle id.
  - `codesign --verify --deep --strict` passes against a designated requirement pinned to team `V3HN8HXZYK` and the bundle id.
  - The staged archive's BLAKE3 digest (`UpdatePackage.digest`) matches.
  - The archive contains no symlinks or hardlinks escaping the bundle, no device files and no setuid/setgid bits.
  - Total extracted size and entry count are capped.
  - Extraction happens on the same volume, and the swap is a `rename`.
  - The `ureq` adapter keeps redirects on `https`, and is tested with a lying Content-Length and an endless body.
- **Accept:** Login item: a LaunchAgent with `--hidden`, labelled by identity, written and removed idempotently (tested with a temporary `HOME`). Instance: an OS-held `flock`, released if the app crashes, not a sentinel file; a second bundled launch asks Launch Services to reopen the bundle and exits; a second unbundled launch just exits. Update install: the archive and bundle checks of §1.6 reject a traversal entry, a second top-level item, a wrong bundle id and a lower version; a good fixture extracts beside the bundle, the swap keeps a backup until it succeeds, a translocated path or a read-only volume is refused with `needs_move`, the writability probe leaves nothing behind. Relaunch works from a bundle.
- **Closes:** SHL-5 (login item and single instance), SHL-9 (install part).
- **Depends:** S25, S31 · **Parallel with:** S27, S28, S29, S30, S33
- **Implementer:** `gpt-6.1-terra` (high) · **Reviewer:** `opencode-go/deepseek-v4-pro`
- **Commit:** `refactor(desktop-v2): add the login item, file, instance and update-install adapters`

#### S37 · CloudKit adapter — L
- **Goal:** `CKSyncEngine` behind the `CloudStore` port, as an object. The Tauri app's `cloud/apple.rs` is the behaviour reference, not the structure.
- **Owns:** `crates/kudzu-system/src/cloudkit/**`, `assets/Kudzu.icloud.entitlements`, `cloudkit/kudzu-types.ckdb`.
- **Accept:** No `static` state in the module. Every callback carries the service's generation, and a stale one is dropped. Container, zone and record type names come from `kudzu_core::sync`. The entitlement check gates start. Delegate events map to the port: fetched changes, sent changes, `serverRecordChanged` merge and requeue, account change, zone deletion and re-creation. Engine state is written atomically to `sync/engine-state.data`. Timers are the application layer's (`schedule`), not threads here. Builds for both architectures. The commit body gives the manual test for a signed build (run in S71). No record or schema change.
- **Closes:** SYN-1, SYN-4, SYN-6 (adapter parts).
- **Depends:** S26, S33 · **Parallel with:** S32, S35, S70
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `refactor(desktop-v2): add the CloudKit layer behind the sync port`

#### S38 · AppKit glue for Rune gaps — M
- **Goal:** The production versions of what S10, S12 and S15 proved, one module per gap.
- **Owns:** `crates/kudzu-appkit/src/**`, `spikes/appkit-demo/**`.
- **Deliverables:** `status_item`, `app_menu`, `appearance`, `screens`, `settings_window`; `sidebar_material` and `panel_tuning` only if their spikes decided so. Each: a safe API of plain types and closures, a main-thread check, a header naming the upstream issue and what replaces the module. A window pointer is taken from Rune's `WindowHandle::with_raw_handle` inside the call and never stored.
- **Read first:** the "Review findings" in `docs/migration/spikes/S10-menubar.md` (four required changes and the status-item API).
- **Read first:** the "Review findings" in `docs/migration/spikes/S12-chrome.md` (sidebar-material lifecycle, appearance contract, evidence gaps).
- **Accept:** Tests that need no window server: the menu model, the glyph with the update dot (pixel test), shortcut hint text. A demo binary that exercises each module. `check-deps` shows no `kudzu-*` or `rune` dependency.
- **Closes:** none alone (SHL-1, SHL-2, SHL-4 with S40).
- **Depends:** S03, S10, S12, S15 · **Parallel with:** S07, S26, S31, S34
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add AppKit glue for the Rune gaps`

### UI shell

Screen and window steps share these acceptance rules, so they are not repeated: scenes exist under the names in `reference/STATES.md`; landmark rects are within 2 pt of `reference/metrics.json` in both themes; the screen's `model.rs` has unit tests for every transition listed; tree tests assert roles, labels and focus order; dialogs and popovers trap focus, close on Escape and on an outside click, and return focus to where it was; `scripts/app-audit --strict` (from the pinned Rune checkout) reports nothing for the files owned. The matching Tauri components (named in `FEATURES.md` §3 and §4) are the reference for behaviour; the module structure is this plan's, not theirs.

#### S40 · Shell and composition — L
- **Goal:** The app runs end to end with placeholder content: every service and adapter composed, five windows, menu bar item, hotkeys, lifecycle, theme. This is the first step where all of them exist.
- **Owns:** `crates/kudzu-app/src/assembly.rs`, `crates/kudzu-system/src/lib.rs` (`Ports::macos`), `src/{main.rs,compose.rs}`, `crates/kudzu-ui/src/shell/**` (`windows`, `lifecycle`, `tray`, `hotkeys`, `routes`, `theme_sync`), `crates/kudzu-ui/src/{actions/mod.rs,focus.rs}`, a placeholder view per window.
- **Deliverables:** `Kudzu::new` and `Kudzu::shutdown` (§1.2). `Ports::macos`. The window table in §1.3. `compose.rs`: launch mode → identity → paths → `Ports::macos` or fixtures → `Kudzu::new` → windows → wakers. `Shell`, which hands each window only the services it uses. The tray menu model mapped onto `kudzu_appkit::status_item`. `HotkeyRegistrar` over `AppHandle::register_shortcut`. Theme preference applied to every window and to `NSApp.appearance`.
- **Read first:** the "Review caveats" in `docs/migration/spikes/S11-panels.md` (what this step must re-measure or prove).
- **Read first:** the "Review findings" in `docs/migration/spikes/S10-menubar.md` (four required changes and the status-item API).
- **From S26:** assembly owns the `EngineRouter` (`EngineEvents`), the shared runtime objects, subscriptions and timers. Hold the single-instance guard before opening stores. Stop producers, including untracked `Task`s, before draining writers, and treat `Timeout` from `Writer::shutdown` as incomplete durability.
- **Accept:** On fakes, without Rune: every row of the reactions table, end to end; the shutdown order of §1.2 with a slow writer, run once however quit starts. Headless: `--list-windows` prints `main`, `palette`, `picker`, `form`, `permissions`; a scripted shortcut opens the palette and Escape hides it; `{"reopen": true}` opens `main`; scripts that cross windows run with `--desktop`. With fixtures on a real temp folder, an edited draft is on disk after an immediate relaunch following each of: the tray's Quit, ⌘Q, the close button, a scripted system quit. After every way of dismissing the palette, the picker or a form, nothing is suppressed. Turning recording off hides an open picker. Tray model tests: item order and labels (with Dev names), the status line, glyph priority pause → Secure Input → permission, update label states, Clipboard History disabled while recording is off. New Snippet from Selection captures, then routes to a new snippet even when the selection is empty. Closing `main` hides it and drops the Dock icon. ⌘Q closes every window, cancels a pending form and lifts suppression. `shutdown()` runs once however quit starts. `--hidden` is ignored on first run. A theme change reaches every window in one turn. On a Dev bundle: the status item appears and works; a hotkey conflict rolls back.
- **Closes:** SHL-1, SHL-2, SHL-3, SHL-4, SHL-5, SHL-6, SHL-7, WIN-1, QS-1 and PKR-1 (shell parts).
- **Depends:** S04, S11, S26–S33, S35–S38 · **Parallel with:** none
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `refactor(desktop-v2): add the Rune shell and composition root`

#### S41 · Main window frame — M
- **Goal:** The main window's skeleton that every screen plugs into.
- **Owns:** `crates/kudzu-ui/src/main/{mod,state,frame,toolbar,toasts,keys,scenes}.rs`, `crates/kudzu-ui/src/actions/main.rs`, and a stub `mod.rs` for each screen module (`library`, `editor`, `clipboard`, `settings`, `insights`, `onboarding`, `sidebar`) that later steps take over.
- **Deliverables:** `SplitView` frame: 42 pt toolbar, 224 pt sidebar slot, content card with radius 16, 8 pt gutters. Toolbar: sidebar toggle, breadcrumbs, Search ⌘K. Routing between Library, Insights, Clipboard, Settings and Onboarding. The toast host with Undo (6 s). The main shortcut table through `actions`. Sidebar hiding that keeps the subtree mounted and inert. `MainState` with one field per screen, each typed by that screen's module.
- **From S26 (all screens):** pull snapshots rather than mirror domain state. Show "Saved" only when `saved().durable` has reached the accepted revision with no error. Keep failed drafts and dialogs open, and keep I/O off handlers.
- **Accept:** Key scripts cover every shortcut in LIB-14, among them ⌘1–5, ⌘, , ⌘/ and `?`, and ⌘⇧P through the service, with the text-field and modal guards. Undo restores the snippets and selects the first restored one; deleting the open snippet returns focus to the list. Shortcuts do nothing during IME composition and respect text fields as in the Tauri app. Breadcrumbs collapse past 4.
- **Closes:** LIB-14, LIB-15, LIB-7 (Undo toast).
- **Depends:** S07, S21, S40 · **Parallel with:** S56, S57, S66
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the main window frame`

#### S42 · Sidebar — L
- **Goal:** The sidebar: both sections, the group tree with rename and context menu, drag targets, and the status footer.
- **Owns:** `crates/kudzu-ui/src/main/sidebar/**`, `crates/kudzu-ui/src/dnd.rs`.
- **Accept:** Sections: Snippets (All, Favorites, Recent, Off when any, Groups with +), Clipboard (History, Pinned, populated kinds; an active empty folder stays), fixed Insights and Settings, the status footer with pause. Recursive counts. Collapse arrows, Left and Right, Home and End, arrows across visible rows. Section folds persist in `ui-state.json`. Rename by double-click, F2 or ⌥↩; commit trimmed and non-empty; Escape cancels. A new subgroup expands its parent and starts a rename. The context menu's seven items and both delete variants. Dragging snippets or a group onto a group, refusing self and descendants; the root target unparents; the ghost shows a name or count; hover does not flicker. Folded content is inert and out of the accessibility tree. Motion gate on the selection pill.
- **Closes:** LIB-8, LIB-9, LIB-10, LIB-11, CLP-8 (sidebar part).
- **Depends:** S14, S26, S41 · **Parallel with:** S43, S50, S55, S58, S64, S65
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `refactor(desktop-v2): add the sidebar`

#### S43 · Command menu and cheat sheet — M
- **Goal:** The ⌘K command menu and the shortcut cheat sheet, both fed by the action registry.
- **Owns:** `crates/kudzu-ui/src/main/{command_menu,cheatsheet}.rs`.
- **Accept:** ⌘K opens a 600-wide menu at 18% from the top: frequent and matching snippets (5 and 8) plus actions from the registry; every query term must match a label or keyword; selection loops; Escape closes and resets. The cheat sheet (720) lists every action with its shortcut; a test fails if an action with a shortcut is missing from it.
- **Closes:** LIB-13, WIN-2 (these dialogs).
- **Depends:** S21, S41 · **Parallel with:** S42, S50, S55, S58, S64, S65
- **Implementer:** `grok-4.7` (xhigh) · **Reviewer:** `gpt-6.1-terra` (high)
- **Commit:** `refactor(desktop-v2): add the command menu and shortcut cheat sheet`

### Screens and windows

#### S50 · Library list — L
- **Goal:** The snippet list: scopes, sorting, filtering, selection and bulk actions, smooth at 5,000 rows.
- **Owns:** `crates/kudzu-ui/src/main/library/**`.
- **From S21:** `search::index(snippets, &usage_by_id(..), &summarize)`. `rank(&index, query, limit, now)` returns hits with `position`, an index into the slice passed in; use it instead of looking up by id. `DEFAULT_LIMIT` is 200. `highlight(name, &hit.name_hits)` returns merged byte ranges; hits are UTF-16. `GroupNode<'a>` borrows; `flatten` returns `Vec<&GroupNode>`.
- **Accept:** Model: scopes (All, Favorites, Recent, Off, a group with its descendants); sorts (Best match, Name with unnamed last, Recently edited, Most used, and the empty-query rules); the 10,000 cap; neighbour selection after a delete; anchored Shift selection, ⌘-click, ⌘A, Escape; `/` and typing focus the filter; filter arrows, Return, Escape. View: 64 pt rows, padding 4 and 8 (how many rows are mounted off screen is Rune's choice); name, first trigger plus count, summary, usage, favorite and off marks; no entrance replay on remount. The bulk bar (favorite all, toggle each, move, delete all), inert while it exits. Every empty state with its copy and actions. A drag source for S42. A scroll trace with 5,000 rows inside the budget.
- **Closes:** LIB-1, LIB-2, LIB-3, LIB-4, LIB-5, LIB-12.
- **Depends:** S14, S21, S26, S41 · **Parallel with:** S42, S43, S55, S58, S64, S65
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the library list`

#### S51 · Editor shell — L
- **Goal:** Everything in the editor except the content field's chips and formatting (a plain text area bound to the template source stands in until S52).
- **Owns:** `crates/kudzu-ui/src/main/editor/{mod,model,view,header,triggers,behavior,preview,autosave,scenes}.rs`.
- **Accept:** Autosave: the draft updates at once; saves after 350 ms; flushes on blur, navigation, window close and ⌘S; an outside change is taken only when the draft is clean; **"Saved" appears only when `library.saved()` reaches the draft's revision, and a failed write shows "Couldn't save" until one succeeds, across navigation and hiding the window** (the Tauri app clears its dirty flag before the save finishes, FEATURES §7). Name: Return moves to triggers; new and duplicate focus the name. Triggers: prefix suggestion; a whitespace paste splits atomically; Space, Return, Tab and blur commit; Backspace edits the last; Escape clears; the exact messages for whitespace, duplicate and over 64 graphemes, with shake and duplicate pulse; amber on conflict. Behaviour controls and inherited-source text. Preview: 120 ms debounce, stale results dropped, the four badges, the refresh cue after 600 ms of stillness. Collision banners that select the other snippet; the disabled banner; the no-trigger note. Header menu; the favorite burst only when turning on.
- **Closes:** EDT-1, EDT-2, EDT-3, EDT-4, EDT-16, EDT-17, EDT-18 (shell motion), RUL-6.
- **Depends:** S26, S27, S41, S55 · **Parallel with:** S59, S61
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the snippet editor shell`

#### S52 · Template editor core — L
- **Goal:** The content field: chips, slash menu, Insert menu, undo, IME.
- **Owns:** `crates/kudzu-ui/src/main/editor/{content,chips,slash,insert_menu,history}.rs`, and stubs for `tags/mod.rs`, `format_bar.rs`, `link.rs`, `lists.rs`, `images.rs` that S53 and S54 take over.
- **Read first:** `docs/migration/spikes/S13-editor.md`, including the review guidance and the "Follow-up (S13b)" section. D15 and clipboard behaviour are integration gates.
- **From S13b** (see "Follow-up (S13b)" in `docs/migration/spikes/S13-editor.md`):
  - Chip ids are `{tone}:{source}`, because Rune's rich HTML keeps only the pill id. Move `chip_id`, `source_of`, `copy_content` and `paste_own` into `kudzu-ui/src/main/editor/`, and tag the double-click detector `RUNE-GAP(#128)`.
  - Wire `on_input_key` (Cmd+C and Cmd+X go through `Tx::copy_rich` with the template source as plain text), `on_input_paste` (own rich copies paste back as chips in one undo step), and `on_pointer_move_local` (the D15 detector; call `Tx::live_only()` unless it fires).
  - **D15 gate:** check the inferred double-click in a real window, including a trackpad tap and a nudged double-click. If it's unreliable, add a visible "Configure" button beside the caret whenever the selection is exactly one chip or the caret touches one. Return and Space always work.
  - Clear history after every load, including a snippet switch.
  - **Parity:** plain text containing `{{tags}}` pasted from another app becomes chips, because today's CodeMirror document *is* the source. Confirm the macOS Edit menu's Copy arrives as Cmd+C.
  - **Temporary difference:** the field's right-click Cut and Copy copy chip labels until Rune #578 adds a copy hook. We keep the spelling menu rather than take over `on_secondary_press`.
  - Image pills copy as their alt text (S76).
- **Accept:** Binding tests without a window: edits on `TextEdit` ↔ `EditorDoc` ↔ source keep S23's round-trip over the `template_grammar` corpus. A chip's configuration opens per D15 (a double-click on the chip, or Return or Space when the selection is exactly that chip; a single click once Rune #128 lands), with the gesture proven against an adjacent word, a drag selection and a keyboard selection, as the S13b spike shows. Copying a selection with chips puts the `{{tag}}` source on the plain-text pasteboard and a rich form that same-app paste turns back into chips. The first Undo on an untouched document does nothing. Chips are atomic; unknown and verbatim chips are red with a source tooltip and an accessible edit label. The slash menu opens only for a typed `/` at a word start (start, whitespace, `(`, `[`, a quote), searches variables and snippets, leaves out the snippet itself, caps nested snippets at 400, shows "Nothing matches.", inserts on Return, closes on Escape and returns focus. The Insert menu's 13 entries and the quick toolbar's four. A configurable tag opens its form at once (a stub until S53). Undo and redo restore text and chips. A composition never lands in a chip. Wrapping, Tab indentation, spell checking and the standard editing keys. A script types `/da`, Return, and gets a date chip.
- **Closes:** EDT-5, EDT-6, EDT-7, EDT-8 (popover mechanics).
- **Depends:** S13, S23, S51 · **Parallel with:** S60, S62, S63
- **Implementer:** `claude-fable-5-1` (high) · **Reviewer:** `gpt-6-astra` (xhigh)
- **Commit:** `refactor(desktop-v2): add the template editor with chips and the slash menu`

#### S53 · Chip configuration forms — L
- **Goal:** A configuration form for each of the 12 tags, and for unknown tags.
- **Owns:** `crates/kudzu-ui/src/main/editor/tags/**` (one file per tag).
- **Accept:** Per tag, the controls, clamps and defaults of EDT-9 to EDT-11: date and time (kind; 11 presets and a custom pattern; Today, Tomorrow, Yesterday, +week, +month, +hour and a custom shift; language; an example refreshed every 15 s that, as in the Tauri app, ignores the chosen shift: a known quirk kept as it is, §6); clipboard (history 0–99, trim); selection (trim, the Quick Search note); UUID (capitals, a stable example); random (one option per line); field; choice; nested snippet (search); cursor; key (10-key grid, repeat 1–50, keycap preview, the undo and cursor caveat); delay (presets 100, 300, 500, 1000, 2000; step 50 within 1–5000; the budget hint); unknown (explanation and source). Changes apply at once; Remove tag; Return finishes except in a text area; Escape closes. A golden per form.
- **Closes:** EDT-8 (forms), EDT-9, EDT-10, EDT-11.
- **Depends:** S52 · **Parallel with:** S54
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `refactor(desktop-v2): add the chip configuration forms`

#### S54 · Formatting and images — L
- **Goal:** Formatted snippets in the editor: styles, links, lists and embedded images.
- **Owns:** `crates/kudzu-ui/src/main/editor/{format_bar,link,lists,images}.rs`, `crates/kudzu-app/src/images.rs` (a module S01 pre-declares).
- **Accept:** The Plain text / Formatted selector is one undo step; controls show only in Formatted. Bold, italic and underline need a selection, answer ⌘B, ⌘I, ⌘U and show pressed state. Link editing validates http, https and mailto and keeps the selection. Clear removes styles and images and leaves alt text. Spans follow edits; a style change is its own undo step; undo restores text, formatting and images. A selection grows over chips and images; typing beside an image does not extend it; composition blocks style, list and image operations. List toggles per EDT-14. Images: PNG, JPEG or WebP under 20 MiB; compacted to PNG at most 512 on a side in up to 8 attempts at ×0.75; the data URL and per-snippet limits; width and alt rules; preparing, error and too-many states; a stale request is cancelled; images appear as chips with a preview popover (gap 5). Decoding uses `image`'s pixel and allocation limits. For the S02 editor fixtures, the saved spans (ranges, kinds, URLs, widths) equal what the TypeScript editor produced; embedded images are compared by decoded size and the limits, not by bytes, because the encoders differ. `scripts/tests/formatted-paste.swift` passes on HTML from the new path.
- **Closes:** EDT-12, EDT-13, EDT-14, EDT-15.
- **Depends:** S23, S52 · **Parallel with:** S53
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add formatted snippets and embedded images to the editor`

#### S55 · Group settings and app picker — M
- **Goal:** The group settings dialog, and the app picker every rules control shares.
- **Owns:** `crates/kudzu-ui/src/main/group_settings.rs`, `crates/kudzu-ui/src/widgets/{app_icon,app_picker}.rs`.
- **Accept:** The 520 dialog: name, On/Off, Default/Type/Paste, All apps / Only in / Except with the picker, Save and Cancel; the name is trimmed and the old one kept when blank; **Save waits for the service and shows an error instead of closing on failure** (the Tauri dialog closes without waiting, FEATURES §7). The picker searches names and bundle ids, offers a syntactically valid custom bundle id when nothing matches, stays open with checks, shows a bundle id or monogram for a missing app, and loads icons off the main thread with a cache.
- **Closes:** RUL-7 (UI), RUL-8.
- **Depends:** S26, S30, S41 · **Parallel with:** S42, S43, S50, S58, S64, S65
- **Implementer:** `grok-4.7` (xhigh) · **Reviewer:** `gpt-6.1-terra` (high)
- **Commit:** `refactor(desktop-v2): add group settings and the app picker`

#### S56 · Quick Search palette — L
- **Goal:** The Quick Search panel, from hotkey to insert.
- **Owns:** `crates/kudzu-ui/src/palette/**`, `crates/kudzu-ui/src/actions/palette.rs`.
- **Read first:** the "Review caveats" in `docs/migration/spikes/S11-panels.md` (what this step must re-measure or prove).
- **From S21:** the same `search::index` / `rank` / `highlight` API as S50. The palette's old TS `Highlight` indexed by code point, while the Rust version is consistently UTF-16.
- **From S21 review:** extend the palette tests to cover scope filters, `#group` (with descendants), favourites, recent ordering and a text query together. The S21 fixtures only rank a slice that has already been selected.
- **Accept:** Model: reset of query, scope, active row and flash on show; scopes with Tab and Shift-Tab; `#prefix` group filter (case-insensitive prefix, directly assigned groups); empty-query ordering; limits (term 64, content 2,000, 60 results); arrows and ⌃N/⌃P wrap; Page ±7; ↩ insert; ⌘↩ copy with "Copied" and hide after 260 ms; ⌘E edit; ⌘N create; ⌘1–9; hover selects and click inserts; the modifier lights number hints and key-up or blur releases them; Escape clears a non-empty query, then dismisses and restores the target unless `main` has focus; blur hides without restoring; the create row (a prefixed single token becomes the trigger, otherwise the name). Preview: after 40 ms, keeping the old one, dimming after 150 ms, shimmer on first load, the form badge, metadata, a target-aware footer. Empty-scope copy. Layout: radius 18, search 58, list 54%, hints 40, rows 44. Search through `rune::Latest` within budget. On a Dev bundle: open latency within budget.
- **Closes:** QS-1, QS-2, QS-3, QS-4, QS-5, QS-6, QS-7.
- **Depends:** S07, S21, S27, S40 · **Parallel with:** S41, S57, S66
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `refactor(desktop-v2): add the Quick Search palette`

#### S57 · Fill-in form window — M
- **Goal:** The fill-in form panel that appears at the caret.
- **Owns:** `crates/kudzu-ui/src/form/**`.
- **Read first:** the "Review caveats" in `docs/migration/spikes/S11-panels.md` (what this step must re-measure or prove).
- **Accept:** The window shows itself when `forms.pending()` appears: at caret x − 24, y + caret height + 6; above the caret when it would overflow; x clamped 8 pt inside the screen; centred at 28% without a caret. A draggable header with the snippet and target app. Text, multi-line and choice controls in first-definition order; one value per shared name. The first field is focused; ↩ submits outside a multi-line field and ⌘↩ inside one; Escape cancels and returns to the target. Preview after 50 ms with formatting, images, actions and the selection; shimmer first; at most 160 pt. Height follows content within 160–640, as today (the window itself clamps at 680). No dismissal on blur; the window reports `panel_focused` to `forms`.
- **Closes:** FRM-2, FRM-5 (UI), FRM-6.
- **Depends:** S07, S27, S40 · **Parallel with:** S41, S56, S66
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `refactor(desktop-v2): add the fill-in form window`

#### S58 · Clipboard history: list and actions — L
- **Goal:** The list side of the clipboard history screen (sections, search, selection, actions, bulk) and the row widgets the picker reuses.
- **Owns:** `crates/kudzu-ui/src/clip/{model,list,tile,filters,motion}.rs` (shared with the picker), `crates/kudzu-ui/src/main/clipboard/{mod,view,toolbar,bulk,scenes}.rs`.
- **From S22:** use `RowOptions::history`; selectable indices follow the returned `order`. Highlight ranges are UTF-16 offsets.
- **Accept:** List width clamp 300 / 30% / 372; the 48 pt header with the recording badge and options; search; the paused banner with Resume; sections (Pinned first only without a search and outside Pinned; then Today, Yesterday, weekday, date); rows 58, headers 30, tiles 36. Keys: arrows, J/K, Home, End, Page ±8, Shift range, ⌘-click, ⌘A, ↩ or ⌘C copy, ⇧↩ flips plain and formatted, Space preview, ⌘P pin, ⌘S save as snippet (full plain text), ⌘O open, ⌘R reveal, the delete keys; `/` and typing focus search; the Escape rules. Copy feedback without re-capturing our own write; double-click copies. Bulk pin and delete with a 6 s Undo; secret titles hidden in toasts. Clear history with "Also delete pinned items". Loading, welcome, recording-off and no-match states. Only changed rows move; more than 10 changes stay still; no arrivals while hidden. The 36 pt retention and storage footer. A scroll trace with 5,000 clips inside the budget.
- **Closes:** CLP-8, CLP-9 (UI), CLP-10, CLP-11, CLP-12, CLP-19, CLP-20 (list states).
- **Depends:** S14, S22, S28, S41 · **Parallel with:** S42, S43, S50, S55, S64, S65
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the clipboard history list`

#### S59 · Clipboard history: previews — L
- **Goal:** The preview side: a body per clip kind, metadata, the copy timeline, secret handling and the large preview.
- **Owns:** `crates/kudzu-ui/src/clip/{preview,facts,timeline,secret,large}.rs`, `crates/kudzu-ui/src/clip/content/**`.
- **From S22:** `text_facts` takes optional full text for word counts. Spoken labels for concealed items omit content. Dates and counts use en-US formatting, as the fixtures do.
- **Accept:** Text: selectable, matches highlighted, full text loaded off the main thread with shimmer, at most 60,000 characters drawn with a truncation note. Code: mono with line numbers, at most 1,500 lines and a hidden-line count. Link: host, path, full URL, Open in browser or Write email, no fetched thumbnail. Files: up to 200 paths with Finder icons, Open and Reveal per file, a single-image preview with fallbacks. Colour: a checkerboard swatch, HEX/RGB/HSL/OKLCH rows that copy, contrast against white and black. Image: the original on a checkerboard with a thumbnail fallback. Other: the "all formats retained" note. Metadata rows, the raw-formats expander, the Formatted badge. The copy timeline with tooltips (hidden in compact). Secrets: 8–18 dots in rows; a concealed preview that keeps its layout and cannot be selected, copied or read by an assistive client; reveal on hover or keyboard focus; concealed accessibility labels; Not a secret? and Hide as a secret with Undo; the Show secrets setting. The large preview (min(920, 90%) by 86%) with Space, Escape and arrows.
- **Closes:** CLP-13, CLP-14, CLP-15, CLP-16, CLP-17, CLP-18, CLP-20 (preview states), WIN-2 (large preview).
- **Depends:** S24, S58 · **Parallel with:** S51, S61
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `opencode-go/deepseek-v4-pro`
- **Commit:** `refactor(desktop-v2): add clipboard previews, metadata and secret handling`

#### S60 · Clipboard picker window — M
- **Goal:** The clipboard picker panel, from hotkey to paste.
- **Owns:** `crates/kudzu-ui/src/picker/**`, `crates/kudzu-ui/src/actions/picker.rs`.
- **Read first:** the "Review caveats" in `docs/migration/spikes/S11-panels.md` (what this step must re-measure or prove).
- **From S22:** use `RowOptions::picker`; it keeps history order and shows sections only when there is no query.
- **Accept:** Unavailable while recording is off, and hidden if recording is turned off while it is open. Each opening resets filter, query, selection and undo and focuses search. Newest first including pinned; date sections only without a query. Filter pills All, Pinned, Text, Images, Links, Files with Tab and Shift-Tab. Arrows and ⌃N/⌃P clamp; Page ±7. ↩ and double-click paste; ⇧ flips the plain default; ⌘↩ copies only and hides after 420 ms; ⌘1–9; ⌘P; ⌘S; ⌘O and dismiss; ⌘, opens clipboard settings. Escape clears, then closes. ⌘⌫ deletes and moves to a neighbour; ⌘Z restores the last deletion only with an empty query. The footer flash. Without permission: the footer says "Copy", the fallback text shows, and the window hides after 1,800 ms. The four empty states. Compact rows 50, 28, 32. Layout: radius 18, search 58 with pills, list 50%, hints 40.
- **Closes:** PKR-1, PKR-2, PKR-3, PKR-4, PKR-6.
- **Depends:** S28, S40, S59 · **Parallel with:** S52, S62, S63
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add the clipboard picker window`

#### S61 · Settings: frame, General, Expansion, Apps — L
- **Goal:** The settings frame with its eight-pane navigation, and the three panes that need only settings and apps.
- **Owns:** `crates/kudzu-ui/src/main/settings/{mod,nav,general,expansion,apps,scenes}.rs`, and stubs for the five panes S62 and S63 take over.
- **Accept:** Eight panes in order (General, Expansion, Activity, Clipboard, AI agents, Apps, Backups & data, About) with ↑ and ↓ cycling; the 192 pt nav, 48 pt heading and a body at most 680 wide. General: Permissions Set up or Review with status; the shortcut recorder ("Press shortcut…", "That shortcut is taken", rollback); the iCloud switch with a status slot S63 fills; Appearance; Open at login; Show in Dock while open. Expansion: Pause, the trigger prefix (at most 4), Insert by, the informational rows. Apps: Never expand chips; per-app insertion rows sorted by name with Auto, Type, Paste and remove; add (default Paste); a typed bundle id. Each control writes through `settings`, and the reaction is observed on the fake ports (login item, Dock, theme, engine).
- **Closes:** SET-1, SET-2, SET-3, SET-4, SET-9.
- **Depends:** S26, S27, S30, S41, S55 · **Parallel with:** S51, S59
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `opencode-go/glm-5.3`
- **Commit:** `refactor(desktop-v2): add the General, Expansion and Apps settings`

#### S62 · Settings: Clipboard, AI agents, Backups & data — L
- **Goal:** The three panes that drive the clipboard, agents and data services.
- **Owns:** `crates/kudzu-ui/src/main/settings/{clipboard,agents,data}.rs`.
- **Accept:** Clipboard: Record; the shortcut; Keep history presets 1, 7, 30, 90, 365, forever and Custom 1–3650 (rounds on blur or Return, an invalid value resets); Paste as plain text; Show secrets; ignored apps; storage (count, bytes, pinned) and Clear history (keeps pinned, no undo). AI agents: Allow, Create and edit, Share clipboard, with dependents dimmed and their stored values kept; command and JSON copy with a 1,600 ms check and a toast; Install or Update skill states; the redaction text. Backups & data: the detected Espanso row; Raycast, TextExpander, Kudzu and Espanso choosers (answered by scripted `file-dialog` events); counts, warnings, sample and failures; JSON and YAML export; the backup list (newest 14, exact and relative time, reason labels); Back up now and Restore busy states; the restore message; Show data folder.
- **Closes:** SET-6, SET-8, SET-10, MCP-10, DAT-9, DAT-12.
- **Depends:** S28, S29, S32, S61 · **Parallel with:** S52, S60, S63
- **Implementer:** `gpt-6.1-terra` (high) · **Reviewer:** `grok-4.6` (xhigh)
- **Commit:** `refactor(desktop-v2): add the Clipboard, AI agents and data settings`

#### S63 · Settings: Activity, About, sync status, update toast — M
- **Goal:** The surfaces that show live status: Activity, About with updates, sync status, and the update toast.
- **Owns:** `crates/kudzu-ui/src/main/settings/{activity,about,sync_status}.rs`, `crates/kudzu-ui/src/main/update_toast.rs`.
- **Accept:** Activity: the health card with its action; rows (time, snippet, trigger, source, app, outcome badge, reason, fix); names for a deleted snippet and the keyboard listener; Clear disabled when empty; the "Nothing yet" guidance; fix buttons that re-evaluate and show "Fixed since"; the Secure Input help dialog (460), which never changes a system setting; a scene with every outcome. About: icon hover, wordmark, version, tagline; every update phase with progress (bytes and percent, 30% when the total is unknown, shimmer); Check now, Try again, Restart; Check automatically; Get beta updates; release notes through `Markdown` with the All release notes link; the privacy card. Sync status for all eight states with relative time; Sync now only for up-to-date and error; the newer-format action opens About. The update toast: once per version outside Settings, 15 s, Restart and What's new.
- **Closes:** SET-5, SET-11, ACT-3, ACT-4, ACT-5, SHL-10, SYN-7.
- **Depends:** S24, S27, S31, S33, S61 · **Parallel with:** S52, S60, S62
- **Implementer:** `grok-4.7` (xhigh) · **Reviewer:** `gpt-6.1-sol` (medium)
- **Commit:** `refactor(desktop-v2): add the Activity and About settings and update status`

#### S64 · Insights — M
- **Goal:** The Insights screen.
- **Owns:** `crates/kudzu-ui/src/main/insights/**`.
- **From S21:** use `usage_by_id` and `totals` (`minutes_saved` matches `lib/stats.ts`). Duration, count and relative-time formatting come from S24's `fmt`.
- **Accept:** Four tiles (Expansions, Characters saved, Time saved, Active snippets). Most used top 10 with relative bars; Recent 6; Never used up to 18 (enabled, with a trigger). A click goes to All with that snippet selected. Empty states. The WPM slider 10–150 in steps of 5, default 40, saved at once. Count-up over 560 ms (quartic) and bars over 480 ms with a 60 + i×40 ms stagger on arrival only; later changes are instant; Reduce Motion skips both. At most 880 wide; columns 1.4 : 1. Motion gate.
- **Closes:** INS-2, INS-3, INS-4.
- **Depends:** S21, S26, S41 · **Parallel with:** S42, S43, S50, S55, S58, S65
- **Implementer:** `opencode-go/deepseek-v4-pro` · **Reviewer:** `gpt-6.1-terra` (medium)
- **Commit:** `refactor(desktop-v2): add Insights`

#### S65 · Onboarding — L
- **Goal:** The first-run flow.
- **Owns:** `crates/kudzu-ui/src/main/onboarding/**`.
- **Accept:** Welcome → Access → Import → Try it with Back, Skip and progress dots; content centred, at most 480 wide, with a stable gutter. Access uses the permissions machine and opens the companion. Import: three source cards with Looking, Found, Not found and Failed; the choose-folder fallback; counts, warnings and errors; busy guards; focus restored after a dialog. Try it: typing `;kudzu` substitutes "Hello from Kudzu!" locally, with the celebration and the hotkey reminder; Open library saves completion, shows "Opening…", and offers a retry on failure. Scenes `onboarding`, `onboarding-espanso`, `onboarding-espanso-retry`, `onboarding-finish-retry`. Step motion ±12 pt over 240 ms.
- **Closes:** ONB-1, ONB-3, ONB-4.
- **Depends:** S29, S30, S41 · **Parallel with:** S42, S43, S50, S55, S58, S64
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `refactor(desktop-v2): add onboarding`

#### S66 · Permission companion window — L
- **Goal:** The permission companion window and its guide over System Settings.
- **Owns:** `crates/kudzu-ui/src/permissions/**` (window and guide overlay).
- **Read first:** "Review findings" in `docs/migration/spikes/S15-permissions.md` (native guide panel as the default, the `macos.m` lifecycle, the Input Monitoring pane, a keyboard alternative, the privacy guard, exact placement tests).
- **Accept:** Two selectable rows (Accessibility, required; Input Monitoring, optional) with the macOS 27 name and the real app name. States checking, opening, waiting, allowed, connecting, revoked, error; Help, close, Finder, restart. The app icon drags the running bundle out with `drag_out` (disabled with an explanation when there is no bundle); a cancelled drag returns; a click reveals the bundle. Open Settings lands on the pane; the companion moves beside the Settings window (18 pt gap, 12 pt screen margin; retries every 200 ms up to 12 times after 450 ms) and keeps its place when the frame is unknown. The guide: a click-through outline and a ghost icon, twice on a 2.4 s cycle; it stops on a real drag, a grant, a move of either window, another app in front, close or Reduce Motion, and is skipped when the screen cannot fit both. A one-second refresh while visible and on focus, never overlapping. Restart offered after 5 s of grant without a listener. Escape or close hides it and shows `main`. It reopens after an explicit restart. A scene per state.
- **Closes:** ONB-5, ONB-6, ONB-7, ONB-8, ONB-9, ONB-10.
- **Depends:** S15, S30, S38, S40 · **Parallel with:** S41, S56, S57
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `refactor(desktop-v2): add the permission companion and guide`

### Release, verification and cut-over

#### S70 · Release pipeline in xtask — L
- **Goal:** Build, sign, notarize and package a release of the new app without the Tauri CLI or Node, proven on CI before cut-over.
- **Owns:** `xtask/src/{release,sign,dmg,feed,version}.rs`, `scripts/release/{build.sh,set-github-secrets.sh,update-cask.sh,cloudkit-schema.sh}` (new files in `apps/desktop-v2`, written from the Tauri app's scripts), `.github/workflows/desktop-v2-release.yml` (new, dispatch-only, publishes nothing; deleted in S75).
- **Deliverables:** `cargo xtask release` and `cargo xtask feed` per §1.6, including the CI keychain (create, import the P12, unlock, add to the search list, delete on exit). `cargo xtask set-version`, which writes `version.txt`. `build.sh` with every check the Tauri app's `build.sh` has (design assets, profile app id and container, certificate fingerprint against the profile, universal binary, entitlements, `spctl`, stapler). `update-cask.sh` with the current description and a `zap` list without the WebKit and HTTPStorages paths. The dispatch workflow: builds the release candidate from the branch with the real secrets, signs, notarizes, staples, verifies, and attaches the artifacts to the run only.
- **From S03:** the DMG background renderer reads a pre-rendered wordmark bitmap. The design manifest tracks it, but a wordmark SVG change still needs the bitmap regenerated first. Make the exporter regenerate it, or add the step to RELEASING.md.
- **From S31:** sign with the production secret (`TAURI_SIGNING_PRIVATE_KEY` → `KUDZU_UPDATE_KEY`). The public key is `PRODUCTION_PUBKEY`, and its Tauri wrapping is `PRODUCTION_PUBKEY_TAURI_B64`. Verification is prehashed (`allow_legacy=false`). The feed JSON is `{version, notes, pub_date, platforms: {darwin-aarch64, darwin-x86_64: {signature, url}}}`, where `signature` is the **base64-wrapped** minisign file text, exactly as Tauri's `.sig` is today (S31 review: raw text would strand installed copies). The trusted comment is `timestamp:<unix>`, a tab, then `file:Kudzu_<version>_universal.app.tar.gz`, and the archive is named `Kudzu_<v>_universal.app.tar.gz`.
- **Accept:** Feed tests reproduce `feed.mjs` (semver precedence with pre-release tags). `latest.json` has the current keys and both darwin platforms. A signature made from a key in the existing format verifies with the app's verifier. `SKIP_NOTARIZE=1 scripts/release/build.sh` works locally. The DMG shows the background, a 660×440 window, and icons at (170, 220) and (490, 220). One dispatched run is green end to end: app and DMG notarized and stapled, `spctl` accepts both, both slices report `minos 14.0`, and the keychain is gone afterwards.
- **Closes:** SHL-12 (artifacts), SHL-8 (feed part).
- **Depends:** S03, S31, S36 · **Parallel with:** S32, S35, S37
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `build(desktop-v2): build, sign and package releases from xtask`

#### S71 · Native verification: typing, forms, clipboard — L
- **Goal:** Evidence, on a real Mac, for the input paths headless tests cannot show, with the Tauri app beside the new one.
- **Owns:** `docs/migration/NATIVE_INPUT.md`; fixes go to the owning module as separate small commits.
- **Needs from the owner (agreed, §6 D7):** Accessibility and Input Monitoring granted to a Kudzu Next bundle signed with the owner's Developer ID.
- **Checklist (record pass or fail with evidence):** EXP-6 to EXP-18 in TextEdit, Notes, a browser field and Terminal, with ABC, Dvorak, a non-Latin layout, Japanese IME and Vietnamese Telex (copy and paste keys follow the layout; Roman and composing modes); Secure Input; losing and regaining permission, listen-only and active tap; Backspace undo, also after a cursor move; key and delay sequences across fields, and their cancellation; formatted and image paste, including an app that reports no caret movement; held-input replay; forms at the caret from a typed trigger and from the palette, with the FRM-4 interruptions; Quick Search insert, copy and selection, with focus returning to the target under a non-activating panel; picker paste, the copy fallback without permission, a vanished target; capture of every clip kind and of private markers from a password manager; MCP through `claude mcp add` and the offline bridge; Spaces, a fullscreen app, two displays with different scales; a VoiceOver pass over each window.
- **Read first:** the "Review caveats" in `docs/migration/spikes/S11-panels.md` (what this step must re-measure or prove).
- **Owner live checks deferred here:** the 13-step checklist in `docs/migration/spikes/S10-menubar.md` (run against the production shell, not the spike) and the S11 caveat list in `docs/migration/spikes/S11-panels.md`.
- **Owner live checks from S15** (`docs/migration/spikes/S15-permissions.md`): drag Kudzu Next into Accessibility and cancel the prompt; reveal in Finder; click through the guide overlay into Settings; the companion on a full-screen Space and a second display; a narrow display and Reduce Motion; set `KUDZU_DEV_SIGN` so permission grants survive rebuilds.
- **Live checks from the S39 review:** in TextEdit, Notes, browser fields and Terminal, test typed and Quick Search forms with clicks inside the form, clicks into the target before focus notifications arrive, moving between Kudzu panels, and input during submission, and check the text and caret after submitting. Test slow or busy targets that read the pasteboard after ⌘V near the 400 ms restore, and apps that consume, delay or reorder synthetic keys or rich and image pastes. Cover Japanese IME, Vietnamese Telex, non-Latin and alternate layouts, secure fields and Secure Input, and losing permission mid-insertion. Check held-key replay, click cancellation, focus changes, and clipboard-history picks or newer external copies during a pending restore, with several flavours and promised data.
- **Live checks from the S15 review:**
  - Cancel a drag mid-way (Esc, or drop outside) and cancel the auth prompt, as separate checks.
  - The first click and drag work with Settings frontmost and Kudzu inactive.
  - The listed item is this bundle.
  - The Input Monitoring pane works.
  - Move or resize Settings while the guide plays; put Settings on another display or in a full-screen Space.
  - Stage Manager, and a notched or Dock-side display.
  - No Screen Recording prompt appears, and the bundle is absent from that pane.
  - Keyboard-only and VoiceOver use of the icon alternative.
  - A real click through the native guide panel.
- **Accept:** `NATIVE_INPUT.md` complete. Every failure fixed, or carried to S74 for the owner.
- **Closes:** EXP-6–19, TPL-9, TPL-10, PKR-5, WIN-3 (live confirmation).
- **Depends:** S39, S40–S66 · **Parallel with:** S72, S73
- **Implementer:** `gpt-6-astra` (xhigh) · **Reviewer:** `grok-4.7` (xhigh)
- **Commit:** `test(desktop-v2): record native verification of typing, forms and clipboard`

#### S72 · Native verification: lifecycle, sync, updates, performance — L
- **Goal:** Evidence for the app as an installed product: the release candidate, its lifecycle, iCloud, updates and the budgets.
- **Owns:** `docs/migration/NATIVE_SYSTEM.md`; fixes go to the owning module as separate small commits.
- **Needs from the owner (agreed, §6 D7):** one production-identity build run against the owner's iCloud account. It uses `sh.saeed.kudzu`, so quit the installed Kudzu and back up its data folder first.
- **Checklist (record pass or fail with evidence):** the notarized artifact from S70's workflow installs from its DMG and runs on Apple Silicon, and its x86_64 slice runs under Rosetta (`arch -x86_64`); first run; hidden start at login; the Dock toggle; ⌘Q, the tray's Quit, Dock Quit and logout, each with an unsaved draft; light, dark and System; vibrancy, native shadows and corner clipping beside the Tauri app; Reduce Motion; iCloud between two Macs through each status, with an account change; an update from a local feed, new app to new app (download, verify, install on quit, Restart, needs-move from the DMG, a tampered archive refused). Then every row of the §1.7 budget table beside `reference/BASELINE.md`.
- **Accept:** `NATIVE_SYSTEM.md` complete, with the machine, macOS version and display. Every failure fixed, or carried to S74. Every budget met, or the miss listed with its cause and upstream issue.
- **Closes:** SHL-8, SHL-9, SYN-1–6 (live confirmation).
- **Depends:** S40–S66, S70 · **Parallel with:** S71, S73
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `opencode-go/gemini-4-argon`
- **Commit:** `test(desktop-v2): record native verification of lifecycle, sync, updates and performance`

#### S73 · Visual parity pass — L
- **Goal:** Every reference state compared with its Rune scene, and every difference fixed or explained.
- **Owns:** `docs/migration/PARITY_REPORT.md`; fixes go to the owning module. Generated sheets stay in `target/parity/`, not in git.
- **Accept:** `cargo xtask parity` renders every scene in `reference/STATES.md` in both themes. No landmark is off by more than 2 pt and no token colour differs, or the row is in `PARITY_REPORT.md` with its reason, for the owner to approve in S74; a reviewer cannot waive it. In-main dialog sizes match WIN-2. Motion durations and curves spot-checked against FEATURES §3.
- **Closes:** WIN-2.
- **Depends:** S02, S41–S66 · **Parallel with:** S71, S72
- **Implementer:** `gpt-6.1-sol` (high) · **Reviewer:** `claude-opus-5-5` (high)
- **Commit:** `test(desktop-v2): compare every screen with the reference and close the gaps`

#### S74 · Parity audit — L
- **Goal:** An independent check of the finished app against `FEATURES.md`, while the Tauri app is still there to compare with.
- **Owns:** `docs/migration/AUDIT.md`.
- **Deliverables:** One row for each of the 195 checks and each FEATURES §7 item: **pass** (with evidence: a test name, a scene, or a row of `NATIVE_INPUT.md` or `NATIVE_SYSTEM.md`), **intentional change** (with the §6 entry that allows it), or **gap**. The §3 table re-checked against upstream: which issues closed and which workarounds can go. `cargo metadata` compared with §2. The final budget numbers. The follow-up list: S76, `icu_datetime`, SMAppService, the `secrets_scanner` trial, syntax highlighting for code clips.
- **Accept:** No row without evidence. The auditor runs the app and the tests; it does not copy claims from step reports. **Cut-over gate:** no open gap that can lose data, or type or delete text in the wrong place; every other gap, and every difference not already in §6, is approved by the owner in `AUDIT.md`.
- **Closes:** all remaining checks.
- **Depends:** S71, S72, S73 · **Parallel with:** none
- **Implementer:** `claude-opus-5-5` (high) · **Reviewer:** `gpt-6-astra` (xhigh)
- **Commit:** `docs(desktop-v2): audit the Rune app against the parity checklist`

#### S75 · Cut-over — M
- **Goal:** The Tauri app is deleted and `apps/desktop-v2` becomes `apps/desktop`, so CI, release-please, the Homebrew cask and the docs keep their paths. One commit, so every commit on the branch builds.
- **Owns:** the whole repository for this one step.
- **Before the commit:**
  1. S74's gate has passed and the owner has signed `AUDIT.md`.
  2. `git log <commit in COPIED_FROM.md>..HEAD -- apps/desktop` shows no change to a copied file, or each change has been ported to `apps/desktop-v2`.
  3. No release-please PR is open (the owner merges or closes it first).
- **In the commit, in this order:**
  1. **Carry over** into `apps/desktop-v2`: `apps/desktop/CHANGELOG.md` as it stands; `version.txt` set to the value in `.release-please-manifest.json`.
  2. **Finish the new app's own files:** the Dev identity becomes Kudzu Dev (`sh.saeed.kudzu.dev`, `kudzu-dev`); delete `spikes/`, `COPIED_FROM.md`, `docs/migration/reference/{capture.mjs,fixtures.mjs}`; write `README.md` (layout, `cargo xtask run`, headless flags, scenes in place of the preview's query params), `RELEASING.md` (version source, xtask, secret mapping, local build), `AGENTS.md` (Rust and Rune rules), `docs/PERMISSIONS.md` and `docs/TEMPLATE_SYNTAX.md` (module paths, verification).
  3. **Swap:** `git rm -r apps/desktop`, then `git mv apps/desktop-v2 apps/desktop`.
  4. **Rename references:** every `desktop-v2` in the moved tree (`xtask` constants, scripts, docs outside `docs/migration/`) becomes `desktop`. `check-deps` drops its rule against the string `apps/desktop/`, which now names the app itself, and keeps the rule against paths that leave the workspace.
  5. **Root workspace:** remove `dev:desktop`, `dev:app`, `build:desktop`, `test:rust`, `lint:rust` from `package.json`; run `pnpm install` to drop the desktop importer from the lockfile. `pnpm-workspace.yaml`, `turbo.json` and `packages/*` stay: the website uses them.
  6. **CI:** delete `.github/workflows/desktop-v2.yml` and `desktop-v2-release.yml`. In `ci.yml`, the job id `desktop` stays (it is a required check) and becomes the v2 job at `apps/desktop`: design-asset checks, fmt, clippy, tests, `check-deps`, headless trees and goldens, `app-audit --strict`. A `website` job keeps `pnpm check`, the website tests and build. Update path filters and the Rust cache path. Add `workflow_dispatch`, because `ci.yml` otherwise runs on pushes to `main` and on pull requests only, and this branch has neither.
  7. **Release workflows:** `build-macos.yml` and `beta.yml` drop pnpm and Node, read the version from `version.txt`, stamp betas with `cargo xtask set-version`, write the profile to `assets/Kudzu.provisionprofile`, map the updater secrets to `KUDZU_UPDATE_KEY*`, and call `scripts/release/build.sh`. `release.yml` keeps its shape.
  8. **release-please:** `apps/desktop` becomes `"release-type": "simple"`, which updates `version.txt` and `CHANGELOG.md`. Confirm with a dry run.
  9. **Root docs:** `AGENTS.md` (`CLAUDE.md` is a symlink to it): layout, commands, the crate rules of §1.1, the `RUNE-GAP` convention, the Rune pin. `README.md`: source links (`src-tauri/crates/…` → `crates/…`) and run instructions.
  10. **Website (links only):** `apps/website/lib/site.ts` and `lib/worst-case.ts` point at the moved source files and at the executable `Kudzu.app/Contents/MacOS/Kudzu`; `apps/website/README.md` mentions the new layout. Nothing else in the website changes.
- **Accept:** `apps/desktop-v2` does not exist. `rg "desktop-v2" -g '!apps/desktop/docs/migration/**'` finds nothing. `rg -il "tauri|next\.js|react|shadcn|tailwind|typescript|webview" apps/desktop -g '!docs/migration/**' -g '!CHANGELOG.md'` lists only files where the mention is deliberate history; name them in the commit body. No `package.json`, `node_modules`, `.ts` or `.tsx` under `apps/desktop`. The paths the root README links to exist (`design/app-icon/Kudzu-1024.png`, `docs/PERMISSIONS.md`, `docs/TEMPLATE_SYNTAX.md`, `README.md`, `RELEASING.md`). At the root, `pnpm install --frozen-lockfile && pnpm check && pnpm build` pass. A dispatched run of `ci.yml` on the branch is green. `SKIP_NOTARIZE=1 scripts/release/build.sh` passes from `apps/desktop`. The release-please dry run proposes the next version from this commit alone: no `desktop-v2` commit appears in the proposed changelog. A Kudzu Dev bundle built after the rename launches with its own data folder, `Kudzu --mcp --dev` answers as `kudzu-dev`, the login item points at the new path, and the bundle carries `skills/kudzu/SKILL.md`.
- **Afterwards:** when the owner accepts the cut-over, a follow-up commit deletes `docs/migration/reference/`.
- **Closes:** the goal: no Tauri, webview, Next.js, React, shadcn, Tailwind or TypeScript in the desktop app.
- **Depends:** S74 · **Parallel with:** none
- **Implementer:** `gpt-6-astra` (high) · **Reviewer:** `claude-sonnet-5-5` (high)
- **Commit:** `feat(desktop): rebuild Kudzu as a native Rust app on Rune` (the one changelog entry for the rewrite)

#### S76 · Follow-up: inline images in the editor — L
- **Goal:** Draw embedded images inline in the snippet editor, as the Tauri app does, if Rune still cannot (§6 D2).
- **Runs only if** anomalyco/rune#350 is still open at the pinned rev when S75 is done. If Rune has shipped inline images, bump the pin and use them instead; the step shrinks to S.
- **Owns:** `apps/desktop/crates/kudzu-ui/src/main/editor/images.rs` (the path after the rename), its scenes and tests. The whole workaround stays in that one file.
- **Deliverables:** An image drawn at its width inside the text, where S54 shows a chip, by the approach S13's note found for drawing over a reserved range. It stays atomic: the caret steps over it, a selection covers it whole, Backspace deletes it, typing beside it does not extend it. The width and alt controls of S54 are unchanged. A `RUNE-GAP(#350)` header says what to delete when Rune ships it.
- **Accept:** S54's formatting and image tests still pass. The editor scene with an image matches its reference state within 2 pt. Undo and redo, and IME composition beside an image, behave as in S54. Typing cost on a 2,000-character snippet does not rise. The inline-image entry leaves §6's list of intentional differences.
- **Closes:** EDT-15 (presentation), ending the exception in §6 D2.
- **Depends:** S75 · **Parallel with:** none
- **Implementer:** `claude-fable-5-1` (high) · **Reviewer:** `gpt-6-astra` (high)
- **Commit:** `feat(desktop): show embedded images inline in the snippet editor`

---

## 5. Risks and mitigations

| # | Risk | Likelihood, impact | Mitigation |
|---|---|---|---|
| 1 | **The template editor does not fit Rune's text field.** No pill press handler (#128), no text hit-testing (#353), no inline image (#350), reshaping on every keystroke (#535). | High, high | S13 runs in wave 2, before any editor code. S23 keeps the hard logic pure and tested, so the Rune binding in S52 is thin. If S13 fails outright, stop and report: the fallback (an app-drawn editor on `App::on_ime`) is a different, larger plan. |
| 2 | **Rune API drift.** Pre-release, no tags, README behind the source. | High, medium | One pinned rev. Bump on purpose, as its own commit, after reading the changelog and re-running goldens and the issues in risk 4. Rune types stay inside `kudzu-ui`; `kudzu-appkit` and everything below never see them. |
| 3 | **rune-kit cannot match the current look**, pushing work toward a bespoke kit. | Medium, high | S06 measures every primitive before screens start. Wrappers need a matrix row and an upstream issue. §6 D6 sets how exact to be. |
| 4 | **Virtual list defects** (#203, #546, #549, #550) hurt the two hot lists. | Medium, medium | S14 measures with 5,000 rows first. Keys are ids; filtering replaces the slice and bumps the version; selection is kept in the model, not in Rune. Re-test at every Rune bump. |
| 5 | **Panel latency and focus** (#335, #388) make the palette feel slower than the Tauri app's. | Medium, high | S11 measures against the budget in wave 2. Gap 9 has a ready workaround. |
| 6 | **Memory with three hidden prewarmed panels** (#161, #162). | Medium, medium | Budget and baseline in §1.7; measured in S11 and S71. If over: create the form panel on first use, or release the picker's GPU surface while recording is off. |
| 7 | **⌘Q and the app menu depend on Rune's menu internals** (first submenu, `terminate:`). | Low, high | Proven in S10; `app_menu.rs` is idempotent and fails closed (if it cannot find the item, ⌘Q quits and the tray still offers Open). The upstream issue asks for the supported route. |
| 8 | **Behaviour drifts between the two apps.** The TypeScript logic is rewritten, and the copied Rust is restructured. | Medium, medium | TS parity fixtures (S02) pin search, clipboard, stats, chips, colour and editor transactions. S20 and S34 change structure under the copied tests, with assertions untouched and byte-exact goldens for persisted formats. The Tauri app stays runnable beside the new one until S75. The audit (S74) is done by a different model than most implementers. |
| 9 | **The two copies diverge.** A fix lands in `apps/desktop` after S01's copy and is missed. | Low, medium | `apps/desktop` is frozen by rule 1; `COPIED_FROM.md` names the source commit, and S75 refuses to proceed while `git log` shows unported changes. |
| 10 | **A release is cut from the half-built app.** | Low, high | Three guards: work never touches `apps/desktop/`; release-please counts only commits under `apps/desktop/` (S01 proves `apps/desktop-v2/` is not matched, with a dry run); every commit before S75 uses a hidden type. The existing workflows are not edited until S75. |
| 11 | **Two Dev apps collide** (same bundle id, data folder, grants or hotkeys). | Medium, low | The new Dev bundle is Kudzu Next (`sh.saeed.kudzu.next`) until S75. It uses the same Shift hotkeys as the Tauri Dev app, so a conflict when both run shows as a registration error and rolls back, as designed. |
| 12 | **The engine regresses during its restructure.** Its failures are silent and system-wide. | Low, high | S39 pins the insertion protocols with tests first; S34 then changes structure only, with test names compared before and after. S71 re-verifies EXP-6–19 live, side by side with the Tauri app. |
| 13 | **Headless goldens differ between machines** (system font, GPU). | Medium, low | Goldens use bundled fonts and stay few. Trees and landmark rects carry most of the weight. S01 records what the CI runner can render. |
| 14 | **Native checks need a person.** TCC grants, iCloud and notarization cannot be automated. | Certain, medium | S71 lists exactly what the owner must grant. A stable Dev signing identity keeps grants across rebuilds. |
| 15 | **Release tooling regresses at cut-over** (signing, notarization, DMG layout, updater signature, the switch to `version.txt`). | Medium, high | `xtask` ports every check the Tauri `build.sh` has. S70's dispatch workflow notarizes a real candidate on CI before S75. S75 dry-runs release-please. Cut a production-identity beta before the first stable release, as RELEASING.md already requires. |
| 16 | **Parallel steps collide.** | Medium, low | Disjoint owned paths; pre-declared module trees and dependencies; append-only registries; the gate re-runs after every merge. |
| 17 | **The serial middle.** Wave 7 is one step (S40) that everything visible waits for. | Certain, low | It is assembly, not invention: every service and adapter it composes is already tested. Screens build on fixtures, so nothing after it waits for a real adapter. |
| 18 | **Sidebar material is visibly different.** | Medium, low | S12 produces side-by-side images for the owner; the AppKit fallback is scoped in §3. |

---

## 6. Decisions

Recorded 2026-11-11, when the owner approved the plan. A step that needs to depart from one of these stops and asks.

| # | Decision | Applies to |
|---|---|---|
| D1 | **Dependencies:** everything §2 marks *approved* may be used. Keep-ours stands for secret detection, the MCP server (no `rmcp`, no tokio), fuzzy ranking and date formatting. | §2 |
| D2 | **Inline images in the editor:** a chip with a preview popover until Rune ships inline images. Comment on #350 with the snippet-editor case. If Rune has not shipped them by cut-over, S76 builds them, contained in `kudzu-ui/src/main/editor/images.rs`. | §3 gap 5, S13, S54, S76 |
| D3 | **Sidebar material:** decided 2026-11-11 from S12's side-by-side: the **native sidebar material** (`NSVisualEffectMaterialSidebar`, behind-window, follows the window's active state), as the Tauri app uses. It flattens when the window is inactive, as macOS sidebars do. About 35 lines in `kudzu-appkit/src/sidebar_material.rs`, deleted once Rune ships #562. | §3 gap 3, S12, S38, S42 |
| D4 | **Secret detection:** keep `kudzu_core::secrets`; trial `secrets_scanner` as an extra token source after the audit. | §2 row 2, S74 follow-ups |
| D5 | **Known quirks of the Tauri app** (FEATURES §7) are built as they are. Three exceptions, all about failed saves: the editor's "Saved" state (S51), the group dialog's Save (S55), and disk write errors (§1.6). | S27 (RUL-9), S29 (backup pruning), S53 (date example), and the rest of FEATURES §7 |
| D6 | **Component metrics:** exact for buttons, inputs, switches, **and (added 2026-11-11 after S06)** the select trigger (32 pt), menu, dialog and toast radii (menus 10, dialogs 14–18, as in the Tauri app), hover and pressed shadows plus the switch knob stretch, and the checkbox's checked colours (primary). rune-kit's own metrics are used elsewhere. Wrappers carry `RUNE-GAP(#573)` (or `#360` for the shadows) and go when Rune ships them. | S06, S07, §3 gap 17 |
| D7 | **Live verification:** the owner grants Accessibility and Input Monitoring, provides a Developer ID identity, and runs one production-identity iCloud build when S71 and S72 start. | S71, S72 |
| D8 | **Dev identity before cut-over:** Kudzu Next (`sh.saeed.kudzu.next`). | §1.6, S03, S75 |
| D9 | **FRM-4 form safety:** build the fix. A form interrupted by typing in the target app is not submitted, so it never deletes text in the wrong place. A fourth fix beside D5's three. | §1.2, S39, S57 |
| D10 | **CI:** the orchestrator pushes `t3/rune-migration-plan` to origin after merges so the `desktop-v2` workflow runs. No PRs. | S01 onward |
| D11 | **Model routing (2026-11-11):** spread implementation across providers (Claude, Codex, Grok, OpenCode Go); no single provider takes most steps. Claude has headroom for now and takes the hardest steps; Codex is used a little more lightly than §4 says. The orchestrator may reassign a step's implementer and reviewer from §4; implementer and reviewer still come from different providers, and the step report names the models used. | §4 |
| D12 | **Reference screenshots stay out of git.** Scripts, `metrics.json` and fixtures are committed; the PNGs live at `$KUDZU_REFERENCE_DIR` (default `~/.local/share/kudzu-migration/reference/`). Pixel comparisons run locally only. | S02, S06, S73 |
| D13 | **Collation:** `icu_collator` (ICU4X) is approved (2026-11-11) for `localeCompare`-compatible sorting of snippet and group names, replacing S21's hand-written approximation. Pin the version and data, check against browser-recorded orderings, and report the binary-size cost. | §2, S21 |
| D14 | **Refused paste never loses text (fix, 2026-11-11):** a paste-based expansion saves and writes the clipboard **before** sending any Backspace. If posting ⌘V still fails afterwards, the trigger is typed back. Today's engine sends the Backspaces first and leaves neither the trigger nor the snippet (found by S39). This is a fix beyond D5 and D9. | S34 (with S39's pinned test updated to the fixed behaviour) |
| D15 | **Opening a chip's settings (2026-11-11, after S13):** Rune's text field doesn't select a chip on a single click (#128, #353), so a chip's settings open on a **double-click**, or with **Return or Space** when the caret selection is exactly that chip. This switches to a single click once Rune ships a pill press handler. The code is tagged `RUNE-GAP(#128)` in `kudzu-ui/src/main/editor/chips.rs`. | §3 gap 6, S52, S53 |
| D16 | **Update digest:** the staged-archive integrity check between the update service and the installer uses `blake3`, which Rune already compiles, rather than adding `sha2` (decided 2026-11-11). | S31, S36 |

**Intentional differences from the Tauri app.** Everything else is parity. The audit (S74) accepts a difference only if it is on this list or the owner adds it.

- Images in the editor are chips with a preview (D2), until S76 or Rune.
- A chip's settings open on a double-click or Return/Space, not a single click (D15), until Rune #128.
- Until Rune #578, the editor's right-click Cut and Copy copy chip labels rather than `{{tag}}` source. Keyboard and Edit-menu copy are correct.
- rune-kit's own metrics outside buttons, inputs and switches (D6).
- The three save-failure fixes (D5).
- No data migration: the copy from `sh.saeed.kudzu.desktop` is not built (no users).
- An unbundled binary is Dev, never production (§1.6).
- The palette, picker and form are non-activating panels, not focusable app windows (WIN-3). What the user sees must be the same; S11 and S71 check it.
- No visible change: `kzclip://` is gone; the version lives in `version.txt`; the updater checks an archive before installing it.
- FRM-4: a typed form interrupted by typing in the target app is not submitted (D9).
- A refused paste-based expansion keeps the trigger text (D14). The Tauri engine skips that check while a form is open.

---

## 7. Review resolution

`PLAN_REVIEW.md` (Codex Astra 6, 2026-11-11) raised 2 blockers, 12 major and 3 minor findings. Each was checked against the Tauri code and the Rune source at the pin.

| Finding | Verdict | What changed |
|---|---|---|
| B1 headless adapter selection | accepted | Launch mode is decided first with `rune::is_headless()`, which reads the arguments and so works before the window factory runs; the headless branch never reaches `Ports::macos`; S01 tests the four headless invocations. |
| B2 save acknowledgment, quit order | accepted | A commit and its durability are separate (`saved()`); one shutdown order; drafts are committed on every path that hides or quits; tests in S25, S40 and S51. |
| M1 library transactions, change journal | accepted | `library.transact` for agent edits, imports, restores and sync merges; reactions are state-based, so `Origin` and a journal are not needed; sync commits library and index in one job, with generations fencing stale callbacks. |
| M2 form ownership, suppression | accepted | `forms` owns the pending form, its token and its guard; panels dismiss through one function; interruption is a separate signal from suppression (`panel_focused`), pinned by tests in S39. One question for the owner in §6. |
| M3 dependency gate | accepted | `core-foundation-sys` is allowed under `kudzu-app` (it comes with `chrono`); containment is scoped to first-party files; a first-party `build.rs` is banned; S01 tests the real graph. |
| M4 steps cannot stay green | partly | Fixed: S20 owns the engine's `use` paths; `Kudzu` and `Ports::macos` moved to S40, which now waits for every service and adapter; S32 owns core's `lib.rs` and README lines; `images.rs` is pre-declared; S03 owns its workflow step; S75 adds a dispatch trigger. Rejected: a machine-readable step manifest in the repository. The step fields are regular enough for a script to read, and a second artifact would only need keeping in sync. |
| M5 cut-over before the audit | accepted | The audit (S74) now runs before the cut-over (S75), with a gate: no gap that loses data or puts text in the wrong place, and the owner's approval for every other difference. Spikes end with go, fallback or no-go. The reference is kept until the cut-over is accepted. |
| M6 release keychain and evidence | partly | Fixed: the CI keychain lifecycle is in `xtask release`; a dispatch-only workflow notarizes a real candidate before cut-over; the updater checks archives; a smoke test follows the rename. Rejected: required runs on Intel hardware and on a macOS 14 machine. The x86_64 slice runs under Rosetta and `minos` is checked; the pre-release beta that RELEASING.md already requires covers real hardware. |
| M7 release-please Rust strategy | accepted | The version moves to `version.txt` with the `simple` strategy. The `rust` strategy re-versions every workspace member (confirmed in its source). Decided now and dry-run in S01, not at cut-over. |
| M8 performance measurement | partly | Fixed: the app's own timing marks replace `RUNE_READY_DIR`; hidden-panel redraws are acknowledged and counted separately; baselines come from release builds; sample counts are stated; S39 owns the poster seam. Rejected: a separate benchmark-harness step. Four marks in `timing.rs` and Rune's headless timing output are enough. |
| M9 parity harness, determinism | partly | Fixed: `xtask parity` is built in S06; the matrix covers primitives in use; fixtures fix clock, zone, locale and UUIDs, and headless queries run inline; parity uses the system font and goldens are regression only; images compare by decoded size, not bytes; cross-window scripts use `--desktop`. Rejected: a deterministic executor for application services. Fixed inputs and inline queries remove the race without one. |
| M10 changes without owner approval | accepted | §6 records the decisions and the full list of intentional differences. One-click chip configuration is a requirement of S13 and S52. The date-example and backup-pruning changes are reverted to the Tauri behaviour (D5). |
| M11 keep-ours versus reuse | partly | Fixed: the `rmcp` rationale is corrected (the cost is tokio and a rewrite, not transport); core's hand-written Base64 gives way to the crate; `tasks` is one thread per operation, with no pool or framework. Rejected: swapping the FFI declarations for `core-graphics` and `accessibility-sys`, and the CSV reader for `csv`. Both are risk or new dependencies for no change in behaviour. `rmcp` stays out (D1). |
| M12 native safety contracts | accepted | New step S39 adds seams and protocol tests (paste and restore, held input, cancellation, image delivery, listener upgrade, form interruption) before the engine is restructured. |
| N1 privacy rule wording | accepted | Rule 9 is rewritten by sink. |
| N2 global state, `Kudzu` scope | accepted | The pasteboard record is the one named singleton, with one owner; `Kudzu` is for composition and shutdown only. |
| N3 sizes and reviewers | partly | Fixed: S25 is L and no longer builds `Kudzu`; native verification is two steps; sizes exclude copied code; a rule covers an unavailable model. Rejected: checking provider availability in the plan. That happens when a step is dispatched. |
| Feature trace table | accepted where concrete | Acceptance added for LIB-7, LIB-14, CLP-1, CLP-6, CLP-17, CLP-18, PKR-1, PKR-5, MCP-9, ACT-5, DAT-13, SET-12, SYN-5, SYN-6, WIN-2 and EDT-5; FRM-6 is back to 640; overscan counts are dropped as Rune's concern. |
| Dependency audit | accepted where concrete | Feature ownership per crate, rotation at exactly 2,000,000 bytes, an OS-held instance lock, HTTP limits, an independently signed fixture, image decode limits, window-pointer provenance. |
