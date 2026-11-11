# S10 — menu-bar lifecycle spike

2026-11-11, arm64 macOS 27.2 (26B123), rustc 1.99.1; Rune `29aacb688883a9712bc27175bdafd99fc6edb421`. Implementation: Codex gpt-6.1-terra, xhigh. This is a throwaway probe, not production AppKit glue. It uses approved workspace dependencies and does not modify the frozen app.

Overall: **no-go for closing S10**. The menu model and a hidden-window Rune quit have evidence; actual keyboard routing, system quit, reopen and editing are unverified. They are not negative results, and must not be inferred from the synthetic actions.

## Reproduce

From `apps/desktop-v2`, `cargo run -p menubar -- --probe --auto` starts a windowless accessory app, installs the status item in an `AppHandle::update`, swaps its template image, promotes to regular, patches the app menu twice, opens and hides main, and requests Rune quit. It should finish itself. The recorded run is `spikes/menubar/evidence/auto.txt`. `--probe` without `--auto` leaves the item running for manual checks; use its Quit item afterward. This is an unbundled debug executable, not the final Dev bundle. For a separate UI attempt, the same executable was copied to a disposable `/tmp/KudzuS10Probe.app` with bundle ID `sh.saeed.kudzu.s10probe` and launched with `--probe`. Desktop control could attach to the bundle but could not attach to SystemUIServer (timeout); the probe was terminated by signal afterward. No login item, permissions, `/Applications/Kudzu.app`, or system settings were changed.

The item holds a retained Objective-C target on the main thread. Its callbacks queue work through handles. Promotion is asynchronous, so the spike schedules the patch after 300 ms; this is a probe convenience, not a proven production synchronization strategy. The patch finds `terminate:`, fails closed if absent, removes Show All, inserts the missing separator, clears Quit's key, and inserts one Close Windows. Repeating the patch left one item. The intended production adapter must handle ownership/teardown, failed or repeated promotion, menu replacement, and reliably patch after Rune actually promotes.

## Questions

1. **Status item — fallback (partial yes).** The run reached `install` before any window was shown, read back `Dev`, seven menu items and `isTemplate=true` after replacing the image with the paused PNG. The callback then hid main and the process remained alive. This verifies AppKit object state and a direct in-process callback, not visibility, appearance, click delivery or dynamic disabled/title changes. Rune's own `Tray::new` returns Unsupported on macOS. The #141 AppKit fallback remains necessary.
2. **⌘Q from Dock-on, Dock-off and launcher — no-go (unverified).** The AppKit menu model after promotion contains exactly `About | separator | Services | separator | Hide | Hide Others | separator | Close Windows [q] | Quit Kudzu []`, unchanged after a second patch. A direct callback hid main. No physical key event was sent. In particular, an accessory app and a nonactivating launcher may not receive its app-menu equivalent: model contents do not establish routing. Dock-off before the first promotion has no populated app menu in Rune source. Test all three situations before selecting this workaround.
3. **Dock Quit and logout invoke `on_quit` — no-go (unverified).** Neither Dock Quit nor logout was performed. Rune `quit/mac.rs` says system termination starts the quit sequence; this is source evidence, not a runtime result. A deliberately synchronous `NSApplication terminate:` sent *inside* an `AppHandle::update` aborted with winit's `tried to handle event while another event is currently being handled` before `on_quit`; see `evidence/reentrant-terminate.txt`. This was a synthetic reentrant call, not Dock Quit or logout, and does not establish how either behaves.
4. **`on_quit` with no open window — go for Rune-requested quit only.** After `open`, `is_visible` was true; after direct Close Windows callback and queued hides it was false. `AppHandle::quit` then logged `S10 on_quit` and the event loop returned with exit 0. This says nothing about a system-initiated termination with all windows hidden, or about a window that was never created.
5. **`on_reopen` — no-go (unverified).** A handler calls `main.open()` and returns true, but no real reopen event was delivered. `AppHandle::open` alone made main visible, which is not a reopen test.
6. **Edit shortcuts with `standard_menus()` — no-go (partial).** In the bundled probe, desktop control sent ⌘A in main, typed `S10 test`, and the accessibility value became `S10 test`; ⌘Z restored `edit this text`. Cut, Copy, Paste, and the launcher field remain untested. The original clipboard was not intentionally modified. Source installs standard responder-chain items, but two working shortcuts do not establish the complete set.

## Accept

| Requirement | Result | Evidence or missing proof |
| --- | --- | --- |
| Every question answered yes or no with evidence | **FAIL** | Above separates the observed results from the five unverified live paths; a no-go is not a claim of a negative test. |
| App-menu item order achieved | **PASS, model only** | `evidence/auto.txt` records the requested order after two patches; no physical menu display was inspected. |
| Gap 2 filed and #141 commented | **PASS, existing upstream work** | [#561](https://github.com/anomalyco/rune/issues/561) exists and [the #141 Kudzu comment](https://github.com/anomalyco/rune/issues/141#issuecomment-6782611542) exists, both verified read-only. No issue or comment was posted by this step. |

## Proposed upstream follow-up, not filed

- Existing [#141](https://github.com/anomalyco/rune/issues/141), proposed comment: A windowless accessory app created an `NSStatusItem` in `AppHandle::update`; AppKit read back a `Dev` title and a template image after swapping it. Actual display and event routing still need a live check.
- Existing [#561](https://github.com/anomalyco/rune/issues/561), proposed comment: After Dock promotion, modifying the `terminate:` item and inserting Close Windows yielded the desired model order and was idempotent over two calls. Physical ⌘Q delivery with Dock off and from a nonactivating panel remains untested.
- Proposed title: **macOS: synchronous AppKit terminate from an update aborts on winit reentrancy**. A synthetic `NSApplication terminate:` inside `AppHandle::update` triggered winit's nested-event panic and a non-unwinding abort before the quit hook. Is calling AppKit termination during an update unsupported, and what is the supported way to schedule it after the event turn?

## Owner live checks

Run `cargo run -p menubar -- --probe` and use the `Dev` status item. Verify actual glyph appearance and swap, title, tooltip, left-click menu, and menu callbacks. Open main, inspect the app-menu order and exercise ⌘Q with Dock on and off; repeat with a launcher key and the previous app frontmost. Confirm Close Windows leaves the process and item alive and a real Quit exits. With Dock on, use Dock Quit and confirm `S10 on_quit`; repeat when all windows are hidden. Reopen via Dock/Finder/open and confirm the handler opens main. Test editing shortcuts in both fields. Validate logout only in a safe owner-controlled session and verify the hook's durable output; terminal logs alone do not survive logout reliably. Quit the probe when done.

## Gate and integration

On this worktree, sequentially: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask check-deps`, and `python3 scripts/check-headless.py` all exited 0. The two gallery tests passed while flexi_logger emitted a symlink `AlreadyExists` warning. The headless script passed all four modes under its filesystem write sandbox. The spike process was no longer running after both live attempts.

The only integration edit outside Owns is Cargo's generated `Cargo.lock` entry for the new workspace spike and its already-approved dependencies. No production module was changed.

**Verdict: no-go** until the missing live acceptance evidence is collected; do not treat the observed menu model as proof of real ⌘Q delivery.

## Review findings (Claude Opus review, 2026-11-11: accept with changes)

By source (Rune 29aacb6 and winit 0.30.13), the mechanisms are sound:
- ⌘Q routes to Close Windows through the main menu once the app menu exists. Its target fires before Rune's key deferral.
- Dock Quit, logout, Activity Monitor Quit and the app-menu Quit all reach `on_quit` through `applicationShouldTerminate:` → `will_terminate` → `exiting` → `RunHooks`, including for hidden or never-created windows.
- `on_reopen` fires for a Dock click even with the Dock icon off. Reopening from Finder or with `open` needs a real bundle.
- The re-entrancy abort is **not** a Rune bug. It came from sending `[NSApp terminate:]` synchronously inside a winit handler, which production never does. The tray's Quit commits drafts, calls `shutdown()`, then `tx.quit()` or `AppHandle::quit()`. If a terminate is ever needed, defer it with `performSelector:withObject:afterDelay:0`. No upstream issue.

**S38 must do these (majors):**
1. **⌘Q before the first Dock promotion.** Rune leaves the app menu empty until the first `set_dock_icon(true)`, so with the Dock icon off (SHL-5) or a `--hidden` launch, ⌘Q does nothing. `app_menu::install()` runs once after the menu bar exists (first update or `on_ready`) and inserts "Close Windows ⌘Q" into the first menu even while it's empty. The post-promotion patch then moves it directly above `terminate:`, clears Quit's key and removes Show All. A duplicate key added later is dropped by AppKit, so ⌘Q stays on Close Windows.
2. **No sleep-timed patch.** Queue the patch from inside an update (`h.update(move |_, _| h2.update(|_, _| app_menu::patch()))`), so it runs on the turn after Rune applies the Dock change. Test this ordering.
3. **Status menu state at runtime.** Call `setAutoenablesItems(false)`, keep a `Retained<NSMenuItem>` per item id, and add `set_title`, `set_enabled`, `set_tooltip` and key-plus-modifier hints. Drive them from `main`'s `poll`. This covers the disabled status line, Clipboard History disabled while recording is off, and the ⇧⌘P, ⌘O and ⌘, hints (SHL-1, SHL-2).
4. **⌘Q in launcher panels.** The palette, picker and form `on_key` treat ⌘Q as `Shell::close_windows()`, as insurance in case the main menu doesn't see the key while a nonactivating panel is key.

**Minors for S38:**
- Re-insert Close Windows at `indexOfItem(quit)` rather than just checking that it exists. Re-run the patch after `rune::set_labels`, or keep the process name.
- Build the glyph as one 18×18 pt `NSImage` with 1x, @2x and @3x reps.
- Give the status item an owner type whose `Drop` removes it.
- Keep `kudzu-appkit` free of `rune` and `kudzu-*`. Use per-item `Rc<dyn Fn()>` closures, cloned out of the `RefCell` before calling. Callbacks only queue work.
- Use `NSVariableStatusItemLength`, and pass `mtm` down.

**S03, S70 and S72:** `Info.plist` must leave `NSSupportsSuddenTermination` and automatic termination off, so logout runs `on_quit`. S72 verifies logout with a hook that writes to a file.

**Owner's live checklist (about 5 minutes)**, run with `cargo run -p menubar -- --probe` from `apps/desktop-v2`, terminal visible:
1. "Dev" and the glyph appear in the menu bar, with no Dock icon or window. Log: `S10 status item installed…`.
2. Hover the item: the tooltip is "Kudzu S10 spike". Click it: Open Kudzu…, Close Windows, Show launcher, Swap glyph, Dock on, Dock off, Quit Kudzu.
3. Before ever turning the Dock on: click another app, choose Show launcher, type, then ⌘A, ⌘C, ⌘V, ⌘Z work. ⌘Q does nothing, which confirms Major 1. Close the panel from the status menu.
4. Swap glyph: the paused glyph shows. Log: `alternate=true`.
5. Open Kudzu…, then wait 1 s: the Dock icon and main appear. The app menu reads About, Services, Hide ⌘H, Hide Others ⌥⌘H, Close Windows ⌘Q, Quit Kudzu with no key, and no Show All.
6. In main's field, ⌘A, ⌘X, ⌘V, ⌘C, ⌘V, ⌘Z and ⇧⌘Z all work.
7. ⌘Q with main focused: main hides and the Dock icon goes. Log: `S10 close windows`. The status item stays.
8. Dock off, after promotion: click another app, Show launcher, ⌘Q hides the panel. Log: `S10 close windows`. ⌘C and ⌘V work in the panel.
9. Dock on with no window, then click the Dock icon. Log: `S10 on_reopen visible=false`, and main appears. Click it again. Log: `visible=true`.
10. Close main, then Dock right-click → Quit. Log: `S10 on_quit`, then it exits and the item is gone. No "event loop returned" line, which is expected.
11. Relaunch, then Activity Monitor → Quit, not Force Quit. Log: `S10 on_quit`.
12. Relaunch, Open Kudzu…, then app menu → Quit Kudzu with the mouse. Log: `S10 on_quit`.
13. Relaunch, status menu → Quit Kudzu. Log: `S10 on_quit`, then `S10 event loop returned`, exit 0.
