# Rune fit for Kudzu

Read against Rune `dev` at `29aacb688883a9712bc27175bdafd99fc6edb421` (clone: `/private/tmp/claude-501/-Users-saeed--t3-worktrees-text-expander-t3-d91e2bfb-785bc60d361af6a7/c2025f02-8131-4d9f-956b-c62a6eb9dbb5/scratchpad/rune-src/rune`). Paths below are relative to that clone unless they start with `apps/desktop/`. GitHub issues were listed on 2026-11-11 (`gh issue list -R anomalyco/rune --state all --limit 300`): 254 issues came back, 241 open.

Claims cite a file that was read. A few example programs (`drop-files`, `drag-out`, `frosted-bar`, `glass-panel`, `kit-gallery`, `todo`, `settings`, `overlay`, `rich-clipboard`) are cited from `crates/rune/README.md` or their crate READMEs, not from a full read of each `main.rs`. Those sentences say so. Nothing here was inferred from the name of an API without opening the declaration, except where a paragraph says "inferred".

Kudzu today (`apps/desktop/README.md`) is a menu-bar text expander. ⌘Q closes its windows and leaves expansion, clipboard history, and the menu bar running. **Quit Kudzu** in the menu bar exits. The main window is a transparent overlay titlebar with a sidebar material (`apps/desktop/src-tauri/tauri.conf.json` lines 25–37) and a translucent sidebar fill so that material shows through (`apps/desktop/app/globals.css` `.vibrant`, lines 247–251). The caret form is placed from a rect the engine already has (`apps/desktop/src-tauri/src/windows.rs` `show_form`, line 280).

Tally for the table in section 2: **35 covered, 13 partial, 5 gap** (53 rows).

## 1. How a Rune app is built

### App model

A view is `fn view(s: &S, cx: &mut Cx<S>) -> El`. Handlers are queued and run between frames with `&mut S`. Hover and press retarget tracks and do not re-run the view (`crates/rune/README.md`, "What exists").

`rune::run(spec)` opens one window and quits when it closes. An app that outlives its window uses `rune::App::new(spec)` (`crates/rune/src/app.rs`):

| Builder | Line | What it does |
|---|---|---|
| `windowless` | 354 | Start with no window. Implies `keep_running`. |
| `keep_running` | 362 | Closing the window keeps the process, state, and loop. |
| `accessory` | 392 | macOS menu-bar policy: no Dock icon at launch. |
| `standard_menus` | 409 | Edit and Window menus. Never adds Quit. |
| `on_close` | 417 | Runs before every close. `tx.quit()` quits instead. |
| `on_quit` | 432 | Runs when the process is ending. |
| `on_reopen` | 459 | Dock click, Finder, Spotlight, `open`. Return true to skip the default. |
| `chrome` | 180 | `Chrome { full_size_content, traffic_lights, drag_regions }`. |
| `overlay` | 315 | Edge HUD. Non-activating, all Spaces, status window level. |

`AppHandle<S>` is `Clone + Send + Sync` and works before `run`. `update` (line 750) runs a closure on the UI thread, in call order, even with no window showing. `set_position((i32, i32))` (line 676) is physical desktop pixels. `place` (line 683) is display-local points. `set_dock_icon` (line 732) switches Regular and Accessory while running. `quit` (line 718) ends the process.

`AppSpec` is the window's title, size, `init`, `view`, fonts, and the rest. `AppSpec::new(title, size, init, view)` fills defaults (`crates/rune/README.md`, "Window chrome"). `AppSpec::fonts` is `&'static [&'static [u8]]` (`crates/rune/src/tooling.rs` line 29).

Several windows share one winit loop and one GPU device:

- `rune::run_windows`, `run_windows_with`, `run_windows_with_args` (`crates/rune/src/tooling.rs` lines 193–210). `RunOptions { accessory: true }` sets the policy before the loop starts.
- `AnyWindowSpec` mixes state types (`crates/rune/README.md` line 1813).
- `App::window` adds a titled window with its own state beside the main window.
- `WindowSpec::from_app(key, app)` keeps that window's `AppHandle`, `keep_running`, and close callback.

Keys are one namespace per window. rune-kit components key their parts (`label`, `value`, `icon`), so an app key must not collide with those (`crates/rune-kit/README.md`, component builders). `rune::memo(id, version, build)` and `memo_with_hash` (`crates/rune/src/memo.rs` lines 611 and 619) reuse a cached subtree when the version matches.

Background work does not touch `S` from its thread. It keeps an `AppHandle` and calls `update`, or publishes through `rune::Latest` (`crates/rune/README.md`, "Requests off the main thread", around line 808; the `Latest` type body was not re-read for this note). `poll` should return false when nothing visible changed, or a resting window draws anyway.

### Windows Kudzu actually needs

**Main library window.** `Chrome { full_size_content: true, traffic_lights: Some((18.0, 22.0)), .. }` matches today's overlay titlebar and light position. `Cx::titlebar_inset()` is the clearance. `.window_drag()` marks the drag band. The shell is `rune_kit::shell`: `SplitView`, `Sidebar`, `Toolbar`, `Page` (`crates/rune-kit/README.md`, "App shell", and `crates/rune-kit/src/shell/`). `shell::TabStrip` is a public reorderable tab strip (`crates/rune-kit/src/shell/tab_strip.rs`). `cargo run -p rune-kit --example shell` is the large-window pattern, including 5,000 virtual sidebar rows.

**Floating panels.** `WindowOptions::launcher_panel()` (`crates/rune/src/window/spec.rs` line 107) is transparent, borderless, hidden, non-activating, and prewarmed at alpha 0. Show takes keys without activating the app. On macOS the content view moves into an `NSPanel` whose collection behaviour is `CanJoinAllSpaces | Stationary | IgnoresCycle | FullScreenAuxiliary | CanJoinAllApplications`, at window level 3 for an ordinary panel (`crates/rune/src/platform/macos.rs` lines 175–222). `MacWindowOptions::collection_behavior` and `level` can override that (`configure_native`, lines 109–117). `examples/launcher` is the pattern: a notes window with `keep_running`, plus a launcher built `windowless` with `global_shortcut("cmd+space")` and `configured_options(WindowOptions::launcher_panel())` (module comment and first 40 lines of `examples/launcher/src/main.rs`).

`crates/rune/README.md` lines 1822–1825 say non-activating `NSPanel`s, all-Spaces policy, and status items "are not implemented here." That sentence is stale for panels. It is still true for status items. Issue #263 is the docs bug that covers this class of drift.

**Edge overlays** (`App::overlay`) are a different object: click-through or non-activating HUDs at status level. Mouse-moved events for a non-key overlay go to the key window, so the palette and the clipboard picker should be launcher panels, not overlays. `examples/overlay` is the HUD pattern (cited from the README, not re-read here).

**Theming.** `tok::theme(dark)` (`crates/rune/src/tok.rs` line 168) is the built-in OKLCH theme. `Cx::system_dark()` (`crates/rune/src/runtime/cx.rs` line 127) follows the system appearance. `rune::theme::blends_srgb` and `theme::for_blend` (`crates/rune/src/theme.rs` lines 45 and 73) resolve translucent tokens for the window's blend mode. rune-kit's `Kit::from_palette` derives a component theme from three colours (`crates/rune-kit/README.md`, "Use"). `examples/settings` is the small themed window and the headless golden (cited from `crates/rune/README.md` line 4 and the headless section).

### Performance rules

`scripts/app-audit` is a ripgrep checklist for the "Fast apps" page (`site/src/FastApps.tsx` was not opened; the script's header and checks were). It exits 0 unless `--strict`. It flags:

- `.virtualize(` that still builds every child ("virtualize over built rows"). Use `El::virtual_rows` (`crates/rune/src/el/virtual_rows.rs` line 80). `virtual_rows_version` skips the key walk when the row set is unchanged. `on_visible_rows` returning false is bookkeeping and does not re-run the view. `memo` inside the row builder keeps the subtree while the row stays mounted.
- Blocking calls near `view`, `poll`, `init`, or a handler (`Command::new`, `rusqlite`, `fs::read_to_string`, `reqwest::blocking`).
- Unconditional `wake_after` of 33 ms or less.
- Reading a layout rect only for its width or height.
- A static `HashMap` with no eviction in the file.
- `window_layout` (re-runs on scroll frames).
- `on_layout` / `on_hover` without `live_only`.
- `scroll_to_instant` stepped by `wake_after`.

Also from the README ("Launching fast", "Smooth scrolling"): call `rune::prewarm_gpu()` first in `main`; defer indexing and secondary windows to `on_ready`; paint hover with `El::hover` and `hover_group`, not state set from `on_hover_change`; return false from `poll` when idle; hold the first second still (`tx.instant()` for data that arrives during it). `RUNE_COMMIT_BUDGET_MS` warns when a commit overruns a frame (named in the core README's performance notes; the env var's reader was not re-opened for this note).

### Headless mode and tests

Every app accepts the same flags (`crates/rune/README.md`, "Agent tooling", from line 2422):

- `--snap out.png` renders one PNG. `--theme`, `--size`, `--scale`, `--at`.
- `--tree tree.json` dumps key, path, role, label, text, rect, opacity, colours.
- `--script` drives `move`, `down`, `up`, `tap`, `drag`, keys, `type`, wheel, `{"reopen": true}`, `{"open_url": ...}`, `menu-pick`, `file-dialog`, `drop_files`.
- `--frames`, `--video` (ffmpeg), `--window KEY` for one window of a multi-window app.
- `--desktop backdrop.png` composites every visible window. `examples/launcher` is the documented recording.

The clock is virtual. `Runtime::accessibility_json` is the same tree for tests. `rune::motion_gate` fails a test on motion a person would notice. rune-kit's `tests/a11y` goldens cover each component (`crates/rune-kit/README.md`, "Accessibility").

### macOS bundle and relaunch

`scripts/mac-app` installs `~/Applications/NAME.app` and swaps a rebuilt bundle in without the window blinking (`crates/rune/README.md` line 889). `MAC_APP_SIGN` keeps a stable identity so TCC answers survive rebuilds; ad-hoc signing is a new app every build. The app side is `rune-mac` (`crates/rune-mac/src/lib.rs`): `launch()` first in `main`, `remember_frame(name)` from `poll` until it returns true, `saved_size(name)` for the launch size, `busy`, `toggle_full_screen`. `remember_frame` writes the same autosave name onto every `NSWindow` (lines 15–28).

### Depending on it, and building it

```toml
rune = { git = "https://github.com/anomalyco/rune", rev = "29aacb688883a9712bc27175bdafd99fc6edb421" }
```

Pin a rev of `dev`. `dev` is never rewritten. There are no releases (`README.md` lines 49–55). The project does not take pull requests; file an issue.

Toolchain: Rust 1.99.1 (`rust-toolchain.toml`). Linux builds need `libxkbcommon-dev` and `libwayland-dev` (`crates/rune/README.md`, "Linux"). Kudzu is macOS-only, so that is for CI if the UI crate is built on Linux. The default GPU backend on macOS is Rune's Metal backend (`metal-backend`).

### Recommended shape for this app

Follow `examples/launcher` plus `rune-kit`'s shell example, not a single `rune::run`:

1. One `App` for the library, `accessory()` + `keep_running()` + `standard_menus()` + `chrome` for the overlay titlebar. `on_reopen` opens it. `on_quit` is the real exit, called from the tray's Quit.
2. One `App` per floating panel (palette, clipboard, caret form), each `windowless` and `WindowOptions::launcher_panel()`, shown with `AppHandle::show` / `set_position`.
3. The permission companion is an ordinary `App::window` (small, titled or borderless). Inferred: nothing in Rune is specific to that screen.
4. Snippet rows go through `Sidebar::virtual_rows` or `El::virtual_rows`, keyed by snippet id, with `memo` inside the row.
5. A thin module owns `App`, `WindowSpec`, `AppHandle`, and the objc2 calls below, so a Rune rev bump edits one file.

## 2. Fit table

Status is `covered` (the named API does this on macOS), `partial` (the API does part of it), or `gap` (there is no Rune API; the cell says what to build). "Upstream?" means worth an issue on `anomalyco/rune`. They ask for issues, not pull requests.

| # | Kudzu need | Rune API | Status | Workaround, and whether to file an issue |
|---|---|---|---|---|
| 1 | Main window, full-size transparent titlebar, traffic lights | `App::chrome` / `WindowSpec::chrome` (`Chrome { full_size_content, traffic_lights }`), `Cx::titlebar_inset`, `El::window_drag`, `rune_kit::shell::{SplitView, Sidebar, Toolbar, Page}` | covered | Lights at `(18, 22)` match `tauri.conf.json`. |
| 2 | Sidebar vibrancy (`NSVisualEffectMaterial` sidebar, under the window, active-state aware) | `El::system_material`, `SystemMaterial::{Glass, Popover}` (`crates/rune/src/system_material/mod.rs` lines 18–24). `MAX_NATIVE_MATERIALS = 4`. | partial | `Popover` is `NSVisualEffectView` popover, behind window. `Glass` is `NSGlassEffectView` on macOS 26, otherwise the same popover material. There is no Sidebar material. Put `Popover` on the sidebar rect in a transparent window, and keep the sidebar fill translucent the way `.vibrant` does today. Worth a small issue: a `Sidebar` variant, and active vs inactive. |
| 3 | Floating non-activating panels on all Spaces (palette, clipboard picker) | `WindowOptions::launcher_panel`, `MacWindowOptions::nonactivating_panel`, panel collection behaviour in `platform/macos.rs` `Panel::adopt_kind` | covered | README lines 1822–1825 contradict the source. Issue #335: panels take about 280 ms to appear. Prewarm at launch and `show` a hidden panel. |
| 4 | Fill-in form at the text caret | Same panel, plus `AppHandle::set_position` / `place` (`app.rs` lines 676–685) | partial | Rune has no accessibility caret-rect API. Kudzu's engine already passes a `Rect` into `show_form`. Convert that rect to physical pixels and `set_position`. Not worth an issue while the engine owns the caret. |
| 5 | Permission companion window | `App::window` (`crates/rune/src/app.rs` line 332) | covered | Inferred: a normal small window is the whole framework need. The draggable app-icon onboarding is app UI. |
| 6 | Window frame persistence | `rune_mac::remember_frame(name)`, `saved_size(name)` (`crates/rune-mac/src/lib.rs` lines 15–28) | partial | One name is written onto every `NSWindow`. Safe if only the library window is remembered and panels are sized in `WindowOptions`. Per-window names need `setFrameAutosaveName:` via objc2, called when that window is the only one, or a Rune API that takes a window. Worth a small issue. |
| 7 | Accessory (menu-bar) app, Dock icon toggle | `App::accessory`, `AppHandle::set_dock_icon` (`app.rs` lines 392, 732). Policy read in `platform/reopen.rs` `accessory()` line 120. | covered | Showing the Dock switches to `NSApplicationActivationPolicy::Regular`. |
| 8 | Reopen (Finder, Spotlight, Dock, `open`) | `App::on_reopen`. Default restore is skipped while the policy is not Regular (`window/app_events.rs` lines 32–43). Hooks still run. | covered | While the Dock icon is hidden, open the library from the tray. When the Dock icon is showing, the default restore runs. Launcher panels are not reopen targets (`window/any_host.rs`). |
| 9 | ⌘Q closes windows and leaves the menu-bar process | `applicationShouldTerminate:` in `crates/rune/src/quit/mac.rs` always returns `NSTerminateNow` (line 31). `standard_menus` does not add Quit. `AppHandle::quit` is the real exit. | gap | Once the Dock icon is visible, winit's app menu Quit terminates the process. Rune skips installing the method if the delegate already answers `applicationShouldTerminate:` (lines 49–52), so an app that hooks the delegate first can return cancel, close windows, and stay running. Worth an issue: `on_quit` should be able to return cancel. |
| 10 | Menu-bar status item and menu | `rune::Tray` (`crates/rune/src/tray.rs` lines 1–9, 90–98). macOS returns `TrayError::Unsupported`. Linux (StatusNotifier) and Windows (`Shell_NotifyIcon`) only. `TrayItem` is a flat action or separator. | gap | objc2 `NSStatusItem`, and call `AppHandle::update` from its callbacks. The README says the app owns the tray (`crates/rune/README.md` line 980). Issue #141 is already that request. A submenu-capable menu is part of the same gap. Filing a duplicate is not useful; comment on #141 if Kudzu needs submenus and a template image. |
| 11 | System-wide hotkeys while other apps are focused | `App::global_shortcut`, `AppHandle::register_shortcut` (`crates/rune/src/shortcuts/mod.rs` lines 110–131). macOS uses Carbon `RegisterEventHotKey` (`shortcuts/macos.rs` line 67). | partial | Key codes are physical ANSI positions (`shortcuts/macos.rs` lines 201–204, `shortcuts/keys.rs` "US layout"). On a non-US keyboard the letter in the shortcut string is the US key, not the character printed on the key. `ShortcutRecorder` records a shortcut the same way. Worth an issue only if Kudzu's recorder must bind by produced character. |
| 12 | Single instance | `rune::single_instance` (`crates/rune/src/open_url/instance.rs` lines 27–38) | covered | On macOS it always returns true. Launch Services already delivers the second `open` to the running app, which hits `on_reopen` / `on_open_url`. The Windows named pipe is irrelevant here. |
| 13 | Panel vibrancy (popover material behind the window) | `SystemMaterial::Popover` and `::Glass`, `El::system_material` (`el/backdrop.rs` line 14). `srgb_blend(true)`, `El::continuous_corners`, `Shadow::core_animation`. | covered | This is the palette and form material Kudzu uses as Tauri `Effect::Popover`. Headless snapshots draw the fill, not the native view. At most four native materials per window (issue #297). |
| 14 | Backdrop blur of the app's own pixels | `El::backdrop_blur(sigma)` (`el/backdrop.rs` line 34). `MAX_BACKDROP_BLURS` is 4 (README line 2342). | covered | `examples/frosted-bar` (cited from the README, not opened). `El::blur` blurs the element's own subtree. |
| 15 | Linear and radial gradients | `El::linear_gradient`, `El::radial_gradient`, `El::gradient`. Stop list capped at `MAX_GRADIENT_STOPS = 16` (`el/visuals.rs` line 602, `pub(crate)`). | covered | The README's public examples say 8 stops. The constant allows 16. |
| 16 | Outer and inset shadows | `Shadow { x, y, blur, spread, color, inset }` (`el/visuals.rs` lines 569–577), `El::shadow`, `El::box_shadow`. | covered | `blur` is CSS `2σ`. A Core Animation `shadowRadius` of `r` is passed as `blur = 2r`, or `Shadow::core_animation`. Issue #377: a translucent fill shows its own outer shadow through itself. |
| 17 | Noise / grain texture | `rune_fx::Fx::grain` (`crates/rune-fx/src/lib.rs` line 419) | partial | Not a core paint primitive. It needs the compositor hook (`crates/rune/README.md`, "Compositor hook", around line 2086; the hook's trait was not re-read). A tiled image in `ImageCache` is the cheaper stand-in. An issue is optional; rune-fx already has grain. |
| 18 | Opacity and blur transitions | `El::opacity`, isolated group opacity, `Transition::blur_replace`, `El::sequence` (`Beat` opacity). | covered | README "Isolated opacity" and "Presence". |
| 19 | Springs | `Spring::new(duration, bounce)`, `El::motion`, `tok::motion` (`el/motion.rs` line 89). Layout animates per edge. | covered | `examples/todo` is the row-spring pattern (cited from the README, not re-opened here). |
| 20 | Keyframe sequences: rise, pop, check, bar growth, shimmer, rotation | `El::sequence` / `Beat` (`rune-motion/src/sequence.rs`: `opacity` and `scale` only). `El::rotate` and `El::rotate_motion` (`el/visuals.rs` lines 381–389). `El::looping` / `Cycle` for shimmer. Bar growth is a height spring. | covered | A star burst is `rotate` plus `rotate_motion`, not a `Beat` channel. Issue #360 asks for looping scale and shadow keyframes. |
| 21 | Staggered list entry | `El::enter_delay` (`el/enter_delay.rs` line 36), `rune_motion::Stagger` (`rune-motion/src/choreo.rs` line 26), `Sequence::delay`. | covered | Apply `enter_delay` from `Stagger::delay(index, count)` when a row is inserted. |
| 22 | Press and hover feedback | `El::press_scale`, `El::hover`, `hover_group`, `group_opacity`, `group_color`. rune-kit `State::Hover` / `Pressed` and `Kit::interactive`. | covered | Do not store hover in app state (app-audit flags it). Issue #205: hover repaints on every move inside the same element. |
| 23 | Layout animation of list rows | Per-edge springs and growth stacks (README "Animation"). `SidebarRow::reorder` animates neighbours aside. | covered | Virtual rows must be keyed by id. A commit during a scroll is the app's fault (README "Smooth scrolling"). |
| 24 | Button, switch, checkbox, radio, segmented control, select, combobox, command palette, menus with submenus, popover, tooltip, toast, dialog, tabs, badge, shortcut display, shortcut recorder, accordion, collapsible, card | `rune_kit::{Button, IconButton, Switch, Checkbox, RadioGroup, SegmentedControl, Select, Combobox, PopupList, Menu, Popover, Tooltip, HoverCard, Toast, ToastRegion, Dialog, Badge, Keybind, ShortcutRecorder, Accordion, Collapsible, Card}`. Tabs: `shell::TabStrip`. Command list: `PopupList` (the combobox's list; slash and mention menus use it). | covered | `Menu` has submenus and `context_menu`. `Dialog` is the alert surface (`dialog.rs`). `ShortcutRecorder` is `shortcut_recorder.rs`. No separate `AlertDialog` type; `Dialog` is the one. Issue #553 (`Keybind` uppercases "g"), #397 (segment icons), #290 (checkbox indeterminate draws a check). |
| 25 | Scroll bars | `scroll_bar` (`crates/rune-kit/src/scroll_bar.rs` line 775) and `Kit::scroll_bar` | partial | The kit README still says scroll bars are not drawn. The function is public. Issue #365 asks for an always-visible thin bar. Issue #552: two viewports that share a key share one bar. Give each scroller its own key. |
| 26 | Breadcrumbs | None | partial | A row of buttons. Not worth an issue. |
| 27 | Tree of nested groups | `rune_kit::Tree`, `TreeModel`, `FileTree` (`crates/rune-kit/README.md`, "Tree and Table") | covered | Virtualised, multi-select, type-to-jump, sticky behaviour is the table header's, not the tree's. Drag into a group is row 30. |
| 28 | Virtualised list of thousands of rows: multi-select, keyboard, type-to-filter, sticky headers | `El::virtual_rows`. `Table` (virtualised, `multi_select`, sticky header, type-ahead via the shared list keys). `Sidebar::virtual_rows` with `SidebarHeader` rows for sections. Filtering is app state: pass the filtered slice. | covered | Do not use `rune_kit::List` for the snippet library. Its README path builds the rows it is given (first part of `list.rs` was read in the earlier pass; it is not the virtual primitive). Issues #203, #546, #549, #550 are correctness and cost bugs on this primitive. See risks. |
| 29 | Drag to reorder rows | `SidebarRow::reorder` (kit README lines 224–226). `TabStrip` reorder. | covered | Works inside `virtual_rows`. Same-list only. |
| 30 | Drag a row into another group | No tree drop target. `El::on_drag` exists. Issue #357: drop targets inside the app for non-file payloads. | gap | Own the gesture: `on_drag` plus a hit test against visible group headers, then mutate the model. File an issue only to support #357, which is already open. |
| 31 | Text fields | `TextInput`, `TextArea`, `TextField`, `Field` (`rune-kit`). IME through winit, `App::on_ime`, `.ime_cursor()` (core README "Text fields"). | covered | Issue #530: these widgets drop `on_secondary_press` and `description` meant for the field. Issue #180: blur notifies every field of the last view. |
| 32 | Rich template editor: atomic chips, slash menu, IME, grouped undo, bold / italic / underline / links / lists | `TextEdit::rich()`, `Mark::{Bold, Italic, Underline, Strike, Code, Link, Pill}` (`edit/model.rs` lines 14–27), `insert_pill`, `TextEdit::undo_group` (`edit/history.rs` line 259), blocks `Bullet` / `Numbered` / `Quote` / `Code` (`MAX_LIST_LEVEL = 3`). Slash menu: `PopupList` anchored to the caret. `El::pill_style` / `pill_colors` / `pill_icon`. | partial | Pills are the chips. The icon is an `Icon`, not a bitmap. Issue #548: a click on a link in a field never opens it. Issue #128: no press handler on a pill. Issue #125: a link mark takes presses only on the link. Grouped undo and IME are present. Inline images are row 33. |
| 33 | Inline images in that editor | `Mark` has no image variant. The enum is `non_exhaustive` ("more marks may come"). Issue #350 asks for inline images. | gap | Keep images out of the text model (a pill whose id the app resolves to a thumbnail beside the field), or wait on #350. Worth commenting on #350 with the template-chip case. `El::pill_icon` cannot draw a bitmap. |
| 34 | Read-only rich text | Selectable text (`El` selectable, `selection_color`), `rune_kit::Markdown` | partial | Issue #536: read-only text has no caret, so no Shift-arrow selection, Home/End by line, or word steps. Fine for a preview. A selection-heavy preview needs a read-only `TextEdit`. |
| 35 | Code with line numbers and syntax highlighting | `rune_kit::syntax::highlight`, `Markdown` code fences, `DiffView` line-number gutters (`diff/view/line_info.rs`) | partial | Highlighting is covered. Line numbers exist on `DiffView`, not on a markdown fence or a plain highlighted run. Draw a gutter beside selectable highlighted text. Not worth an issue unless snippets must be a `DiffView`. |
| 36 | Truncation and match highlighting in search results | Text truncation on `El` / table cells (truncated cells get a tooltip). `TreeItem::highlight`, `Markdown::highlights`, span backgrounds. | covered | The search index stays in the app. Rune paints the spans. |
| 37 | SF Pro, Figtree, Paper Mono, variable fonts | `Family::{Sans, Mono, Named, SystemMono}` (`tok.rs` lines 26–38). `Family::Named("System Font")` loads SFNS, which is San Francisco (`text/pixel_compare.rs` lines 55–64, `text/system_fonts.rs`). `TextStyle::variations` and `features` (`tok.rs` lines 111–114). `AppSpec::fonts` registers bytes. | covered | The default `Sans` slot is the font the app registers (Inter, unless replaced). Use `Named("System Font")` for SF Pro and `Named("Figtree")` / `Named("Paper Mono")` after registering the files. `SystemMono` is SF Mono, not SF Pro. Prefer bundling Figtree and Paper Mono: issue #226 (load an installed family, or add fonts while running) is still open, and `Family::Named`'s comment says an installed family loads on first use. Those two sentences disagree; bundling avoids the question. |
| 38 | Font smoothing like `-webkit-font-smoothing: antialiased` | `FontSmoothing::Off` (`text/coretext_match.rs`). `App::font_smoothing`, `Tx::set_font_smoothing`. | covered | README lines 109–112: `Off` is Core Text with font smoothing off, the antialiased browser path. `On` is the heavier macOS default. Kudzu should set `Off`. |
| 39 | PNG, JPEG, WebP | `ImageCache::load` (`image.rs` line 183) | covered | Animated WebP is decoded (image module docs from the earlier pass). SVG and PNG-backed ICNS too. |
| 40 | App icons from NSWorkspace, Finder file icons | `ImageCache::app_icon` / `app_icon_scaled` (`image.rs` lines 244–267): "The Finder icon for an app bundle or any file path." | covered | `rune_kit::file_icon` is a generic glyph by extension, not a Finder icon. Use `app_icon` for both apps and files. Missing paths still get the generic document icon on macOS. |
| 41 | Colour swatches | `El::fill` / a small rounded rect | covered | No widget. A filled element is the swatch. |
| 42 | Charts and bars for insights | No chart widget. `rune-mermaid` has `xychart` and `pie` (`crates/rune-mermaid/src/xychart.rs`, `pie.rs`) as diagram syntax. | gap | Draw bars with `el().h(value)` and a spring, as `examples/todo` does for progress. Mermaid is the wrong tool for a live insights page. Not worth an issue. |
| 43 | File drop into the window | `El::on_drop_files` (`el/drop.rs` line 63) | covered | Issue #532: the handler cannot see app state or the whole drag, and an ancestor cannot follow it. Enough for "drop a file to import." `examples/drop-files` is cited from the README, not opened. |
| 44 | Drag a file or image out to other apps | `El::drag_out` (`el/handlers.rs` line 65). macOS pasteboard types are written in `platform/macos/drop.rs` (file was located, the write path was not re-read line by line). | covered | README: macOS only. Linux is issue #526. `examples/drag-out` was not opened. |
| 45 | Open and save dialogs | `Tx::open_file`, `Tx::save_file`, `Tx::open_folder` (`file_dialog.rs` lines 249–254), feature `native-file-dialog` | partial | One path per call. Issue #367 is multi-select. Kudzu's import step picks export files (`apps/desktop/README.md`); if that is several files at once, loop the dialog or use an NSOpenPanel via objc2 until #367 lands. Issue #147: `open_url` refuses `file:` links, so Reveal in Finder is objc2 `NSWorkspace.activateFileViewerSelecting`. Worth commenting on #147 if the clipboard history reveals files. |
| 46 | Open a URL | `Tx::open_url` (`el/tx.rs` line 307), `rune::open_url` via NSWorkspace | covered | `http` and `https`. Not `file:` (row 45). |
| 47 | Notifications | `rune_notify::{register, post, on_click, available}` (`crates/rune-notify/README.md`). macOS is `UNUserNotificationCenter`. `rune_mac::notify` re-exports it. | covered | Silent. The first post asks permission. A bundled app is required (`available()`). |
| 48 | Clipboard read and write: text, HTML, RTF, images, files | `Tx::copy_rich`, `ClipboardContent::{with_html, with_rtf, with_files, with_image}`, `tx.read_clipboard`. macOS write of PNG and TIFF: `clipboard/macos.rs` lines 30–47 and the round-trip test at line 263. File items are written on that path too (earlier read of `macos.rs`). | covered | `examples/rich-clipboard` writes text plus HTML plus RTF; pasting into its field is still plain text (example header from the earlier pass). Image write on macOS was confirmed in `clipboard/macos.rs`. |
| 49 | Clipboard change detection | `revision()` in `clipboard/native.rs` lines 168–170 reads `NSPasteboard.changeCount()` and is private. Used for secret expiry, not exposed. | partial | Keep the engine's existing poll of `changeCount` (objc2, about every 300 ms). A public `on_clipboard_change` would be a small issue. Not blocking if the engine stays on objc2. |
| 50 | Accessibility | AccessKit (`crates/rune/README.md` "Accessibility", from line 2351). `El::role`, `label`, `focus_ring`. macOS is NSAccessibility. | covered | The tree is built when a client asks. `--tree` and `Runtime::accessibility_action` drive tests. Issue #111 is a WCAG audit goal, not a missing API. Issue #537: an assistive click on a list option does nothing when the container handles the input. |
| 51 | Focus rings and keyboard-only use | Focusable controls, kit focus rings, sidebar as one tab stop (arrows, Home/End, type-ahead, Return). | covered | Issue #363: the ring cannot set its own corner radius. Issue #173: no `Tx::focus(key)` to move focus from code. Both are polish. |
| 52 | Appearance follows system light and dark | `Cx::system_dark()`. `examples/settings` switches Auto / Light / Dark (cited from the kit and core READMEs). | covered | Rebuild the `Kit` when `system_dark()` changes. `theme::for_blend` if the window is not sRGB-blending. |
| 53 | Localised dates | `rune-i18n` (`crates/rune-i18n/src/lib.rs`): catalogs, plurals, grouped and compact numbers, lists, `system_languages`. No date or calendar type (no match for date, datetime, or `icu_calendar` in that crate). | partial | Format dates in Kudzu's existing ICU code and pass strings in. Numbers and plurals can move to `rune-i18n`. A date API is a large issue and not worth it while ICU stays. |

## 3. Existing upstream issues

Relevant to the rows above. State is what `gh` returned on 2026-11-11. All of these were `OPEN`.

**Shell and windows**

| Issue | Title |
|---|---|
| 141 | macOS: `rune::Tray` has no backend, so a menu-bar app needs a second tray crate |
| 146 | Displays: the list is a startup snapshot, with no change event and no way to add a window later |
| 147 | Desktop: no way to open a local folder or reveal a file, because `Tx::open_url` refuses `file:` links |
| 157 | macOS: receive files the system opens with the app (open-documents Apple Event), like `on_open_url` |
| 161 | macOS: a hidden window keeps its full-size drawables, textures and buffers |
| 162 | macOS: `release_gpu_when_closed` leaks one window-size surface on every hide and show |
| 164 | macOS: a window's first frame blocks the main thread until the GPU finishes it |
| 174 | Let `on_close` cancel a window close |
| 201 | macOS: an app can't replace the app menu's standard About item |
| 204 | macOS: scrolling presents on every other refresh of a 120 Hz display while the main thread and GPU are mostly idle |
| 205 | Hover repaints the window on every pointer move, including moves that stay inside the same hovered element |
| 305 | Tray: no way to update the icon or title while running, or to find the icon on screen |
| 314 | Represented file and edited state for macOS windows |
| 318 | macOS: app icons that follow the Appearance |
| 335 | macOS: launcher panels take about 280 ms to appear because of AppKit's default animation |
| 367 | File dialog: pick several files at once |
| 369 | Window: read and observe the main window's full-screen state |
| 388 | macOS: no way to decline first mouse (`acceptsFirstMouse:`) |
| 500 | macOS: moving a window to a display with another scale animates, then shows one frame at the wrong size |
| 540 | macOS: hover flickers brightness on ProMotion/Adaptive-Sync displays, and `MetalSurface::configure` drops the layer's sRGB tag |

No open issue asks for `on_quit` to cancel ⌘Q. #174 is window close, not process quit. No open issue asks for `NSVisualEffectMaterialSidebar`.

**Text, fonts, clipboard, drag**

| Issue | Title |
|---|---|
| 67 | macOS Japanese text falls back to PingFang HK instead of Hiragino Sans |
| 125 | Text field with a `Mark::Link` takes presses only on the link |
| 127 | Text fields: hide a byte range (concealed text) without removing it from the model |
| 128 | Text fields: a press handler for `Mark::Pill` chips |
| 180 | Text fields: a blur calls `on_edit` for every field of the last view, also one the app just removed |
| 209 | macOS: text in a script with no fallback list draws nothing, though the system has a font for it |
| 226 | Load installed fonts by family name, or add fonts while the app runs |
| 350 | Text: inline images as part of a text run (custom emoji) |
| 351 | Text: span boxes with their own padding and corner radii |
| 353 | Text: hit-test a point to a byte offset, and a text field's selection rects |
| 354 | Text fields: expose whether undo / redo are available |
| 357 | Drag and drop: drop targets inside the app (non-file payloads) |
| 363 | Focus ring: its own corner radius |
| 532 | File drops: acceptance can't see app state or the whole drag, an ancestor can't follow it, and nothing says which action it gets |
| 534 | A text input ignores `role(..)` |
| 535 | Colour-only span changes, and every keystroke in a multi-line field, reshape the whole text |
| 536 | Read-only text has no caret |
| 548 | A click on a link in a text field never opens it (fix on a branch) |

No issue mentions a public pasteboard `changeCount` watch. Linux clipboard work is #523 and does not apply.

**Lists, scroll, kit**

| Issue | Title |
|---|---|
| 149 | rune-kit: no Slider component |
| 173 | Add `Tx::focus(key)` to focus an element from code |
| 203 | `virtual_rows`: changing the row count re-reads and copies every row's key, so appending to or filtering a long list costs O(rows) a frame |
| 290 | Checkbox: indeterminate state draws a checkmark instead of a dash |
| 297 | `MAX_NATIVE_MATERIALS = 4`: make the limit configurable, and say when an element falls back |
| 331 | Table: let callers build on each row element (e.g. right-click menus) |
| 360 | Motion: looping keyframes for scale and shadow, a brightness filter, and a pressed offset |
| 364 | Virtual list: shift a running glide's target with the anchor, and anchor on a row the app marks |
| 365 | rune-kit: always-visible thin scroll bar with track inset and minimum thumb length |
| 377 | Paint: a translucent box shows its own outer box shadow through its fill |
| 395 | rune-kit: `MenuItem` has no trailing element or highlight callback |
| 396 | rune-kit: `SidebarToggle` label and tooltip text are fixed English |
| 397 | rune-kit: `SegmentedControl` segments cannot carry an icon, trailing element or tooltip |
| 537 | An assistive client's click on a list option does nothing when the container handles input |
| 544 | One `live_text` anywhere makes every virtual-window shift run the view |
| 546 | A scroll request that moves a virtual list past its mounted rows commits several times and runs the view |
| 547 | `scroll_into_view_in(scope, key)` also scrolls a sideways viewport outside its scope |
| 549 | `virtual_rows`: `reveal_selected` does nothing for rows built by index, and `scroll_key_to` places only a row already laid out |
| 550 | A selection across unbuilt virtual rows is cut at 10,000 rows or 4 MiB, and nothing says so |
| 551 | An element with `on_drag` and `on_press` hears a press after its own drag past the threshold |
| 552 | rune-kit `ScrollBar`: two viewports with the same key share a bar's state |
| 553 | rune-kit `Keybind` draws "g" as "G" |

**Docs drift.** #263: PLAN, the READMEs, rustdoc, and the site describe things the code does not do. The "NSPanels are not implemented" paragraph is one of those.

## 4. Risks

Rune is pre-release. The public API moves, and the README can lag the source (the panel paragraph, the kit README's "no scroll bars", the gradient stop count). Containment:

- Pin `rev = "29aacb688883a9712bc27175bdafd99fc6edb421"` and bump on purpose. Read `CHANGELOG.md` at the bump. Do not track `dev` as a branch.
- One adapter module owns `App`, `WindowSpec`, `AppHandle`, `SystemMaterial`, and the objc2 escapes (status item, terminate veto, pasteboard generation, per-window frame autosave, Reveal in Finder). The rest of the UI calls the adapter.
- Golden tests per window: `--snap` and `--tree` for the library, the palette, the clipboard picker, and the caret form, plus `motion_gate` on the row springs. Native materials do not appear in headless pixels; assert the element tree, and check vibrancy on a Mac.
- Run `scripts/app-audit --strict` on the UI crate.

What is fragile for this app in particular:

- **Virtual lists.** The snippet library and the clipboard history are the hot path. `virtual_rows` is the right API and it is fast when the view does not run during scroll, but #203 makes a filter O(rows), #546 runs the view when a scroll jumps past mounted rows, #549 breaks reveal of an unbuilt row, and #550 silently cuts a selection at 10,000 rows or 4 MiB. Key rows by id, filter by replacing the slice and calling `virtual_rows_version`, and do not call `scroll_to` in a way that commits. Re-test these four issues at every rev bump.
- **Launcher-panel latency (#335).** About 280 ms the first time a panel appears. Prewarm the palette and the clipboard picker at launch (`launcher_panel` already builds a hidden panel at alpha 0) and only `show` them.
- **Hidden windows and the GPU (#161, #162, #164).** Panels that stay hidden still matter. Measure footprint with the library closed and the panels prewarmed. `release_gpu_when_closed` currently leaks a surface on each hide/show.
- **ProMotion (#204, #540).** Scrolling can present on every other 120 Hz tick, and hover can flicker. Kudzu's lists will show this. It is upstream, not an app bug.
- **First mouse (#388).** A click on a non-key panel both activates the window and is delivered. Kudzu's panels want the click. Confirm that on a launcher panel, whose key policy is OnDemand (`setBecomesKeyOnlyIfNeeded` is false in that case, `macos.rs` line 197).
- **⌘Q and the Dock toggle interact.** With the Dock hidden there is no app-menu Quit, so the terminate gap is dormant. Turning the Dock icon on (row 7) exposes row 9. The terminate hook has to be in place before that toggle, and it has to be installed before Rune's `install()` sees a delegate that does not yet answer the selector.
- **Materials are not in goldens.** `system_material` draws a flat fill headlessly. A wrong material still passes `--snap`. Check the sidebar and the palette on macOS 15 and on macOS 26, because `Glass` changes implementation there.
- **Global shortcuts are physical keys.** A user on a non-US layout can record a shortcut that fires on a different character than the one they see. Surface the recorded key in `Keybind` and test on at least one non-US layout.
- **Clipboard images and files are covered on the macOS writer, but change detection is not a Rune feature.** Leaving the watch in the engine avoids a Rune dependency for the one thing the engine already does well.

The rewrite can keep Kudzu's structure. The main window, the panels, the component set, the springs, the gradients, the shadows, and the rich-text marks all have a Rune or rune-kit API. The pieces that need AppKit beside Rune are the status item, the ⌘Q veto, the pasteboard generation counter, and (if the popover material is not close enough) the sidebar visual-effect view.
