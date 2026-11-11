# S11 — floating panel spike

Date: 2026-11-11. Host: arm64, macOS 27.2 (26B123), Rust 1.99.1 (58cf8f9c3 2026-10-13).
Source: fc8eed95a8db1589cf8842bf19b5617cb0eedcb4. Worktree branch: `t3/floating-panel-spike`.
Rune: git revision 29aacb688883a9712bc27175bdafd99fc6edb421.
No branch change, push, PR, or upstream issue write was performed.

Overall verdict: **go** for launcher panels. The §1.7 visible and encode budgets hold on the real `RuneOverlayPanel`. Gap 9 stays unused. Four questions are not a clean go: hide-on-blur was not demonstrated, fullscreen and Spaces rest on the collection mask, first-mouse was not posted to the window server, and a same-turn `show()` left the prewarmed fill on the first captured frame.

## How this was measured

`cargo run -p panels -- --probe` in `apps/desktop-v2`. The binary exits 2 with no windows unless `--probe` is passed, so `cargo test` does not open panels. Accessory policy (`RunOptions { accessory: true }`), `prewarm_gpu`, three windowless `WindowOptions::launcher_panel` windows (palette, picker, form) plus an ordinary host. The trigger is an in-process `AppHandle::show` inside the same `update` that starts the clock. It is not a Carbon global hotkey. Input Monitoring was not requested.

`NSApp.windows()` lists two windows per title: the hidden winit shell (`NSKVONotifying_WinitWindow`, style 4, level 0, alpha 1 while ordered out) and the panel Rune adopts (`RuneOverlayPanel`). Facts below are the panel. Selection prefers a same-title window that has a content view and whose class name contains `Panel`. The end-of-run inventory is in `spikes/panels/evidence/report.json` under `windows`.

Visible time is `on_focus(true)` when that snap already has alpha ≥ 0.95 and `visible`. Every latency sample took that path (`polled_alpha` false on 60 cached and 20 fresh runs), so the 8 ms poll did not add to these numbers. Encode time is the first `Drawn` `on_frame_encoded` after `t0`. `t0` is taken inside the update, before `show()`. Profile: dev, workspace `opt-level = 2`, not release LTO. The budget already passes, so there is no release batch. Other agents were building on this Mac; the spread below is the three batches inside this one process.

Pointer screen only: full frame `[0, 0, 1800, 1169]`, scale 2, `visibleFrame` `[0, 39, 1800, 1065]` (menu bar 39 pt). A second display was not live. Placement against another origin is covered by the `place` unit tests.

PNGs under `spikes/panels/evidence/` are gitignored. `report.json` is the committed record. TextEdit (pid 2738, `com.apple.TextEdit`) was already running. The probe did not launch it and did not quit it. `AXIsProcessTrusted()` was already true. The probe did not prompt, did not grant a permission, and did not post a global click.

Panel chrome on the measured window, throughout: class `RuneOverlayPanel`, style 132 (Miniaturizable | NonactivatingPanel), level 3, collection behavior 262481 (`CanJoinAllSpaces | Stationary | IgnoresCycle | FullScreenAuxiliary | CanJoinAllApplications`), `animationBehavior` 0 (`NSWindowAnimationBehaviorDefault`), `sharingType` 1, `canBecomeKey` true. Hidden prewarm: alpha 0, visible false, `ignoresMouseEvents` true.

## Questions

### 1. Hotkey-to-visible latency — **go**

Three batches of 20 cached shows, one warmup discarded, first open kept separate. Nearest-rank p95. Median of the three batch p95s:

| Clock | Median of batch p95 | Batch p95s | Batch medians | Batch min | Batch max |
|---|---|---|---|---|---|
| Visible | 15.15 ms | 15.15, 14.54, 24.00 | 9.56, 9.54, 9.95 | 6.69, 4.15, 4.36 | 510.92, 15.02, 514.83 |
| Encode | 8.05 ms | 7.91, 8.05, 8.07 | 5.63, 5.60, 5.02 | 0.52, 0.57, 0.69 | 8.01, 8.05, 19.71 |

§1.7 asks for handler-to-encode ≤ 16 ms p95 and visible ≤ 80 ms. Both hold. Zero encode samples were missing. First open, not in the 60: visible 14.11 ms, encode 1.14 ms, encode cost 0.10 ms, alpha 1, key, frontmost `com.colliderli.iina`.

Three stalls sit near 511–515 ms. Cached batch 0 index 14: visible 510.92 ms, encode 3.61 ms. Cached batch 2 index 10: visible 514.83 ms, encode 8.07 ms. Fresh index 0: visible 510.95 ms, encode 7.01 ms. On each, `key_ms` equals `visible_ms` and `polled_alpha` is false, so the wait is inside gaining key, not the poll. Encode on those runs stayed under 16 ms. One stall in a batch of 20 does not enter nearest-rank p95; batch 2's visible p95 is 24.00 ms because of the next-slower run. Frontmost on all 60 cached and all 20 fresh runs was IINA, and `NSApp.isActive` was true at the key callback. `animationBehavior` stayed 0.

The clock is an in-process `show()`, not a global hotkey. A signed bundle with Input Monitoring is the owner's measurement.

### 2. Keys without activating; previous app stays frontmost — **go**

The panel became key (key window true, focused true, visible 6.81 ms on the key-test show). Before the show and after the hide, frontmost was TextEdit pid 2738, activation policy 1, `NSApp.isActive` false. At the key callback `NSApp.isActive` was true while TextEdit was still frontmost. The same split held for the latency runs: IINA stayed frontmost while `isActive` was true. The previous app stayed frontmost. Becoming key does flip `NSApp.isActive` on this accessory process.

### 3. Hide on blur — **no-go** for the live proof

Palette and picker set `dismiss_on_blur`, which calls `close_window()` from `on_focus(false)`. Form does not. That loss path does run on `hide()` (question 11). It did not run for blur.

`NSRunningApplication.activateWithOptions(ActivateAllWindows)` on TextEdit returned activated true. TextEdit was already frontmost, so the activate can be a no-op. After it, the palette (`expect_hide` true) was still key, visible, alpha 1, `lost_focus` false. The form (`expect_hide` false) was also still key and visible, `lost_focus` false. The two cases were not distinguished. This is not a silent pass, and it is not a reason to reject launcher panels. The owner confirms it by clicking another app.

### 4. Placement under the pointer — **go**

Origins use the screen's full frame, the same frame the Tauri presenter uses, not `visibleFrame`. Tolerance in the probe is about 2 pt.

| Panel | Expected origin | Measured | Delta |
|---|---|---|---|
| Palette, 20% of 1169, 760×480 | 520, 233.8 | 520, 234, 760×480 | 0, 0.2 |
| Picker, 18% of 1169, 800×520 | 500, 210.42 | 500, 211, 800×520 | 0, 0.58 |

Both `frame_ok`. Class `RuneOverlayPanel`, level 3.

### 5. Caret rect, flip, content height — **go**

Caret `[220, 140, 2, 16]` places the form below at `[196, 162]`, measured `[196, 162, 440, 220]`, delta 0. Shot: `spikes/panels/evidence/form-below.png` (window capture, opaque coral, caret in Search).

Caret `[220, 1141, 2, 16]` flips above at `[196, 915]`, measured `[196, 915, 440, 220]`, delta 0. The window-id PNG `form-above.png` is empty (center alpha 0). The screen-rect shot `form-above-screen.png` shows the coral form above the Dock, rounded corners, a caret, and a shadow on the wallpaper.

`resize_window_instant` then changed height and kept the bottom edge. 240 → height 240, y 915 → 895. 500 → height 500, y 895 → 635. 900 clamped to 640, y 635 → 495, frame height 640. `top_held` is false on all three. The clamp is the plan's 160..640. The Tauri presenter clamps to 680. Production re-places after a content resize if the top edge must stay put. The owner picks 640 or 680.

### 6. Popover material, radius 18, native shadow — **go**

The picker was not given a spike shadow override (`shadow_forced` false). Before and after the material flag: class `RuneOverlayPanel`, `NSVisualEffectView`, material 6 (popover), blending 0, state 1, content-view `cornerRadius` 18, `hasShadow` true, level 3, `animationBehavior` 0. The palette, with no material flag, has an empty effect view, radius 18, shadow true.

`spikes/panels/evidence/picker-material.png` is the window-id capture: rounded gray panel, "picker token 0", caret. Center pixel (102, 103, 103, 255). A vibrancy view's backdrop often looks flat in a window-id capture, so this PNG is not evidence of desktop bleed-through. The material numbers are the identity.

`palette-shadow.png` (window id) is fully transparent. `palette-shadow-screen.png` is the desktop shot: rounded coral panel, "palette token 0", the search field showing `kk` and a caret, and a shadow on the wallpaper.

### 7. Focus in the search field on show — **go**

The panel was key on show. Every capture shows the caret in Search. The in-process `keyDown` (keyCode 40, addressed to the panel's window number) was sent twice. The model string read 80 ms later was still empty (`text_after` `["", true]`, `tx_focus` `{sent: true, text: ""}`). `palette-shadow-screen.png`, taken later, shows `kk` in the field. The keystrokes landed after the 80 ms read.

### 8. Returning focus to the target app — **go** for the case this probe could see

After hide, the palette panel is alpha 0, not visible, not key, and `NSApp.isActive` is false. TextEdit is still frontmost, the same pid as before the show. The probe did not call activate on hide. Frontmost had never left TextEdit, so there was no stolen activation to hand back. What was shown is that hiding the panel releases key and `isActive` while the other app stays frontmost.

### 9. Fullscreen and Spaces — **fallback**

The collection mask on the real panel is the Space behavior: `CanJoinAllSpaces`, `FullScreenAuxiliary`, `CanJoinAllApplications`, plus `Stationary` and `IgnoresCycle`. `on_space` was true whenever the panel was sampled.

The accessory host was shown and focused, then `toggleFullScreen` was sent. Within 5 s the FullScreen style bit stayed clear (`entered` true, `became_fullscreen` false, `left` true). When sampled, the host was visible, not key, style 32783, level 0, fullscreen false. The panel beside it was key and on the current space. The host is this process, which is a weaker test than another app's fullscreen space. No Dock or Spaces setting was changed, and no other Space was visited. `spikes/panels/evidence/fullscreen-palette.png` is the shot from that attempt. The owner runs the panel over a different app's fullscreen space.

### 10. First mouse (#388) — **fallback**

`acceptsFirstMouse:` is true with no event and with an event. The panel was resigned and stayed non-key (`resigned` true, `key_before_click` false). An in-process `mouseDown` re-keyed the panel (`key_after_click` true) and did not increment the root `on_press` (`clicks_delivered` 0). A window-server click was not posted: a global click could land on the owner's desktop. Accessibility was already trusted; the missing piece is the posted click, not a denied prompt. This does not reject the step. Cached reveal already takes keys, so first-mouse matters for a non-key form. The owner posts one real click at a non-key form panel.

### 11. `App::on_focus` gain and loss (FRM-4) — **go**

Gain: every measured show recorded `on_focus(true)` with the panel key (the 60 cached runs, the 20 fresh runs, the first open, and the key test). Loss on hide: 62 hides, 62 focus-loss callbacks. Loss on blur was not observed (question 3).

### 12. Same-turn state change and `show()` — **no-go** for a fully fresh first frame

The fresh series sets `token` from 0 to 1 in the same update as `show()`. `view()` reads that one field for both the fill and the label. Token 0 fill is linear (0.86, 0.16, 0.14), which displays near (239, 111, 105). Token 1 fill is linear (0.12, 0.72, 0.28), which would display near (97, 219, 143).

All 20 fresh runs: `flashed` false, `token_at_last_drawn` 1, one drawn frame. The flag only looks at the encode callback's token, and that callback saw token 1 with no earlier visible token-0 frame. The window capture disagrees about the fill. `spikes/panels/evidence/fresh.png` is 1612×1052, label "palette token 1", caret in Search. Center pixel (223, 118, 110, 255). An 8 px grid of the opaque pixels found 0 green-dominant samples, 22732 red-dominant, 46 near-white (the glyphs). The label updated. The background stayed in the prewarm red family. Production should not treat a same-turn mutation as fully on screen until this is understood. The budget decision does not depend on it.

## Accept

| Item | Result | Evidence |
|---|---|---|
| Visible ≤ 80 ms p95 | PASS | Median of three batch p95s is 15.15 ms. Worst batch p95 is 24.00 ms. First open 14.11 ms. |
| Handler-to-encode ≤ 16 ms p95 | PASS | Median of three batch p95s is 8.05 ms. Worst batch p95 is 8.07 ms. One encode sample reached 19.71 ms and stays outside p95. |
| 20-run contract, spread stated | PASS | 3×20 cached inside one process, plus 20 fresh. The ~511 ms key stalls are 2/60 cached and 1/20 fresh. |
| Decision on gap 9 | PASS, **go**, fallback not applied | `animationBehavior` remained 0. The report's `fallback` field is null. `kudzu-appkit/src/panel_tuning.rs` was not created. Encode misses are not something animation behavior would fix, and encode p95 is inside 16 ms. |
| Memory, hidden, panels prewarmed | PASS for this process | `footprint -p` on the spike, after the three panels were prewarmed and hidden, before the scenarios: 64 MB, pid 30843. Budget 150 MB. This is not `/Applications/Kudzu.app` and not the future app with the engine resident. No production mitigation was added. |

## Gate

Run in `apps/desktop-v2` on 2026-11-11, one cargo at a time, warm target cache. No `panels` process was left running. `cargo test` did not open windows.

- `cargo fmt --all --check`: PASS (exit 0). The copied-crate fmt failure recorded in S01 is not present on this tree.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS in 16.47 s.
- `cargo test --workspace`: PASS. 465 tests, 0 failed, including the 6 `panels` placement and percentile tests. Compile of the test profile finished in 1 m 08 s.
- `cargo xtask check-deps`: PASS (`workspace dependency and containment checks passed`).

## Proposed Rune issues

Not filed.

**Launcher first reveal keeps the prewarmed fill.**
A same-turn token change plus `show()` encoded a frame whose callback token and on-screen label were the new value, but the window capture's fill stayed the prewarm color (no green-dominant pixel; center 223,118,110 against a green of about 97,219,143). The hidden-frame fence did not cover the background fill.

**Activating another app did not resign a launcher panel.**
`activateWithOptions(ActivateAllWindows)` on TextEdit left the `RuneOverlayPanel` key and visible, so `on_focus(false)` did not run and hide-on-blur could not be told from "stay up". TextEdit was already frontmost, so this activate may have been a no-op rather than a Rune bug.

## Existing issues, mention only

- **#335.** p95 visible and encode are inside the budget. About 511 ms `on_focus` stalls remain on 2 of 60 cached shows and 1 of 20 fresh shows. The trigger here is in-process `show()`, not a Carbon hotkey.
- **#388.** `acceptsFirstMouse:` returned true. A window-server click was not posted, and the in-process `mouseDown` re-keyed the panel without delivering `on_press`.
- **#146.** Rune's display list is a startup snapshot. This spike read `NSScreen` itself. The pointer stayed on one display, so a second screen was not measured live.
- **#161 and #162.** The hidden prewarmed spike process was 64 MB. That sample is not the full app.

## Owner runs and decisions

- Time a real global hotkey on a signed bundle with Input Monitoring. The numbers above start at `show()`, inside the process.
- Click another app, one that is not already frontmost, and check that the palette and the picker hide and the form stays.
- Fullscreen a different app and show the panel on that Space. This process's `toggleFullScreen` did not set the FullScreen style bit within 5 s.
- Post one window-server click on a non-key form panel, with Accessibility, for #388. This spike did not grant that and did not post the click.
- Choose the form max height: 640, which is what the plan and this spike clamp to, or 680, which is the Tauri presenter.
- Decide whether a same-turn `show()` is acceptable while the first captured frame can keep the prewarmed fill under an updated label.
- Re-place after a content resize when the top edge must stay put. `resize_window_instant` kept the bottom.

## Review caveats (Codex Terra review, 2026-11-11: accept with caveats)

The spike's measurements are early evidence. They don't prove the budgets.

- **Latency (§1.7) isn't proven yet.** The clock started inside an in-process `show()`. The spike ran 3 batches of 20 instead of 100 shortcut-to-frame observations, inferred visibility from window state, and reported the median of batch p95s rather than one p95. Gap 9 stays provisional. **S56 and S60:** record 100 real global-hotkey opens from `shortcut_received` to `panel_frame`, confirm visible frames by screen recording, and find the cause of the ~511 ms key stalls (2 of 60 cached opens, 1 of 20 fresh) before tuning animation.
- **Stale first frame isn't proven.** The screenshot was taken after the show cycle, and the token tracks the view, not pixels. **S40 and S56:** update the state while the panel is hidden, wait for a confirmed render, `show()` on the next frame, record the first visible frames, and check the fill once it settles. Treat this order as the candidate workaround. No Rune issue until this test shows a Rune defect.
- **Hide on blur isn't proven either way.** Activating an app that was already frontmost didn't make a transition. **S40 and S56:** test `on_focus(false)` against a real click in another app; the palette and picker hide without stealing focus back. If Rune misses a real key-window resignation, bridge `NSWindowDidResignKeyNotification` through `kudzu-appkit`. Check what it means for a non-activating panel before relying on application-level resignation. **S57:** the form stays open and reports focus loss reliably (FRM-4).
- **Form height is settled.** The UI caps its content-driven height at 640 pt (`form-panel.tsx`), and the native presenter applies a separate 160–680 pt clamp. **S57** keeps both, and re-places the window after a resize so it stays in the same relation to the caret.
- **Narrower "go"s.** Placement, panel properties, gaining key, the search caret and release on hide have spike evidence. Typing responsiveness, focus returning to the target for insertion, loss on blur, visible vibrancy and placement after display changes don't yet. Full screen with Spaces, and first mouse, are unverified, not fallbacks. **S60** needs the same reset, focus, blur, first-mouse and latency checks as the palette. **S71** adds to the owner's four live checks: typing and insertion after dismissal, FRM-4 interruption and recovery, palette and picker blur separately, switching Spaces, display changes and mixed scales, caret resize and re-placement, and real material and shadow appearance.
