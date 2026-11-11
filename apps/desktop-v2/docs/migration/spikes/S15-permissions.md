# S15 — permission companion spike

Date: 2026-11-11. Host: arm64, macOS 27.2 (26B123), Rust 1.99.1 (58cf8f9c3 2026-10-13).
Worktree: `/Users/saeed/.t3/worktrees/text-expander/t3-9d8f27ac-b40898457aa28044`, branch `t3/permission-companion-spike`.
Rune: git revision `29aacb688883a9712bc27175bdafd99fc6edb421`.
No branch change, push, PR, or upstream issue write was performed. Privacy switches were not flipped. `/Applications/Kudzu.app` was not touched. The probe never dropped the icon into a privacy list.

Overall verdict: **go** for the companion window, Settings-frame metadata, placement, drag-out payload, and a Rune overlay on the current Space. Overlay chrome is the winit shell rather than `RuneOverlayPanel`; click-through is `OverlayInput::None`, and `NSWindow.ignoresMouseEvents` stayed false. Native NSPanel is implemented as the §3 fallback when the overlay is not on the active Space.

## How this was measured

`cargo run -p permissions-spike -- --probe` in `apps/desktop-v2`. A bare `cargo run` or `cargo test` opens no windows. The probe packs `target/bundle/Kudzu Permissions Spike.app` (`sh.saeed.kudzu.next.permissions-spike`), codesigns with `KUDZU_DEV_SIGN` or ad-hoc `-` (unset here, so `-`), re-execs into that bundle, then `AppHandle::show` on a windowless accessory app (`RunOptions { accessory: true }`, `prewarm_gpu`).

`App::windowless()` defers native creation. The first live run that never called `show`/`open` listed `windows: []` for 30 s. This probe calls `show` once the event loop answers an `update`; `shown_via` was `"show"`, `window_error` was null, first-frame warn 761 ms (setup 665 ms).

System Settings was opened with the Tauri URL `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility` and quit afterwards because this process opened it. Finder's front window opened by reveal was closed. Input Monitoring / Accessibility were not prompted or changed. `CGPreflightPostEventAccess` was already true; the live drag warped onto the companion icon only and released there.

Record: `spikes/permissions/evidence/report.json`. PNGs are gitignored.

## Questions

### 1. Borderless always-on-top companion stays visible while System Settings is active — **go**

Companion `kudzu-spike-permissions`, class `NSKVONotifying_WinitWindow` (ordinary always-on-top window, not a launcher `RuneOverlayPanel`), style 4 (borderless), level 3, collection 257 (`CanJoinAllSpaces | FullScreenAuxiliary`), radius 18, shadow true, alpha 1, visible, on the active Space.

While Settings was frontmost (`com.apple.systempreferences` pid 82751), `NSApp.isActive` was false and `companion_visible_with_settings` was true. The window is 420×660.

### 2. `drag_out` of the running `.app`; cancelled drag; click to reveal — **go**

`El::drag_out` + `DragItem::file(bundle)` is not a Rune gap. `drag_item` offers only a path whose extension is `app`.

Headless `Runtime`: a move under `DRAG_THRESHOLD` is a press (`presses == 1`, no start); crossing it starts a drag and, with no OS host, ends `Cancelled` on the next frame. A loose binary stays a press.

Live, signed bundle: private pasteboard FileURL + string matched the packed `.app`. During the synthetic drag, `Apple CFPasteboard drag` held

`file:///Users/saeed/.t3/worktrees/text-expander/t3-9d8f27ac-b40898457aa28044/apps/desktop-v2/target/bundle/Kudzu%20Permissions%20Spike.app/`

Release was on the companion (`released_on_companion`, `never_over_settings`). `on_drag_end` stayed empty for posted events; the headless path is the cancelled-drag proof. Reveal used `NSWorkspace.activateFileViewerSelectingURLs` on the running bundle (`revealed: true`). Live `presses` stayed 0 because the probe called reveal directly rather than clicking the icon.

The owner must drag the icon into the list and cancel the auth prompt. This process did not.

### 3. System Settings frame from window metadata only — **go**

`CGWindowListCopyWindowInfo` (on-screen, exclude desktop), no capture, no Accessibility. Filter matches `macos.m`: owner pid, layer 0, at least 400×300. Live destination `[830, 162, 740, 852]` (AppKit, primary height used to flip CG bounds). `opened_settings_url` true.

### 4. Place the companion beside Settings — **go**

`companion_origin` matches `macos.m`: prefer left with an 18 pt gap, flip right when that origin would sit past the 12 pt visible-frame margin, then `min(upper).max(lower)`.

Live: available `[0, 65, 1800, 1065]`, expected `[392, 354, 420, 660]` (`830 - 420 - 18 = 392`). `NSWindow.setFrameOrigin` set that frame (`origin_set` true). `AppHandle::place` with the same companion rect in display-local points left `place_delta [0, 0]`; `after_rune_place` stayed `[392, 354, 420, 660]`. Unit tests cover left, flip, and the 700×500 clamp fixture.

### 5. Guide as click-through Rune overlay, or native panel if it cannot join that Space — **go** (overlay)

Overlay spec name `kudzu-spike-guide`, `OverlayInput::None`, `macos_level(3)`, `prewarm: false`. After `overlay.show()`, the window was found, visible, on the active Space, level 3, canvas `[312, 82, 1338, 1012]` (union of companion and Settings, expanded 80 pt). Decision `"overlay"`; native NSPanel was not shown.

Caveats, not a Space miss: class `NSKVONotifying_WinitWindow` rather than `RuneOverlayPanel`; collection 0 (no `CanJoinAllSpaces`); `ignoresMouseEvents` false. Click-through is Rune's `set_cursor_hittest(false)` for `OverlayInput::None`. The native fallback (`NSPanel`, `ignoresMouseEvents` true, collection 257) runs when the overlay is missing or `on_space` is false. Ghost 2.4 s × 2, 4.8 s cap, 30 fps; skipped under Reduce Motion or overlap (unit-tested). Reduce Motion was false; animate true.

## Accept

| Item | Result |
|---|---|
| Each question answered on a signed Dev bundle | **pass** — ad-hoc signed `Kudzu Permissions Spike.app`, bundle id `sh.saeed.kudzu.next.permissions-spike` |
| Overlay or native decided | **pass** — overlay |

## Integration edits outside Owns

- `apps/desktop-v2/Cargo.lock` — cargo added `permissions-spike` (workspace `members = ["spikes/*"]` already included the crate).

## Rune gaps (not filed)

Proposed title: **macOS: `App::overlay` on a windowless accessory app stays a WinitWindow**

Body: At 29aacb6, `OverlaySpec` name `kudzu-spike-guide`, `OverlayInput::None`, `macos_level(3)`, `prewarm` false. After `OverlayHandle::show`, `NSApp` lists `NSKVONotifying_WinitWindow` (style 4, collection 0, `ignoresMouseEvents` false), not `RuneOverlayPanel`. Kudzu's permission guide needs a click-through panel that can join all Spaces.

Gap 16 (Settings location) is local: `CGWindowListCopyWindowInfo` metadata. It worked. No upstream.

## What the owner must verify live

- Drag the spike (or Kudzu Next) icon into Accessibility and cancel the auth prompt; confirm the listed item is this bundle, not `/Applications/Kudzu.app`.
- Click the icon to reveal in Finder (headless proves press vs drag).
- Click through the ghost/outline into Settings; if the overlay eats clicks, use the native NSPanel path.
- Companion and overlay on a fullscreen Space and on a second display.
- A display too narrow for side-by-side (unit-tested; overlay skipped).
- Reduce Motion (unit-tested skip).
- A stable `KUDZU_DEV_SIGN` identity, so TCC grants survive rebuilds.

## Reproduce

From `apps/desktop-v2`:

```sh
cargo test -p permissions-spike
cargo run -p permissions-spike -- --probe
```
