# S32 application-layer MCP adapter

`AgentsService` owns the listener and its client threads. Protocol tools call concrete
application services; there is no production Host trait, IPC table, or UI dependency.
Library validation, group creation, collision detection and the write happen in one
`LibraryService::transact`. Client threads perform that work off the UI thread.

Clipboard tools consume only `agent_query`, `agent_item` and `agent_history_text`.
Tool limits remain independent of service limits: list defaults to 20 and caps at 100,
list text caps at 320 characters, and full/history text caps at 50,000 characters.
Hidden items omit length and line count. Explicit clipboard not-secret overrides retain
Tauri's intentional sharing behaviour. This adapter has no logging calls.

## Acceptance evidence

All four workspace gates passed: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
and `cargo xtask check-deps`. The red-to-green checks covered the initial absent
adapter and the metadata secrecy regression.

| Accept item | Result and test |
| --- | --- |
| 16 original protocol tests on services | Pass; names below. Library and settings persistence use in-memory targets; clipboard storage uses isolated worktree temporary directories. |
| Two Tauri agent.rs tests ported | Pass; names below. |
| Socket 0600, live duplicate refusal, stale replacement, access revocation, removal | Pass: `socket_lifecycle_access_and_setup_are_isolated`. Implementation starts a named thread for each client and shutdown closes and joins all clients. |
| Bridge newline framing, notification silence, actionable offline listing | Pass: `offline_bridge_frames_lines_and_ignores_notifications`, `relays_through_a_socket_and_falls_back_offline`. |
| One reconnect | Pass: `reconnects_once_after_a_closed_connection`. |
| 30-second timeout without replay | Pass: `timeout_never_replays_a_request_and_the_next_call_reconnects`; production duration asserted at 30 seconds, socket timeout exercised with an injected 50 ms connection timeout. |
| Independent write/clipboard gates, recording required | Pass: `service_gates_are_independent_and_recording_is_required`. |
| Atomic edit and UI race | Pass: `racing_agent_and_ui_edits_preserve_both_and_collision_rolls_back_group`; both writes survive and failed create/update collisions leave revision and groups unchanged. |
| Recorded real-client transcript replay | Fail: no real-client recording was available and tests must not contact a real client. Synthetic replay passes for checked-in `fixtures/session.jsonl` through the real socket and bridge: `replays_client_protocol_transcript`. This is a synthetic protocol fixture, not a capture from an installed real client; real capture remains S71. |
| Quoted executable, user-scoped Claude command, identity name, JSON config | Pass: `quotes_paths_for_the_shell`, `socket_lifecycle_access_and_setup_are_isolated`. Identity name and CLI arguments are supplied by composition. |
| Skill missing/current/outdated | Pass: `socket_lifecycle_access_and_setup_are_isolated`. |
| Skill resource listed/readable | Pass: `serves_the_skill_as_a_resource`. |
| Secret safety | Pass: `clipboard_withholds_secrets`, `clipboard_follows_the_users_corrections`, `lists_and_searches_snippets_without_leaking_secrets`, `field_defaults_and_options_are_redacted_too`, `secrets_never_reach_results_errors_or_logging_calls`, `snippet_metadata_and_unknown_field_names_are_redacted`. |
| Independent tool caps | Pass: `tool_clipboard_limits_remain_independent_of_service_caps`. |

The original 16 MCP tests, preserving their names:

1. `initialize_negotiates_the_protocol`
2. `serves_the_skill_as_a_resource`
3. `notifications_and_responses_get_no_reply`
4. `tools_follow_access`
5. `offline_lists_tools_and_explains_calls`
6. `lists_and_searches_snippets_without_leaking_secrets`
7. `reads_a_snippet_by_trigger`
8. `expands_with_values_defaults_and_a_scrubbed_clipboard`
9. `creates_snippets_and_groups`
10. `rejects_bad_creates_without_saving`
11. `warns_about_shadowed_triggers`
12. `updates_only_what_is_passed_and_protects_secrets`
13. `expand_leaves_out_key_presses_and_guards_clipboard_history`
14. `clipboard_withholds_secrets`
15. `clipboard_follows_the_users_corrections`
16. `field_defaults_and_options_are_redacted_too`

The two ported Tauri agent.rs tests:

- `setup::tests::quotes_paths_for_the_shell`
- `bridge::tests::relays_through_a_socket_and_falls_back_offline`

## S40 composition

Supply `AgentConfig` with socket, HOME, executable, identity's registration name,
CLI arguments, version, locale and optional bundled skill directory. Do not discover
HOME or the executable inside this service. The Dev identity is `kudzu-next` until
cutover; retain `--mcp --dev` when composition selects the Dev endpoint explicitly.

Construct with shared `LibraryService`, `ClipboardService`, the settings Watch and
Clock. Run `apply()` initially and from the settings reactor, off the UI thread.
Reconcile on every settings change, including retries after listener startup failure.
The request path independently checks current access, write permission, clipboard
permission and recording. Call `shutdown()` before engine/clipboard shutdown and
before draining writers. Shutdown is synchronous and idempotent.

Dispatch `--mcp` before starting Rune; call `bridge::run(stdin, stdout, socket, version)`
with paths resolved by identity. Bridge I/O runs on the CLI thread. Tool calls do not
perform insertion; template expansion renders text and reports skipped actions.
Current clipboard substitution uses S28's history slot zero, which yields text only
when the newest recorded entry still matches the current clipboard; it never falls
back to stale recorded content or reads the pasteboard directly.

## S64/S65 settings UI

Read `status()` for listener state. Read `setup()` on a worker for `AgentSetup`, which
contains the command, arguments, quoted Claude command, JSON configuration, skill
path, `SkillState` and optional bundled skill directory. Run `install_skill()` on a
worker, refresh setup afterwards, and display `AgentError` without raw I/O details.
Change the three typed agent settings through `SettingsService`; the S40 reactor
applies listener changes. No client configuration is modified automatically.

## S71 live checks

- Register a composed Kudzu Next build using the displayed user-scoped Claude command.
  Capture initialize/tools-list/tools-call transcripts from that real client and replay
  sanitized recordings alongside the synthetic fixture.
- Confirm `--mcp` and `--mcp --dev` resolve the intended identity without starting Rune.
- Verify revocation in an already-connected client, offline guidance, restart reconnect,
  privacy with real password-manager copies, current clipboard substitution, and skill
  installation into an explicitly selected temporary HOME.
- Verify local date/time rendering and locale against the reference in a composed build.

No real client, user HOME, user socket, system permission, or installed app was touched.

## Scope and questions

No edits outside S32 Owns. Core's module declaration and README removal are explicitly
owned by S32. No new dependencies, manifest changes, engine changes or reference edits.

The reference SnippetRecord has no manual per-snippet secret flag. Detection-based
snippet redaction and protected content updates are preserved; explicit secret override
is a clipboard capability. A future manual snippet secret flag would need a separate
library-model change. Real-client recorded transcript acceptance awaits S71, as these
tests are prohibited from touching a real MCP client.
