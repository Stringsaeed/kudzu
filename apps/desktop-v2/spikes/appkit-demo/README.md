# S38 AppKit adapters and live-check hook

The production crate is `crates/kudzu-appkit`. This demo is opt-in: a normal invocation,
`cargo test`, and headless Kudzu gates create no native demo window or status item.

```sh
cargo run -p appkit-demo
cargo run -p appkit-demo -- --live --bundle 'target/bundle/Kudzu Next.app'
```

The second command is for S71's live session. It starts accessory/windowless: use the Dev
status item to open main or the launcher. Pass the actual Dev bundle for the icon and
drag payload; the demo does not manufacture a signed identity or modify permissions.
It does not open System Settings. Close every demo window by Close Windows, and finish
with Quit Kudzu Next in the status menu. Quit drops the native owners.

Passing `--bundle` selects the icon and drag payload; this process still runs unbundled.
Use the **bundled Kudzu Next Dev app** for Finder/`open` reopen and a real drag that grants a
permission. The unbundled demo cannot prove Launch Services identity or a TCC grant.

## Public API

- `status_item::{StatusItem::new, set_title, set_enabled, set_tooltip, set_shortcut}`: native menu items addressed by `ItemId`, with `Entry::{Item, Separator}` and per-item `Rc<dyn Fn()>` callbacks; auto-enable is off.
- `StatusItem::{set_status_title, set_status_tooltip, set_glyph}`: change the button and its template image; Drop removes the status item and clears item targets.
- `status_item::{Glyph, GlyphKind, Glyph::pixels}`: four committed variants, optional update dot, one 18 pt image with 18/36/54 px representations.
- `status_item::Shortcut::{command, text, closes_windows}`: one key/modifier description for menu equivalents, hint text and S40's launcher insurance.
- `app_menu::AppMenu::{install, patch}`: early Close Windows ⌘Q, then idempotent post-promotion repair; returns `PatchState` and leaves native Quit on the responder chain without a key.
- `appearance::{Preference::resolve_dark, apply}`: preference is the theme source; only System evaluates the supplied system-theme closure. Apply that same preference to `NSApp.appearance`.
- `screens::{snapshot, pointer, under_pointer, screen_at}`: uncached display frames, visible frames, scales and pointer placement.
- `screens::{for_settings, settings_screen, canvas, union_frames}`: Settings-midpoint selection with companion/main-screen fallback, and the union of all display frames.
- `settings_window::{find, records, pick}`: pid/layer/bounds-only on-screen metadata, with the 400×300 minimum and CG-to-AppKit conversion; records retain front-to-back order for S66/S71's ordering check.
- `settings_window::{Pane::url, companion_frame, reduce_motion, screen_recording_access}`: both privacy pane URLs, exact 18 pt / 12 pt placement, Reduce Motion and a read-only recording preflight.
- `sidebar_material::SidebarMaterial::{new, sync, remove}` and `material_frame`: one hit-test-nil, behind-window Sidebar material following window activity; safe idempotent sync/reparent/remove.
- `guide_panel::GuidePanel::{new, show, hide}`: default borderless nonactivating, click-through panel, level 3, all Spaces/full-screen auxiliary, clear/no shadow, persistent while inactive; solid outline, dashed curve and `NSWorkspace.iconForFile` ghost on the companion/Settings union expanded 80 pt and clipped to the supplied Settings-screen frame. `canvas` exposes the same pure calculation used by `show`.
- `guide_panel::Lifecycle::{begin, poll, started, stop, is_current}`: monotonic milliseconds, opaque `Generation`, `Observation` input and `Step` output. Delay 450 ms, first attempt plus 12 retries at 200 ms, `Placing` with no wake and a 200 ms placement timeout, 30 Hz drawing, 2.4 s cycles / elapsed > 4.8 s stop, cancellation and stale-generation rejection. `begin(now_ms)` no longer captures a companion frame.
- `window::{with_raw_view, with_raw_window}`: the only unsafe boundary; borrow a live raw handle *inside Rune's callback*. `WindowRef::{frame, screen, rect_to_screen, set_origin}` are safe; source coordinates use Rune's exact borrowed `ns_view`, `set_origin` returns a Result, and the reference cannot escape (compile-fail doctest). The NSWindow-only helper returns None if it has no content view.
- `geometry::{Point, Rect, Screen}`: point-space values used by the adapters, including midpoint/containment/intersection/union.

`panel_tuning` stays documented and empty: S11 applied no animation fallback. No Rune or
Kudzu crate is a dependency of the production crate. The demo may depend on Rune and the
existing theme window spec; those dependencies do not flow back into the adapter.

## Integration contracts for S40 / S42 / S66

Install AppMenu once in the first update after `.standard_menus()` has made the root.
Do this before requesting the first Dock promotion, including a hidden launch. Promotion
must run in an update that queues another update for `patch`, never in a sleeping worker:

```rust,ignore
handle.update(|state, _| {
    state.main.set_dock_icon(true);
    state.main.update(|_, _| patch_on_main_thread());
});
```

Pinned Rune `app.rs::Shared::drain` (29aacb6) takes the current updates before running them
and preserves nested updates for the next turn. `window/lifecycle.rs` applies the Dock
request after that drain. The demo's turn-model test checks the patch sees the promoted
menu; physical ⌘Q delivery is a separate live check. Reapply `patch` after `rune::set_labels`
and menu replacement. Menu callbacks only queue work. Never send synchronous `terminate:`
from an update. S40 maps `Shortcut::closes_windows` in palette, picker and form `on_key`,
and implements commit-draft/cancel-form/release-suppression before hiding.

Keep the native owners on the main thread. Native entry points require a
`MainThreadMarker`, or an owner/scoped reference whose construction required one. No owner
stores a borrowed NSWindow or NSView pointer. A GuidePanel retains only its own panel/view.

S42 feeds SidebarMaterial the **drawn animated slot rect**, including zero width at collapse,
on every layout/animation frame and after content replacement. A fixed-width child inside
SplitView's clipped slot is insufficient; the demo deliberately owns its animated slot.
`sync` moves the same retained effect view to Rune's actual view parent and inserts it Below
that specific `ns_view`, converting its frame into the parent's coordinate space. Height autoresizing covers the interval before the next layout callback.
`remove` and Drop detach it; reinstalling with `sync` is safe. The host must be transparent.

S40 maps the persisted appearance preference to the adapter and every window's theme in
one turn. Forced Light and Dark never consult `Cx::system_dark()`. System passes a closure
which reads it, and sets `NSApp.appearance` to nil. The demo uses the same preference in both
its windows; the real shell still owns cross-window coordination. Rune's `system_dark()`
mirrors a forced app appearance. After `apply(System)`, it can retain the previous forced
value for one turn: S40 rebuilds every window on `ThemeChanged`.

S66 opens the selected `Pane::url` through its system port, begins a generation and schedules
the returned deadlines through Rune. Before **every** delayed placement, check `is_current`.
Choose a screen by the Settings midpoint, falling back to `window.screen ?: mainScreen`.
Call Rune `place` inside an update, then set the exact native origin in a nested update.
Arm one 200 ms placement watchdog when Ready is returned. A subsequent `Step::Placing`
requests no wake; poll on the watchdog if placement has not completed. Only then read the
actual companion frame and call `started`; late completion is rejected even without a poll.
Do not use the planned frame or a `(0,0)` placeholder. Overlap skips the ghost. Pass the actual drawn icon rect through
`WindowRef::rect_to_screen` on each frame. Show the guide at 30 Hz while `Step::Draw` returns.
Hide it on Stop; a stale callback must leave a newer guide alone. Immediately stop/hide on
real drag start, grant or close. Waiting and Ready check only generation, visibility and a
real drag, so foreground changes, a frame change or Reduce Motion cannot prevent placement. While Running, observe
Settings/companion movement, grant, foreground relevance and Reduce Motion before drawing.
Use the native panel.

The demo's guide controls reuse main as the companion. Collapse the sidebar and resize it
to roughly 420×660 before starting the guide, with Settings already open. A too-wide main
overlaps Settings, intentionally skipping the ghost. The explicit stop button simulates
grant/close; dragging the real icon stops it from the actual Rune drag-start callback.
S66 supplies the permission-service grant, the accessible reveal button next to the icon,
the macOS 27 Device Control label, and the production companion's window configuration.

S66 **must not copy the demo's `expect`s**. Production handles a missing/destroyed window,
missing bundle, display changes and adapter errors with None/Result handling, stops the
current guide safely and leaves the permission companion usable.

## Evidence and limits

Pure tests cover menu ordering/idempotence and pre-promotion close; callback reentrancy and
disabled actions; exact hints and ⌘Q modifiers; all four glyph variants at three scales;
appearance source selection; metadata filtering; exact offset/negative-y/flip/clamp/tall-window
placement; animated/sidebar clipping; display selection/union; guide retry deadlines,
generation replacement, phase-specific cancellation, placement timeout, overlap and the strict > 4.8 s stop. A source guard rejects
capture APIs/frameworks and window-title/owner-name queries throughout the crate's source.
No raw window dictionary is logged.

The following are **unverified live evidence**, not failures observed in AppKit:

- S10 / S40 / S71: glyph visibility, native menu updates, physical ⌘Q with Dock on/off before
  promotion and in each launcher, editing shortcuts, Dock reopen, bundled Dev Finder/`open` reopen, true Quit and
  hidden/never-created system quit. S72 owns logout with a durable hook file.
- S12 / S71: actual sidebar material appearance, resize and full-screen lifecycle,
  collapse/reopen animation, repeated removal/reinstall and content replacement; toolbar
  inset movement from the transition alone, without forcing state; native menus/traffic
  lights in every window for forced and live System appearance; a real drag; a read-only
  real Tauri-window comparison when the owner has one on screen.
- S15 / S66 / S71: panel drawing and actual click-through, above-Settings order from
  pid/layer metadata, fullscreen Spaces and multiple/mixed-scale displays; preflight recorded
  alongside a metadata-only lookup without Recording access; bundled Dev real granting drag and drag
  cancellation, keyboard reveal and first-mouse behavior. This step changes no grant or
  system setting and runs none of those live actions.

No new Rune issue is inferred from these unexecuted checks. Existing gaps remain #141,
#561, #562, #563, #146 and #574; local plan gap 16 has no upstream issue.

Integration edits outside S38 Owns: the approved workspace dependency declarations in
`crates/kudzu-appkit/Cargo.toml`, and Cargo-generated `Cargo.lock` entries for the adapter and
demo. The parent `lib.rs` declarations are inside Owns. No frozen app file is edited.

## Initial S38 acceptance and gate record (2026-11-11, historical)

Implementation: Codex gpt-6.1-sol, xhigh. This implementation task invoked no delegates or
independent reviewer. Rune source was read at the pinned 29aacb6 revision.

| S38 Accept item | Result | Evidence |
|---|---|---|
| Menu-model tests need no window server | PASS | Empty accessory menu, promotion order, repeated patch, duplicate close, callback self-removal and disabled action tests; no native menu construction in tests. |
| Glyph with update dot, pixel test | PASS | Four variants × three scales: dimensions, opaque dot, clear halo, unchanged pixels outside the dot area. |
| Shortcut hint text | PASS | Exact ⇧⌘P, ⌘O, ⌘, strings and exact launcher ⌘Q modifier tests. |
| Demo binary exercises each required module | PASS (hook/build) | `cargo build -p kudzu -p appkit-demo`; ordinary `target/debug/appkit-demo` exits 0 without a window. All native paths are available under `--live`; they were not executed in this step. |
| No Rune or Kudzu dependency in the adapter | PASS | `cargo xtask check-deps`: workspace dependency and containment checks passed. Direct dependencies are image and the three approved objc2 crates. |

Original S38 gate, before the review follow-up:

- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace`: PASS, 706 tests including doc tests, zero failed/ignored.
  S38 supplies 17 library tests, one demo turn-model test and one compile-fail lifetime test.
- `cargo xtask check-deps`: PASS.
- `python3 scripts/check-headless.py`: PASS, all four modes under its write sandbox, using
  the freshly built root binary.
- `git diff --check` and the gap-header/file-size audit: PASS.

The first workspace attempt failed the unchanged clipboard search timing test at p95
7.672334 ms against 5 ms. An isolated rerun passed at 1.758458 ms; the subsequent complete
workspace invocation passed. No test, threshold, core source or runner settings were changed.
The privacy guard was also proven negative: adding a temporary source file containing a
forbidden capture token made it fail; the file was then removed and the full suite passed.

The extra live review checks listed above remain **UNVERIFIED**, including the S12 requests
for live full-screen inset/toolbar movement, appearance, real drag and reference capture.
This report does not count the demo's compilation or model tests as that evidence. No
native demo window, permission prompt, System Settings window or installed Kudzu app was
opened or modified by this step.


## S38 review follow-up

All findings were checked against `permissions/macos.m` and `tray.rs` in the frozen app.
The guide canvas additionally follows the owner's decision to clip to the Settings screen.

| Finding | Fix and evidence |
|---|---|
| Waiting/Ready skipped placement | Only generation, visibility and real drag cancel before Running. Tests reach Ready under Reduce Motion, a moved companion, an unrelated front app and a grant observation; the first Running poll suppresses drawing. |
| Ready spin / late completion / cap | Placing returns no wake. Tests cover hidden/dragging/replaced Ready generations, the 200 ms deadline, a late started call without a poll, and Draw at exactly 4800 ms followed by Stop at 4801 ms. |
| Canvas | show calls the tested canvas function: NSUnionRect + NSInsetRect(-80,-80), intersected with the Settings screen frame. Exact fixtures are (312,82,1338,1012) and the clipped offset/negative-y (1800,-900,1000,780). |
| Update dot | Ported tray.rs with_dot, including per-representation pixels, min-dimension radius/floor, centre, hard disk and alpha-only clear ring. The replacement test failed on the old lower corner; exact @2x disk/ring/outside pixels now pass for all variants. |
| Target lifetime / SAFETY | action retains self before reading ivars; callbacks are cloned before invocation. Pointer comments name non-owning menu/parent/target hazards, and the dashed curve documents its copied two-element pattern. Existing reentrant/disabled callback tests pass. |
| Outline / view coordinates | Outline stays solid and only the curve is dashed, matching macos.m. WindowRef preserves Rune's exact ns_view; conversions start there and the sidebar is a sibling Below that view, including after reparenting. Source/bindings checked; native rendering still needs S71. |
| Screen edges / origin failure | NSMouseInRect uses the inclusive top edge; the Settings midpoint stays half-open. The shared-edge regression failed before the change and now passes. set_origin returns Result and the demo stops its guide on failure. |
| Appearance / local gap | AppKit NSAppearanceName constants replace string names. The API and this README document the forced mirror, stale System transition and S40 ThemeChanged rebuild. Gap 16 is marked RUNE-GAP(none; plan gap 16). |
| Demo limits | Finder/open reopen and real granting drags belong to the bundled Dev app. S66 handles errors explicitly and must not copy the demo's expects. |

The lifecycle, dot and pointer regressions were observed failing before their fixes. Canvas
fixtures were added before the new calculation and then passed through the function used
by show. Native AppKit menu/Spaces/window behavior was not driven during this follow-up;
the existing S71 live checks remain required. No permissions, system settings or installed
Kudzu application were changed. Builds reuse this worktree's existing target directory.


Follow-up verification, on the final Rust source:

- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace`: PASS, 911 passed, zero failed, two pre-existing ignored
  idle-runner clipboard construction/search measurements. No ignore or threshold was changed.
- `cargo xtask check-deps`: PASS.
- `cargo build -p kudzu -p appkit-demo`: PASS, using the existing target directory.
- `python3 scripts/check-headless.py`: PASS, all four modes under its filesystem write sandbox.
- Ordinary `target/debug/appkit-demo`: exit 0 without a native window.
- Adapter-specific tests: 26 unit tests and the scoped-handle lifetime compile-fail test pass.
- `git diff --check`, gap-header and file-size checks: PASS.

This follow-up changes only S38-owned adapter sources and demo files. No dependency or
integration edit outside Owns was needed. No new Rune gap was identified or filed.
