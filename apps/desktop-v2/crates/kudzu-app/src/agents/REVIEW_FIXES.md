# S32 independent-review corrections

All source paths below are relative to `crates/kudzu-app/src/agents/`.
Changes stay inside S32 Owns. No dependency, engine, frozen reference, permission,
installed-app or real client/config changes were made.

## Findings and regression evidence

| Finding | Red test observed before the fix | Fix and location |
| --- | --- | --- |
| B1 | `blocking_clients_survive_idle_repeated_and_large_messages`: EOF after the 50 ms idle. | Blocking accept, explicit blocking accepted streams, private connect wake: `socket.rs:62`, `socket.rs:80`, `socket.rs:181`. The test sends several requests 300 ms apart and request/reply bodies over 64 KiB. `large_request_and_reply_round_trip_through_real_bridge` additionally exercises a 120 KB create/get over the actual bridge. |
| M1 | `oversized_lines_receive_invalid_request_and_disconnect`: -32700 instead of -32600. `sixteenth_client_is_last_admitted_and_finished_clients_are_reaped`: client 17 received a response. `stalled_reader_is_disconnected_by_write_timeout`: read remained blocked. | `socket.rs:14` defines 1 MiB/16 clients/30-second read inactivity/2-second reply limits. `socket.rs:193` uses bounded reads; `socket.rs:241` joins finished clients on every accept pass; `socket.rs:253` enforces a total reply deadline, including partial writes. |
| M2 | `accepts_again_after_descriptor_exhaustion_and_transient_errors`: ConnectionRefused after injected EMFILE. | `socket.rs:102` retries EMFILE/ENFILE, Interrupted, ConnectionAborted and WouldBlock with 25 ms back-off. `socket.rs:116` publishes stopped state on exit and unpublishes the owned endpoint. `fatal_accept_failure_publishes_stopped_status` covers the terminal path. `service.rs:89` can restart a stopped listener. |
| m1 | `shutdown_and_drop_remove_only_the_socket_they_bound`: shutdown hung after the public path was replaced, because its wake connected to the replacement. The isolated hanging test was terminated before changing the code. | `endpoint.rs:19` binds in a newly created 0700 directory, applies no-follow 0600 permissions, then publishes a hard link. ConnectionRefused is retried before declaring an unchanged inode stale. `endpoint.rs:69` checks device/inode before removal. The private endpoint wakes the original accept even if its public name changes. `socket_binding_rejects_symlinks_and_publishes_only_private_mode` verifies symlink rejection and both modes. |
| m2 | `tauri_clean_template_fields_survive_get_update_round_trip`: clean-template default P@ssw0rd! became a redaction marker. `protocol_errors_include_bounded_escaped_names_and_valid_kinds`: missing name. | Removed the extra recursive result scan from `tools/mod.rs:175`; preserved the original Tauri body/field redaction in `tools/snippets.rs:95`. Errors quote at most 64 input characters with JSON escaping in `mod.rs:225`, used by `mod.rs:159` and `tools/mod.rs:190`. `tools/clipboard.rs:46` restores the valid-kind list. `tools/mod.rs:74` documents the intentional stricter current-clipboard policy. |
| m3 | `production_timeout_reports_slow_app_without_replaying`: production bridge output said isn't running after a real timeout. | `bridge.rs:12` routes production through the timeout-injectable path. `bridge.rs:53` distinguishes timeout from offline and never retries a timed-out read or write. Timeout reports that Kudzu did not answer and the request may have completed. EOF retains one reconnect/resend. The new test injects 50 ms into that same production path and verifies that no second connection/request is made. |
| m4 | `agent_records_use_the_injected_identity_and_clock`: random snippet id instead of deterministic id 100. | `service.rs:54` accepts IdSource alongside Clock. `tools/mod.rs:229` reads the supplied Clock; `tools/snippets.rs:389` and `tools/groups.rs:79` use supplied ids, with no random/wall-clock record constructors. Create and update still use one library transaction. S40 must supply the same shared IdSource as the library. |
| m5 | No new behavioural regression test: this finding requests a source-layout refactor. Existing protocol tests cover the moved functions. | `tools/args.rs:1` and `tools/groups.rs:1` own argument parsing and group operations. `tools/mod.rs` is 355 lines, down from 531. No artificial line-count assertion was added. |
| m6 | `fixture_directories_are_absolute_and_under_the_workspace_target`: relative package-local target path. | `test_support.rs:23` roots fixtures under the absolute workspace target. `test_support.rs:121` provides short relative socket spellings for the Unix path-length limit without changing cwd. `tests.rs:104` reuses one Harness per Fake; `fake_reuses_one_service_across_messages` verifies that reuse. Initial fixture timestamps are deterministic too. |
| m7 | `installed_skill_is_private_atomic_and_replaces_symlink`: installed file mode was 0644. | `setup.rs:59` creates a unique regular 0600 file with create_new, writes and fsyncs it, renames over the destination, then fsyncs the parent directory. A symlinked SKILL.md is replaced, never followed; its old target remains unchanged. Failed writes clean up the temporary. |

## Additional acceptance tests

- `idle_access_revocation_closes_the_session_without_another_request` was red with
  WouldBlock. The settings reactor in `socket.rs:132` closes clients and wakes the
  blocking listener. Settings wakers only enqueue work.
- `revoking_access_during_a_request_withholds_its_reply` holds a real client request
  at a test barrier before dispatch, revokes access, verifies EOF with no reply,
  then releases and joins the handler. Production request/reply admission checks
  remain outside the test-only barrier.
- `drop_removes_the_bound_socket`, the existing lifecycle test, and the replacement
  ownership regression cover Drop, explicit shutdown and ownership-safe removal.
- `each_client_has_one_thread_and_keeps_it_for_later_requests` observes actual
  handler thread ids on two live clients and verifies stable reuse on one client.
- `replays_client_protocol_transcript` was red with 3 replies instead of 8 before
  extending the fixture. It now covers initialize, notification silence, tools/list,
  read/write tools, ping, resources/list and resources/read through socket + bridge.

## Implementation choices

- A private hard-linked wake endpoint is necessary for blocking accept plus safe
  public-path replacement. Waking only the public name can deadlock, as the red
  ownership test showed. Both names refer to the bound socket inode; only the
  already-0600 socket is published outside its 0700 directory.
- No-follow chmod uses macOS `lchmod` through a small FFI declaration because std's
  path-based set_permissions follows symlinks. No third-party crate was added.
- The server uses a 30-second read idle timeout and a total 2-second reply-write
  deadline. The bridge keeps its production 30-second request timeout.
- Descriptor exhaustion is injected at the accept seam rather than changing this
  multi-test process's descriptor limit. Real connections verify recovery afterwards.
- The round-trip fixture retains values the frozen implementation leaves alone.
  Old tests that demanded the extra metadata scan were updated to the owner's
  explicit Tauri-parity decision; content-secret and clipboard-secret tests remain.
- Fixture directories are absolute; sockets use an equivalent short relative path
  because this worktree's absolute path exceeds macOS's Unix socket path limit.
- Real Claude registration and recorded real-client transcripts remain S71 live
  checks. The extended checked-in transcript is synthetic, with no user content.

## Validation

All four gates passed: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
and `cargo xtask check-deps`. Workspace results: 1,343 passed, 8 ignored;
all 48 agents tests passed. The final execution report records the commit SHA.
The commit body lists the red tests above. All commands run in the foreground;
no test changes a real HOME, Claude configuration, user socket or system permission.
