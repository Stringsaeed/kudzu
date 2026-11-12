# S61 implementation handoff

Implemented the settings frame, General, Expansion and Apps. No push, GUI launch, native ports, new dependencies or frozen-app changes. Cargo uses `-j 2`. Final verification explicitly relocates `TMPDIR` inside the worktree. Early rig invocations did not explicitly relocate the default temporary directory; this was corrected for the final run.

## Commits

- `aa16a943`: frame and the five separately owned pane stubs.
- `e4a982ef`: General and the Expansion view.
- `444bfa49`: Expansion tests and the settings sync poll hook.
- The final Apps commit also contains the final geometry, output-boundary and regression coverage. Its SHA is in the implementation report / git log.
- Remote-plan synchronization merge commits: `d3cc5296`, `885129bd` (documentation updates), and `774df27a` (S42 sidebar and settings-service updates). The six gates are rerun after the final source merge.

## Acceptance

Paths below are relative to `apps/desktop-v2`. Test names refer to `crates/kudzu-ui/src/main/tests/settings.rs`.

| Accept item | Result and evidence |
|---|---|
| Eight panes in reference order, Up/Down wrap | Done: `crates/kudzu-ui/src/main/settings/nav.rs:8`, `:64`; `settings_frame_has_eight_panes_and_cycles`. |
| 192 pt nav, 48 pt heading, body at most 680 | Done: `crates/kudzu-ui/src/main/settings/mod.rs:73`; `settings_regions_fit_both_themes_and_minimum_viewport` asserts complete frame rects at 1180 × 760 and 900 × 560, both themes. |
| Permissions status, Set up / Review | Done: `crates/kudzu-ui/src/main/settings/general.rs:13`; `general_permission_copy_covers_missing_ready_and_blocked_states` asserts exact visible copy for missing access, ready, and access granted but expansion blocked. |
| Shortcut recorder, cancellation, exact error copy and rollback | Done: `crates/kudzu-ui/src/main/settings/general.rs:106`, `:125`; `general_controls_commit_settings_and_hotkey_collision_rolls_back`, `shortcut_taken_rolls_back_registration_and_shows_exact_error`, `prefix_and_recorder_ignore_ime_composition`, `recorder_shows_its_prompt_and_escape_preserves_the_shortcut`. Calls `HotkeySlots::set` with `shell.registrar()` in the UI handler. Errors also appear in the existing toast host. |
| iCloud switch and S63 status slot | Done: `crates/kudzu-ui/src/main/settings/general.rs:41`, `crates/kudzu-ui/src/main/settings/sync_status.rs:4`; `general_controls_commit_settings_and_hotkey_collision_rolls_back` verifies the switch's source of truth. Status intentionally belongs to S63. |
| Appearance | Done: `crates/kudzu-ui/src/main/settings/general.rs:82`; `general_controls_commit_settings_and_hotkey_collision_rolls_back` verifies stored Light and applied appearance. |
| Open at login | Done: `crates/kudzu-ui/src/main/settings/general.rs:45`; `shortcut_capture_expires_and_login_reaction_completes` observes the fake login item. |
| Login notice reaches UI | Done: `crates/kudzu-ui/src/main/settings/general.rs:76`; `settings_reactions_reach_engine_and_login_errors_reach_visible_copy` waits for the notice watch and asserts `login item unavailable`. |
| Show in Dock while open | Done: `crates/kudzu-ui/src/main/settings/general.rs:46`, `toggle_dock`; `dock_reaction_sends_the_committed_setting_to_its_output_port` records `[false, true]` through the same output boundary the UI binds to Rune. The General control test also clicks the switch and verifies its setting. No claim of native Dock QA. |
| Pause | Done: `crates/kudzu-ui/src/main/settings/expansion.rs:24`; `expansion_pause_reads_settings_and_prefix_limit_is_utf16` clicks twice and checks settings immediately. Uses `ExpansionService::toggle_paused`. |
| Prefix at most four | Done: `crates/kudzu-ui/src/main/settings/expansion.rs:14`, `crates/kudzu-ui/src/main/settings/mod.rs:176`; `prefix_field_commits_and_bounds_typed_input`, `expansion_pause_reads_settings_and_prefix_limit_is_utf16`. Four UTF-16 units, matching reference `maxLength`, including supplementary characters. |
| Insert by | Done: `crates/kudzu-ui/src/main/settings/expansion.rs:41`; `expansion_pause_reads_settings_and_prefix_limit_is_utf16` and `settings_reactions_reach_engine_and_login_errors_reach_visible_copy` observe settings and the fake engine's matching config. |
| Informational rows | Done: `crates/kudzu-ui/src/main/settings/expansion.rs:24`; `expansion_pause_reads_settings_and_prefix_limit_is_utf16` and the full Good to know landmark rect. |
| Never-expand chips | Done: `crates/kudzu-ui/src/main/settings/apps.rs:16`, `:31`; `never_expand_picker_callback_flips_the_latest_settings` proves additions/removals; removable, wrapping chips write the settings deny list. The existing shared picker's selection callback uses this same toggle. |
| Per-app rows sorted by name, Auto / Type / Paste / remove | Done: `crates/kudzu-ui/src/main/settings/apps.rs:31`, `crates/kudzu-ui/src/main/settings/expansion.rs:41`; `apps_adds_paste_and_removes_and_sorts_names` asserts row order, insertion and removal. Icons come from `MainServices.apps` through the worker-backed cache. |
| Add defaults to Paste, typed bundle ID | Done: `crates/kudzu-ui/src/main/settings/apps.rs:16`, `:129`; `apps_adds_paste_and_removes_and_sorts_names` supplies a typed ID and asserts Paste. The mounted shared picker provides its existing custom-ID validation and action. |
| Controls write settings; reactions observed | Done: settings tests observe the login item, engine config, theme application and Dock recording sink. Native OS presentation remains outside headless evidence. |

## Scenes and geometry

`settings-general`, `settings-expansion`, `settings-apps` remain in `NOT_YET_BUILT` as requested. Scene seed functions live in `crates/kudzu-ui/src/main/settings/scenes.rs`, use the copied fixture settings/status, and preserve the selected theme. S73 can register these once all whole-window regions exist.

`settings_regions_fit_both_themes_and_minimum_viewport` replaces those comparisons for S61. It asserts every coordinate of the nav and heading at both window sizes. At the captured size, it asserts complete reference rectangles within 2 pt for Permissions, Quick search, Expanding, Good to know, Never expand in, and How to insert per app, in both themes. The iCloud status region awaits S63.

The four initial acceptance tests were authored before their corresponding implementations. The first dependency build overlapped implementation; it did not produce a clean preimplementation behavioral red run. Subsequent runs exposed the offscreen Dock click in the test and measured the row/heading spacing drift; both were corrected. Assertions are unconditional. Async checks wait on source state with a deadline; they do not count frames.

## S62 / S63 and Rune follow-ups

- S62 owns `clipboard.rs`, `agents.rs`, `data.rs`; S63 owns `activity.rs`, `about.rs` and the separate `sync_status.rs` slot. Each has a `view(&MainState, &Theme) -> El` seam. All five pane modules are separate files.
- Exactly one `AppPicker::new` call site: `crates/kudzu-ui/src/main/settings/apps.rs` in `view_with_cx` (search `AppPicker::new` in that file). It switches between exclusions and additions. Caller key: `settings-app-picker`; no tests depend on the picker's fixed internal keys. Update this call with the new unique ID and `.add_label` builder when S55 lands. Current code sets the existing public `add_label` field. One picker is mounted at a time because today's keys collide.
- No picker Escape handling was added to `main/keys.rs`. The caller's close callback suppresses state changes during composition. Scrolled popup placement remains the shared S55 fix described in the brief.
- Recorder, navigation, and text guards consult composition state. Prefix commits defer until composition ends. Successful-shortcut feedback has a clock deadline and a Rune scheduled wake; `shortcut_capture_expires_and_login_reaction_completes` proves expiration. Losing focus cancels recording.
- Native Dock presentation cannot be observed by Rune's current headless tree (already noted by S40); the recording sink proves the emitted request. Native theme/Dock QA belongs to the later native verification steps.

## Shared-file edits

Final shared-source locations: `main/frame.rs:145` (six additive lines), `main/keys.rs:76` (six additive lines), `main/tests/mod.rs:30`, `:92`, `:114`, `:115`, `:142` (registration and fake-registrar helper), `shell/windows/main.rs:48` and `:65` (watch and constructor), `shell/lifecycle.rs:206` and `:229` (sync and timer/focus polling). Paths in this paragraph are under `crates/kudzu-ui/src/`.

The following is the complete S61 shared-file diff before the final upstream merge (the source positions above reflect that merge), including the corrected test-rig helper. The frame's additive settings view hook supplies `Cx` and overlays for the existing app picker; the key hook routes only settings navigation/recording. The shell gains the login watch field/constructor line and two poll lines.

```diff
diff --git a/apps/desktop-v2/crates/kudzu-ui/src/main/frame.rs b/apps/desktop-v2/crates/kudzu-ui/src/main/frame.rs
index be67918c..c77c2a65 100644
--- a/apps/desktop-v2/crates/kudzu-ui/src/main/frame.rs
+++ b/apps/desktop-v2/crates/kudzu-ui/src/main/frame.rs
@@ -134,0 +135,6 @@ fn build_main_card(
+    let screen = if let MainView::Settings(pane) = s.frame.view {
+        settings::view_with_cx(s, cx, theme, pane, overlays)
+    } else {
+        screen
+    };
+
diff --git a/apps/desktop-v2/crates/kudzu-ui/src/main/keys.rs b/apps/desktop-v2/crates/kudzu-ui/src/main/keys.rs
index 17ed69b3..b0524c68 100644
--- a/apps/desktop-v2/crates/kudzu-ui/src/main/keys.rs
+++ b/apps/desktop-v2/crates/kudzu-ui/src/main/keys.rs
@@ -71,0 +72,6 @@ pub fn handle_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
+    if matches!(s.frame.view, MainView::Settings(_))
+        && crate::main::settings::nav::handle_key(s, ev, tx)
+    {
+        return true;
+    }
+
diff --git a/apps/desktop-v2/crates/kudzu-ui/src/main/tests/mod.rs b/apps/desktop-v2/crates/kudzu-ui/src/main/tests/mod.rs
index eb94f63b..8db7c3b2 100644
--- a/apps/desktop-v2/crates/kudzu-ui/src/main/tests/mod.rs
+++ b/apps/desktop-v2/crates/kudzu-ui/src/main/tests/mod.rs
@@ -29,0 +30 @@ mod routes;
+mod settings;
@@ -89,0 +91 @@ pub(super) struct Rig {
+    pub(super) fake_hotkeys: Arc<FakeHotkeys>,
@@ -111 +113,2 @@ impl Rig {
-        let hotkeys: Arc<dyn HotkeyRegistrar> = Arc::new(FakeHotkeys::default());
+        let fake_hotkeys = Arc::new(FakeHotkeys::default());
+        let hotkeys: Arc<dyn HotkeyRegistrar> = fake_hotkeys.clone();
@@ -137,0 +141 @@ impl Rig {
+            fake_hotkeys,
diff --git a/apps/desktop-v2/crates/kudzu-ui/src/shell/lifecycle.rs b/apps/desktop-v2/crates/kudzu-ui/src/shell/lifecycle.rs
index 7ed746e4..30fe3a7b 100644
--- a/apps/desktop-v2/crates/kudzu-ui/src/shell/lifecycle.rs
+++ b/apps/desktop-v2/crates/kudzu-ui/src/shell/lifecycle.rs
@@ -204,0 +205 @@ impl MainState {
+        changed |= crate::main::settings::sync(self);
@@ -226,0 +228 @@ pub(crate) fn poll(s: &mut MainState, _: f64, tx: &mut Tx) -> bool {
+    changed |= crate::main::settings::poll_frame(s, tx);
diff --git a/apps/desktop-v2/crates/kudzu-ui/src/shell/windows/main.rs b/apps/desktop-v2/crates/kudzu-ui/src/shell/windows/main.rs
index 6ac61fd8..927b7ab3 100644
--- a/apps/desktop-v2/crates/kudzu-ui/src/shell/windows/main.rs
+++ b/apps/desktop-v2/crates/kudzu-ui/src/shell/windows/main.rs
@@ -47,0 +48 @@ pub struct MainServices {
+    pub login_notice: kudzu_app::watch::Watch<Option<String>>,
@@ -63,0 +65 @@ impl From<&Arc<Kudzu>> for MainServices {
+            login_notice: kudzu.login_notice.clone(),

```

## Gates

Final foreground run, with `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=4`, and a unique `TMPDIR` under this worktree's root `target/` (outside the nested Cargo workspace). All six exit 0. All 14 S61 settings tests pass.

```text
cargo fmt --all --check
(no output)
cargo clippy -j 2 --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.01s
cargo test -j 2 --workspace
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
cargo xtask check-deps
workspace dependency and containment checks passed
python3 scripts/check-headless.py
All modes and window scripts passed under a filesystem write sandbox, on fixtures.
cargo xtask parity
608 rows, 0 failed, 96 not yet built; report: /Users/saeed/.t3/worktrees/text-expander/t3-6c90e36e-85797d36ac83aa80/apps/desktop-v2/target/parity/report.md
```

The formatting command prints nothing on success. Logs are retained in `target/s61-verified/`. The test line above is the requested final line (the last doc-test suite); the entire workspace command exited 0.

Earlier attempts: placing TMPDIR inside desktop-v2 caused the dependency-checker fixture packages to inherit the enclosing Cargo workspace; moving it under the repository root resolved that without source edits. One merged run timed out in the existing onboarding retry test under concurrent load; the repeated workspace run with four test threads passed it. An isolated retry also encountered a removed temporary directory during linking; the final run used its own unique directory. No tests or production code outside S61 were changed to make these runs pass.

