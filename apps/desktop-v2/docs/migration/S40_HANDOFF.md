# S40 hand-off — shell and composition

S40 (`refactor(desktop-v2): compose the services in Kudzu::new and order Kudzu::shutdown` … `retry the app-menu install until Rune's menu bar exists`) composes every service and adapter into a running app on fixtures: five windows, tray, hotkeys, lifecycle, theme sync. Implemented by Claude Opus; reviewed by Grok (see `reviews/` when it lands).

## Layout

- `crates/kudzu-app/src/assembly/{mod,reactions,router,selection,tests}.rs`: `Kudzu::new`, `Kudzu::shutdown`, the `EngineRouter`, the reactions thread, `SelectionCapture`. `Config` takes a `Host` (ports, locale, HOME, time zone, local time); `kudzu_system::host` builds the native one and `Host::fixtures` the fake one.
- `crates/kudzu-system/src/{lib.rs,host.rs,unavailable.rs}`: `Ports::macos`, and stand-ins that report `Unavailable` when unbundled or a constructor fails (none pretends to succeed).
- `crates/kudzu-ui/src/shell/**`: `Shell`, the five window states (`shell/windows/`), tray model, hotkey registrar, lifecycle, routes (`shell/routes.rs`), theme sync, placeholders. `crates/kudzu-ui/src/actions/mod.rs` (the `Action` trait and registry) and `crates/kudzu-ui/src/focus.rs` (`focus::Focus`, the pane focus model).
- `src/main.rs`, `src/compose.rs`: launch mode → identity → paths → ports or fixtures → `Kudzu::new` → windows → wakers. `src/` is at 499 of its 500-line budget; add nothing there.

## How the reactions are wired

- library or settings → engine index rebuild: expansion takes the library and settings watches.
- library → sync uploads: sync takes the library.
- snippets removed → form memory pruned: forms takes the library watch.
- `clipboard_*` settings → capture: the clipboard service's poll reads settings.
- `icloud_sync` → sync start/stop: sync takes the settings watch.
- agent settings and `launch_at_login`: a reactions thread in `assembly` runs `agents.apply()` and the login item, never on the UI thread.
- Engine events (router in `assembly`): `expanded` → `library.record_expansion` only; `form_requested` → `FormService::requested`; `activity` → the outcome watch and the Activity ring; `trigger_lost` → expansion, then Activity once.
- Sync NeedsUpdate → `updates.check(Trigger::Scheduled)`, once per transition (`Scheduled` is the quiet trigger).

## For the screen steps

- **S41 (main window frame).** `MainState` is in `shell/windows/main.rs`, routes in `shell/routes.rs`. Replace `placeholder::main` and `MainModel.draft`. Every hide or quit path calls `MainState::commit_drafts()`, so hook the editor's save there. Add services to `MainServices` only. Use the `Action` trait (`actions/mod.rs`) and `focus::Focus`.
- **S56 (Quick Search).** Keep `toggle()` (the hotkey calls it) and `dismiss(restore)` as the only way the palette hides. Show is deferred one turn (the S11 caveat's candidate fix): verify the first visible frame. Placement is not done yet. Blur hides through `on_focus(false)`.
- **S57 (form).** The form shows itself when a request is pending and hides when it clears. Focus reports are numbered from 1 but currently send `after: None`: add the `NSApp.currentEvent` stamp and key code, and pass them as `FormSubmission.event` too. Settle restoration tasks with `shell.settle`.
- **S60 (picker).** `begin_pick` runs on a worker; call `PickSession::paste` on a worker after hiding.
- **S61 (settings).** Shows `MainModel.hotkey_error` and `Kudzu::login_notice` ("login item unavailable"); calls `HotkeySlots::set` with `shell.registrar()` on the main thread.
- **S66 (permissions).** The restart marker is read on a worker at start and opens the window. Escape or close hides it and shows `main`. The window list sent to the engine hard-codes "no click-through"; the guide overlay must report `passes_clicks_through: true`.

## Decisions (orchestrator)

- The writer debounces 250 ms and retries every 5 s.
- The tray tooltip uses the build's identity name ("Kudzu Next — Expanding" for Next); the Tauri app always wrote "Kudzu — …".
- An untracked task (an import, a selection capture) still running after the drain only reaches memory; documented in the assembly header.
- A failed `Kudzu::new` must log and exit cleanly, not panic (follow-up).
- `Ports.hotkeys` is unused (the shell is the registrar); remove later.

## Rune gaps (drafts, not filed)

- Headless scripts can't deliver a system quit that runs `App::on_quit`.
- `--tree` doesn't report Dock icon or activation-policy requests.
- No menu-bar-ready hook; the app-menu patch retries on each poll.

## From the S40 review (Grok)

The review's majors (startup panic, lock outcomes, `Writer` timeout installing an update, the feed wait) and minors are being fixed in a follow-up that touches only assembly, `compose`/`main`, `kudzu-system` and `shell/`. Extra notes for the screens:

- **S41.** `MainModel.draft` is a stand-in (`shell/windows/main.rs:47-49`); hide and quit commit through `library.create_snippet`. Extend `MainServices` there. The wakers already cover settings, library, expansion and updates.
- **S56.** Opening queues `show` on the next update (the S11 same-turn fill). `dismiss` is the only hide path; dropping an in-flight capture abandons the `Task` and the session's `Drop` lifts suppression. Read the `outcomes()` baseline before inserting. The shell has no exhaustive `match` on `InsertionOutcome`, so the palette must handle `EngineStopped` itself.
- **S57.** The form shows itself from `pending()` (`shell/windows/form.rs:86-111`); no blur dismissal. `PanelFocus.after` is always `None` (`form.rs:77-81`): fill it from `NSApp.currentEvent`. `SubmitStartError.restoration` is not settled yet; `close_windows` only settles cancel (`shell/mod.rs:103-108`). Seq starts at 1 per request. The empty `set_kudzu_windows` push already runs before any form.
- **S60.** Hide before `PickSession::paste`, on a worker (about 850 ms). Turning recording off already hides the picker and unregisters the clipboard chord.
- **S66.** `take_pending_restart` already runs off the UI thread (`shell/windows/permissions.rs:72-74`). `WindowList` hard-codes `passes_clicks_through: false` (`shell/window_list.rs:43-44`): the guide overlay must be pushed as click-through, or every click on it interrupts typed forms. The companion's `begin` stays off the UI thread (S30).
