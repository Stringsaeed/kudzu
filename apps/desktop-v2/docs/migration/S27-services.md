# S27 — Expansion, forms and activity services

Implemented on `t3/expansion-form-activity-services` in `/Users/saeed/.t3/worktrees/text-expander/t3-0db992d0-5d236e6eed8686ea`.
Implementer: `gpt-6-astra`, xhigh. No delegated or independent review was run by this implementation agent. The final hand-off supplies the commit SHA; this report belongs to that commit.

## Public surfaces

All services are thread-safe. Watches publish immutable snapshots through `Committed<T>` and notify after releasing the owning state lock. Content-bearing requests/results have no new serialization or logging path.

- **`expansion::ExpansionService`**: `new`, `status`, `status_changed`, `set_paused`, `suppress`, `feed`, `stop_feed`, `begin_quick_search`, `insert`, `preview`, `preview_snippet`, `copy_rendered`. `FeedState` identifies the library/settings revisions applied and reports worker failure. Library usage is omitted from `EngineLibrary`; unchanged feeds do not call reload. Pause persists through `SettingsService` and is applied on the feed worker.
- **`QuickSearchSession`**: `target`, `selection`, consuming `insert` and `restore_target`; drop releases its `SuppressionGuard`. Capture runs on a Task before a session becomes available to show. Selection is captured only when a target exists and a parsed snippet uses the selection tag. The caller must take the session on every hide path.
- **Rendering**: `RenderEnvironment` injects pasteboard, clock, locale, conversion of each instant to local time, and `HistoryReader`. `PreviewContext` carries root snippet ID, formatting, selection and form values. Rendering uses core's renderer and nested snippet/formatting resolution; copy writes plain text plus optional HTML. Pasteboard/target/engine I/O runs on Task workers. No new third-party dependencies.
- **Health**: `sidebar_health`, `tray_summary`, `Health`, `HealthCopy`. These preserve the reference wording and pause → Secure Input → permission priority, including D5's known distinction between visible health wording and posting permission. They are not insertion-readiness checks.
- **`forms::FormService`**: `open` (startup local JSON I/O), `new` (injected persistence), `pending`, `requested`, `preview`, `submit`, `cancel`, `close_windows`, `panel_focused`, `saved`, `pruned`, `stop_pruning`. `FormRequest` contains token, snippet identity/name, fields, initial values, target, caret and captured selection. Tokens are nonzero, strictly increasing across the service/router lifetime; repeated or stale requests and completions do nothing. `Submission` separates the memory acceptance revision/queue error from the typed insertion admission result; native worker outcomes arrive later through `EngineEvents`. Asynchronous write failures remain in `saved()`.
- **`activity::ActivityService`**: `new`, `watch`, `record`, `clear`, `remedy`, `apply`. `RemedyState` is a current action or `FixedSince`; `AppliedRemedy` is a committed revision, `FixedSince`, `Unavailable`, or a typed UI destination/help request. Only resume and enable-snippet mutate domain state. Applying a stale suggestion rechecks the retained event and current state. Missing records are not recreated. The ring is process-local, has no persistence target or agent/sync interface, and stores only an exact stored trigger, never raw input or rendered content.
- **Engine port extension**: `ports::engine::InsertionError` preserves interrupted, paste-unavailable, event-creation, posting-denied, target-gone, pause, Secure Input, global exclusion and missing-snippet failures. `ExpansionEngine::insert` and `FormChannel::submit_form` return it. `set_suppressed` is explicitly synchronous, a matching-state update (flag plus matcher reset), and callback-free so guard transitions can be serialized safely.

## Acceptance evidence

The tests exercise service APIs and Watches against port/storage fakes. The JSON reopen test uses an owned temporary directory. No test reads actual user Activity, clipboard, form memory or library content.

| S27 Accept item | Result | Evidence |
|---|---|---|
| Reload only when engine matching data changes, not usage | PASS | `expansion::tests::engine_feed_reacts_to_matching_edits_but_not_usage_or_unrelated_settings`: initial and content-change reloads; usage and typing-speed changes do not reload. `tests::events` checks a post-insertion usage callback without a reload. |
| Pause persists and reaches engine | PASS | Same feed test: settings commit reaches engine/status and writer shutdown makes its revision durable; pause itself does not rebuild the index. |
| Guards count | PASS | `overlapping_guards_keep_matching_suppressed_until_the_last_owner_leaves`, plus the concurrent form completion test with another guard still alive. |
| Selection capture is conditional and finishes before show | PASS | `quick_search_captures_only_needed_selection_before_returning_a_session`: no selection read without a tag, captured target/selection available in the completed session, target restoration, failure cleanup. |
| Initial values: remembered → default → first choice → empty | PASS | `forms::tests::memory_uses_valid_fallbacks_caps_smallest_uuid_and_prunes_deleted_snippets`; reuses core's validated field fallback policy. Core `form_memory_validates_and_round_trips` additionally covers invalid remembered choices and valid remembered choices. |
| Memory cap 200, smallest-UUID eviction, prune on delete | PASS | Same test starts with 200 deterministic IDs, submits ID 201, checks ID 1 eviction and ID 2 retention, then delete/prune/restore proves remembered values do not return. Core `form_memory_is_bounded` additionally asserts the exact retained count. |
| Remember before submit; failed memory write does not block insert | PASS | `memory_is_accepted_before_submission_and_a_disk_failure_does_not_block_insert`: the fake's before-submit callback opens a new request and sees the just-remembered values; the failing writer reports Encode while the insertion succeeds. `submitted_memory_reopens_from_the_local_json_file` verifies real file durability/reload. |
| Stale/repeated token does nothing | PASS | `tokens_complete_once_replacement_and_close_release_only_their_guard` and `concurrent_submit_and_cancel_consume_one_token_and_preserve_another_suppression_owner`: eight competing callers produce exactly one completion. |
| Cancel reactivates original target | PASS | Token lifecycle test asserts the target activation and exactly one cancellation per replaced/closed request. |
| Submit/cancel/replacement/close release their guard | PASS | Token lifecycle and concurrent-completion tests; `dropping_the_form_service_cancels_and_clears_a_retained_watch` additionally proves teardown clears retained pending snapshots. |
| Panel focus reaches the engine separately from suppression | PASS | Token lifecycle test rejects stale focus; `interrupted_typing_or_click_is_rejected_even_while_suppressed_and_focus_is_separate` forwards focus and surfaces `Interrupted` without a successful submission. |
| Ring: 100 newest-first, monotonic IDs, no persistence | PASS | `activity::tests::ring_is_newest_first_capped_and_ids_survive_clear_without_content_or_disk_access`: 105 entries retain IDs 105..6; clear then append produces 106. Unrecognized trigger input is stripped. The service has no persistence/log/sync/agent sink; core Activity types are not serializable in production. |
| Every outcome has its explanation/remedy; resolved state says Fixed since | PASS | `every_outcome_retains_its_explanation_fix_and_current_state_resolution`: all outcomes, all app-rule sources, both permissions, all insertion failure codes and both tap-disable causes; exact labels/fixes and state resolution. `remedies_recheck_current_state_apply_only_typed_edits_and_do_not_resurrect_records` checks application and stale suggestions. |
| Explicit insertion preserves RUL-9 | PASS | `explicit_insertion_preserves_rul9_and_surfaces_typed_paste_refusal`: a disabled snippet in a disabled group with an empty allow-list is forwarded/accepted; global exclusion and pause block; typed paste refusal is preserved. The service adds no matcher-eligibility filter. |

Additional deliverables: `previews_resolve_clipboard_history_selection_forms_and_copy_plain_plus_html` covers rendering/copy; the form interruption test covers captured-selection preview and stale preview tokens. `sidebar_and_tray_health_keep_reference_priority_and_wording` covers health copy. `expansion::tests::events::typed_engine_callbacks_route_to_service_snapshots_and_post_insertion_usage` demonstrates S40's routing contract with actual services: submit acceptance does not increment usage; an `expanded` callback calls `library.record_expansion`, with per-device usage and no index rebuild.

Tests were added before their implementation slices and run red (missing public surfaces), then green. The retained-Watch teardown test additionally reproduced an assertion failure against the first implementation before the fix. The Activity explanation/fallback rules reuse already-tested core functions; their service acceptance matrix verifies the integration.

## Gate

- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace`: PASS — 822 passed, 0 failed, 2 existing performance tests ignored; includes 101 application tests (17 added by S27).
- `cargo xtask check-deps`: PASS — workspace dependency and containment checks passed.
- `git diff --check`: PASS, including the staged change before commit.

The first default-profile workspace test build stopped before tests while archiving Rune with `No space left on device (os error 28)`. Cargo cleaned only this worktree's disposable dev artifacts. The retry uses `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2` to reduce disk usage. The workspace selection, test code, assertions and test runner are unchanged. Clippy had already passed on the same source in the default dev profile. The two pre-existing ignored tests are `construct_5000_two_kb_entries` and `search_5000_p95_under_five_ms`, both documented for an idle runner; S27 did not change their ignore annotations. Local gate logs: `/tmp/kudzu-s27-clippy.log`, `/tmp/kudzu-s27-workspace-tests.log`, `/tmp/kudzu-s27-deps.log`; the failed initial build is retained in `/tmp/kudzu-s27-workspace-tests.initial.log`.

## Integration edits outside Owns

Allowed by §4's consumer-owned port/fixture rule:

- `crates/kudzu-app/src/ports/engine.rs`: typed insertion result, suppression/callback contract, form token/focus/interruption contract. No engine-crate change.
- `crates/kudzu-app/src/fixtures/engine.rs`: typed insertion failures, explicit eligibility contract, focus-independent interruption, and a callback outside locks for ordering tests.
- `crates/kudzu-app/src/fixtures/apps.rs`: selection-read and activation observations.

This report is the requested delivery artifact. No library/settings/foundation/assembly implementation, Cargo dependency/lockfile, existing Tauri app, release configuration or other worktree was edited. The designated warm-cache source was copied into this worktree before building. Builds ran one at a time in the foreground.

## Downstream obligations

### S35 — engine adapter

Implement both `ExpansionEngine` and `FormChannel`; preserve `InsertionError` codes rather than collapsing them into a string or generic I/O error. Implement nonzero, monotonically increasing form tokens across engine restarts within a router lifetime. Cancellation and focus must be token-conditional, so an old task cannot affect a replacement form. `set_suppressed` must be a synchronous matching-state update with no OS I/O or callbacks. `cancel_form`/`panel_focused` must remain fast and do no target I/O on the caller's thread.

The engine owns real RUL-9 eligibility, insertion-method resolution, target activation/wait/settle, actual interruption and D14 recovery. A typed form interrupted by key-down or click outside the key form panel must be refused even while matching is suppressed. Clipboard snapshot/write must precede deletion; refused paste must retain or restore the trigger. Emit `expanded` only after actual insertion, including successful form insertion; emit content-free Activity separately. An accepted insertion can merely open a form. No UI, permission, native target, real clipboard recovery or live typing claim is established by these service/fake tests.

### S40 — composition and shutdown

Build and own `EngineRouter`; the production router is intentionally not added here because S40 owns assembly. Route status to `expansion.status_changed`, successful expansion to `library.record_expansion` (handle its typed error), form requests to `forms.requested`, Activity to `activity.record`, and history reads to the clipboard service. The test router in `expansion/tests/events.rs` uses weak service owners to avoid an engine/router/service cycle.

Compose one shared `ExpansionService`, and pass that same instance to forms, palette and picker suppression owners. Wait for the initial feed before enabling insert actions. Subscribe before the initial Watch read. Inject actual local time conversion on each render, system locale and the clipboard service's history reader. Only local product UI receives Activity and pending forms; never serialize or give them to agent tools. Persist only form memory locally; it must not enter library sync or agent output.

Close window task admission, consume palette/picker sessions, call `forms.close_windows`, and settle all outstanding Tasks (including target activation/capture/insert). `Task` drop is not cancellation. Stop `expansion.stop_feed` and `forms.stop_pruning` before draining the shared writer; stop the engine/other producers in the prescribed shutdown order. Treat writer timeout/errors as incomplete durability. Use the supplied health wording but the actual permission/engine fields for readiness. Handle feed/pruner worker errors instead of ignoring their Watches.

### S56 — Quick Search

Hold `Option<QuickSearchSession>`. Show only after `begin_quick_search` completes successfully; discard an obsolete capture result. Take the session on every hide path, even though the Rune window remains cached. Hide before consuming `session.insert`; blur drops it, explicit dismissal can consume `restore_target` unless main owns focus. Pass captured selection to `preview_snippet`; copy its rendered result with `copy_rendered`, then perform the UI's 260 ms feedback/dismissal. Do not add a snippet/group eligibility filter to explicit insert. Surface typed task/insertion failures; acceptance is not an expansion count.

### S57 — form panel

Pull `forms.pending()` and use its token, first-definition fields, initial values, caret and captured selection/target. Do not own a second pending form or a second suppression flag. Forward key-window focus changes using the current token; blur does not cancel. Use `forms.preview(token, values)` and discard results for an old token. Hide before submit/cancel, consume the result once, and handle `Interrupted`/typed failures. Form values remain remembered even when insertion is refused, matching submit-before-engine behavior. Report memory durability through `saved()`; a successful insertion does not prove the memory write succeeded.
