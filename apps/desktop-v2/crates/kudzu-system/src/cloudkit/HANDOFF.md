# S37 hand-off

`CloudKit` implements S33's `CloudStore` contract at `3cfeb335`. `wiring.rs` is the
only connection to `CloudEvents`. No port, fake, or service file was edited here.
Container, zone, payload field and record types come from `kudzu_core::sync`.

## S40 integration

Construct one `Arc<CloudKit>` with `CloudKit::new()` and supply it as
`Arc<dyn CloudStore>` to the sync service. Construction does not access CloudKit.

The adapter forwards `state(generation: u64, bytes: Vec<u8>)` in delegate order,
never sends empty bytes, and reports typed `AccountChange::{SignIn, SignOut, Switch}`.
It queries availability after start and on every account change. In particular,
`CKAccountStatus::CouldNotDetermine` becomes `CloudAvailability::CouldNotDetermine`;
the service decides to retain its current status. There is no checkpoint pull.

The sync worker owns port calls; event receivers only enqueue. The service owns
the Dev identity gate, timers, merges, and ordered durable state writes. Native
stop drains admitted callbacks and rejects later ones. Restart waits up to 30
seconds for CloudKit cancellation; a timeout keeps the retiring engine and refuses
to create an overlapping one.

## Local evidence

- 20 offline adapter tests pass; one signed test is ignored. Tests cover native
  record/batch translation, pending upserts, conflicts and errors, zone/account
  effects, ordered event forwarding, all account statuses and stop fencing.
- All five gates pass: formatting, workspace Clippy with `-D warnings`, workspace
  tests, dependency/containment checks, and the x86_64 `kudzu-system` check. The host
  is aarch64; Intel runtime verification remains a signed manual test.
- The dependency scanner initially rejected `r#type`. Its regression test failed
  before the fix and passes now; raw include/path syntax still cannot escape.
- An initial workspace run hit two existing PAC tests returning `Unavailable`.
  Their focused recheck and subsequent normal workspace runs passed, including
  the final run after S33 integration; no PAC code changed.
- Tests used a temporary directory inside this git worktree, outside the Cargo
  workspace so fixture workspaces remain independent. No container or sync engine
  was created by the executed tests; only local CloudKit record/batch objects were used.

No schema or entitlement file changed. There are no CloudKit binding gaps or
upstream issue drafts. The Objective-C `msg_send!` only initializes our delegate's
superclass. No port-contract questions remain.

## Edits outside Owns

Paths below are relative to `apps/desktop-v2`.

| File | Reason |
|---|---|
| `Cargo.toml` | Select only the approved `objc2-cloud-kit` engine features; default features would add unused Contacts bindings. |
| `crates/kudzu-system/Cargo.toml` | Enable the already-approved CloudKit and block2 workspace dependencies for this adapter. |
| `Cargo.lock` | Record those dependency edges and the block2 feature; no new package is added. |
| `xtask/src/check_deps.rs` | Recognize the generated API's valid `r#type` identifier instead of rejecting it as an invalid raw string. |
| `xtask/src/check_deps/tests.rs` | Prove raw identifiers work while raw include/path syntax cannot bypass containment checks; verified red before green. |

## S71/S72: signed manual verification

Do these only in the owner-authorized production-identity session with a matching
provisioning profile and a backed-up test library. Quit the installed production
app first. Do not run Kudzu and the candidate against one local store concurrently.

1. Unsigned/default and Kudzu Next builds: enable sync and quit. Expect Unavailable
   or the Dev gate, no CloudKit exception, and no empty state file.
2. Run the ignored `signed_engine_emits_nonempty_secure_state_then_stops` test from
   a signed test executable packaged in a production-identity app with the matching
   embedded provisioning profile. It may create the Kudzu zone and fetch records;
   it creates no snippets. Check nonempty, decodable archives and no events after stop.
3. In the assembled app, verify first sync and archive resume. Confirm state lands
   atomically in `sync/engine-state.data` only after the preceding library merge is
   durable. Kill during remote add/delete, relaunch, and verify both Macs retain the
   add and deletion. Repeat with a fetch large enough to span batches.
4. Sync synthetic snippets, groups, usage, and meta between Tauri and Rune on two
   Macs. Compare record names and payloads. Verify unrelated container zones stay intact.
5. Edit one snippet offline on both Macs, then reconnect. Confirm the service's
   winner, server change-tag replacement, and no repeated conflict/requeue loop.
   Exercise UnknownItem and ZoneNotFound in an approved test account.
6. Sign out, wait over four minutes, sign in, and switch accounts. Confirm stable
   account status, local library retention, generation fencing and fresh server knowledge.
7. Reset only the Kudzu zone in the approved test account. Confirm recreation is
   queued before notification, the local library survives and the service resubmits it.
8. Toggle sync repeatedly and quit with work in flight. Resume without losing
   queued edits; check no empty state overwrites and no callbacks after stop.
9. Exercise transient network failure, quota exhaustion and a permanent failure.
   Confirm retries/status and inspect logs for absence of snippet or raw error text.
10. Run the signed candidate on Apple Silicon and its Intel slice under Rosetta.
    Compile success here does not establish either signed runtime result.

CloudKit delegate calls follow Apple's [serial-event contract](https://developer.apple.com/documentation/cloudkit/cksyncenginedelegate-1q7g8):
they do not initiate fetch/send operations from inside the delegate.
