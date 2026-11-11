# S12 — main window chrome and sidebar material spike

Date: 2026-11-11. Host: arm64, macOS 27.2, Rust 1.99.1, one display (1800×1169 pt, 2×). Rune: `29aacb688883a9712bc27175bdafd99fc6edb421`.
No branch change, push, PR or upstream issue write. Nothing under `apps/desktop` was read at build time or touched.

Verdict, per question:

| Question | Verdict |
|---|---|
| Transparent full-size-content window, traffic lights at (18, 22), drag regions | **go** (drag itself not driven) |
| Sidebar as `SystemMaterial::Popover` + .72/.55 tints | **go** technically; **differs from Tauri when the window is inactive** — owner decides (D3) |
| Fallback: `NSVisualEffectView` sidebar material under the content view | **go** (built and captured; resize and full screen not exercised) |
| `NSApp.appearance` forces materials and `Cx::system_dark()` | **go** |
| Menus follow `NSApp.appearance` | **not demonstrated** |
| Toolbar inset in full screen | **go** |
| `SplitView` collapse | **go** |

## What was run

`cargo run -p chrome -- --probe --out DIR --report FILE` (in `apps/desktop-v2`), then `cargo run -p chrome -- compose --raw DIR --reference REF --out docs/migration/spikes/S12`. Dev profile, workspace `opt-level = 2`. The probe is a regular (non-accessory) app so the menu bar exists; it opens for about 40 s and quits.

- **Window:** `App::new(AppSpec::new("Kudzu chrome spike", (1180, 760), …)).chrome(Chrome { full_size_content: true, traffic_lights: Some((18, 22)), drag_regions: true }).standard_menus()`, `WindowOptions { transparent: true, min_size: Some((900, 560)) }`.
- **Scene** (`spikes/chrome/src/scene.rs`): rune-kit `SplitView` with a 224 pt sidebar and a rounded card, over one window-wide element carrying the material, with a child tint at the Tauri values (`--sidebar` under `.vibrant`: oklch .985 at α .72 light, oklch .216 at α .55 dark). The kit's `bg_deep` is cleared so the shell lets the material through. This is what the Tauri window does (`bg-sidebar` on the root, the card opaque on top). Icons are grey squares and the toolbar title is centred: the scene is for material, not for layout parity.
- **Backdrop:** a borderless, mouse-transparent window under the spike window, painted with a fixed pattern of saturated hue bands, a diagonal sweep, hard black and white blocks and a checker field. Every capture sees the same thing behind the material, whatever the wallpaper is. Captures are `screencapture -x -R` of the window's frame (2360×1520 px), so they include the real compositor output.
- **Inactive** is a second small titled window of the same app taking key (`NSApp.deactivate` was ignored on this macOS, and leaves the app "active"). The spike window then has `isKeyWindow` false and grey traffic lights, the same state a window has when another app is in front. A capture with another app actually in front was not taken.
- **Window level** was 4 (above the backdrop) for captures. At level 4 `toggleFullScreen:` did nothing, so the full-screen step drops to level 0 first. That is a spike artefact: Kudzu's main window is level 0.

## The images the owner decides from (D3)

All under `docs/migration/spikes/S12/`, 1.0 MB in total, fixed test backdrop, left to right **Reference | Popover | Native sidebar material**:

- `sidebar-light.png`: top row active window, bottom row inactive (not key). Reference column is the browser capture only.
- `sidebar-dark.png`: the same in dark.
- `window-dark.png`: whole window, Popover against native sidebar, dark, active.
- `collapse-dark.png`: `SplitView` with the sidebar closed.
- `stats.json`: mean RGB of a text-free strip of the sidebar (x 4–14 pt, y 100–700 pt) in each capture, and the measured traffic-light disc.

**What the columns are.** Reference: `library.{light,dark}.png` from the S02 capture. It is a browser capture, so it is opaque (`--sidebar` without `.vibrant`) and shows no vibrancy. **The installed Kudzu's main window was not on screen** (every Kudzu window reported `onscreen: false`, hidden in the tray), so I did not capture it. Popover: Rune's `SystemMaterial::Popover`. Native sidebar material: an `NSVisualEffectView`, material `sidebar` (7), behind-window, `state` 0 (follows the window's active state), under the content view, with the same tints. That is the material Tauri's `windowEffects: ["sidebar"]` / `followsWindowActiveState` configuration asks for in `tauri.conf.json`. It stands in for the Tauri window; it was not compared with a capture of the real Tauri window.

Mean RGB of the strip (8-bit sRGB), same backdrop:

| | Reference (opaque) | Popover | Native sidebar | Native, inactive | Popover, inactive |
|---|---|---|---|---|---|
| Light | 249.8, 249.8, 248.8 | 237.8, 236.2, 236.2 | 243.0, 242.3, 241.8 | 245.0, 245.0, 245.0 | 237.8, 236.2, 236.2 |
| Dark | 28.2, 25.2, 23.2 | 55.9, 52.2, 52.8 | 45.8, 43.2, 43.3 | 37.0, 37.0, 36.0 | 55.9, 52.2, 52.8 |

What the numbers and the images say:

1. **Active, Popover differs from the sidebar material by about 5–6 levels in light (darker, a little cooler) and 9–10 in dark (lighter, greyer).** Both let the backdrop's hue through weakly under the .72/.55 tints; neither is flat. In the images the difference is a slight shift in tone, not in structure.
2. **Inactive is where they diverge.** Rune creates its material view with `state` 1 (always active; read back from the live view), so Popover looks exactly the same active and inactive (identical means). The sidebar material, with `state` 0, goes flat and neutral when the window is not key (light 245, 245, 245; dark 37, 37, 36: the hue is gone). That is what the Tauri window does today. So Popover keeps the sidebar vibrant in a background window, where the app it replaces dims it.
3. In dark, the Popover sidebar is visibly lighter than the Tauri one, which is the thing the owner may or may not like.

Caveats: the backdrop is synthetic; on a real wallpaper both materials show less of it and the differences shrink or grow with the wallpaper's brightness. One display. No capture with a real foreground app behind. The resize and full-screen behaviour of the fallback view was not exercised (it has an autoresizing mask for width and height).

## 1. Window chrome — go

From AppKit on the live window (`spikes/chrome/evidence/report.json`, `initial`):

- Style mask 32783 = titled, closable, miniaturizable, resizable, **full-size content view**. `titlebarAppearsTransparent` true, title hidden, `isOpaque` false, `hasShadow` true. The system draws the rounded frame and shadow.
- Traffic lights: close, minimise and zoom frames inside the 58 pt titlebar container are (18, 22), (41, 22), (64, 22), each 14×14 pt (pitch 23). Measured from the capture's pixels, the close disc is 12 pt across with its left edge at 19.0 pt and its top at 22.0 pt (±0.5 pt, `stats.json`): the button's top-left is at (18, 22) within a point, as the plan sets it.
- The AppKit `convertRect:toView:nil` of those frames reads 2× the pixel measurement (36, 44); I treat it as unreliable and rely on the raw frame and the pixels. Whether the Tauri window's lights sit at the same place was not compared (no Tauri window to measure). `Chrome::traffic_light_rects` assumes 14 pt buttons on a 23 pt pitch: matches.
- Drag regions: the content view is `RuneDragRegions_RuneDrops_AccessKitSubclassOfWinitView` with `mouseDownCanMoveWindow` false, so only `.window_drag()` regions can move the window. `SplitView`'s 48 pt band is one. **A real drag was not driven** (it needs a physical mouse); this is configuration evidence only.
- Minimum size 900×560 was passed; I did not resize the window to test the floor.

## 2. Sidebar material — go for both; see the images

- `SystemMaterial::Popover` on a window-wide element: one native view, `NSVisualEffectView`, material 6, behind-window, state 1, 1180×760, appearance `NSAppearanceNameVibrantLight` or `VibrantDark`. One material, against `MAX_NATIVE_MATERIALS` 4. The tint has to be a child element: a native material replaces its own element's fill.
- Fallback: `native::set_sidebar_view` (`spikes/chrome/src/native.rs`, about 35 lines): allocate an `NSVisualEffectView`, `setMaterial: 7`, `setBlendingMode: 0`, `setState: 0`, width+height autoresizing, `addSubview:positioned:NSWindowBelow relativeTo:nil` on the content view. It shows under Rune's transparent Metal layer and its tint and card. Rune's own idle Popover view stays in the hierarchy at `alpha` 0 (read back), so nothing sits over it. The finished module is `kudzu-appkit/src/sidebar_material.rs`; it would take the sidebar's drawn rect when the material covers only part of the window, but a window-wide view needs no frame sync.

## 3. Appearance — go

`NSApp.setAppearance(NSAppearance(named: …))`, system appearance **dark** throughout (`AppleInterfaceStyle` read back):

| `NSApp.appearance` | Window `effectiveAppearance` | Material view appearance | `Cx::system_dark()` | View rebuilt |
|---|---|---|---|---|
| Aqua (forced light) | Aqua | VibrantLight | **false** | yes |
| DarkAqua (forced dark) | DarkAqua | VibrantDark | true | yes |
| nil (System) | — | — | **true** (follows the system) | yes |

So materials follow the app setting, and `Cx::system_dark()` follows `NSApp.appearance` too (Rune's winit `ThemeChanged` path), including back to the system value when it is cleared. **The theme can be `cx.system_dark()` in every window once `NSApp.appearance` is set; no second source is needed.** That simplifies the §3 gap-4 workaround: `kudzu-appkit/src/appearance.rs` is the setter alone.

Not shown: native menus. They follow `NSApp.effectiveAppearance` by AppKit's contract, but I did not open one (the only way I found was a modal menu-tracking call), so this is not demonstrated. Traffic lights: the light-run captures show them drawn normally; I did not isolate an appearance effect on them.

## 4. Full screen — go

`Cx::titlebar_inset()` read from a view rebuilt after each transition: **78 pt** windowed (18 + 2×23 + 14), **0 pt** in full screen (`NSWindow` style mask had the full-screen bit), **78 pt** after leaving. `SplitView`'s title row and toggle follow it. A forced rebuild (a state bump) was needed to read the new value in the probe; Rune's own redraw on the transition was not timed.

## 5. `SplitView` collapse — go

`PaneState::set_open(false)` closes the sidebar and the card takes the full width with its 8 pt frame; `set_open(true)` reopens it. The window-wide material is unaffected (one 1180×760 view throughout), so the collapse needs no material work. Stills only: the animation was not recorded. A material limited to the sidebar rect would have to follow the pane's animated width; Rune syncs native views to layout, but that was not tested here.

## Gaps and comments to file

No new Rune gap. The orchestrator files:

- **Comment on #562:** "Measured on 29aacb6: `SystemMaterial::Popover` creates its `NSVisualEffectView` with `state` 1, so a window's sidebar keeps its vibrancy when the window is not key; the sidebar material with `followsWindowActiveState` flattens to a neutral fill, which is what apps ported from Tauri/AppKit show. A Sidebar material (or a `follows_window_state` flag on Popover) would remove our native fallback."
- **Comment on #563:** "`NSApp.setAppearance` works today: the window's `effectiveAppearance`, Rune's materials and `Cx::system_dark()` follow it, and clearing it returns to the system value. Documenting it, or a `Tx::set_appearance`, would save the unsafe call."

## Decision for the owner (D3)

Both work. **Recommendation: take the native sidebar material (the fallback).** It is the same material the Tauri app uses, it keeps the inactive-window behaviour, and it costs about 35 lines of AppKit in one module (`kudzu-appkit/src/sidebar_material.rs`) beside `appearance.rs`. Choose Popover only if you prefer no native view and accept two differences: a slightly lighter, greyer sidebar in dark (about 10 levels on this backdrop) and a sidebar that stays vibrant when the window is in the background. Parity is the first priority in the brief; that is why I lean to the fallback. The decision, and S42's work, stay with you.

## Verdict

Window chrome: go. Popover material: go with a visible inactive-state difference. Fallback material: go. Appearance: go. Full-screen inset: go. Collapse: go. Not demonstrated: native menus' appearance, a real drag, the fallback under resize and full screen, a capture of the real Tauri window.

## Review findings (Codex Luna review, 2026-11-11: conditional go)

**Supported:** traffic-light coordinates, the measured full-screen inset values, and static `SplitView` collapse. The owner chose the native sidebar material (D3).

**Not yet proven:**
- The fallback view's lifecycle. It was framed to the whole content view, and it was off during the collapse and full-screen probes.
- Inset changes driven by the transition itself. The probe forced a rebuild before reading the inset.
- Appearance for native menus and traffic lights.
- A real window drag.
- A capture of the real Tauri window.

**S38 must add:**
- A main-thread-checked `sidebar_material` API that uses the window handle transiently and never stores a window pointer.
- A view sized to the sidebar, under Rune's Metal content, whose frame follows resize, full-screen transitions, collapse and reopen animations, and content-view replacement. Installing and removing it are idempotent.
- Verification of the full-screen inset and toolbar movement without forcing a state change.
- A typed `appearance` API. **Theme-source contract:** the theme resolves from the Appearance preference (Light or Dark). Only System reads `Cx::system_dark()`. `NSApp.appearance` is set from the same preference, so native menus, traffic lights and materials match. S38 checks menus, traffic lights, every app window, and live System appearance changes.
- The remaining evidence: a capture of the real Tauri main window when one is on screen (read-only), and a real drag of the window.
