# S33 review — sync service

Independent review (Claude Opus) of `refactor(desktop-v2): add the sync service`. Verdict: **fix B1 and M1–M6 first**; M1–M4 change the port, so the orchestrator's port decisions at the end bind both S33's fix and S37. B1, M1, M3 and M4 were confirmed with temporary probe tests.

## Blocker

**B1. The index and checkpoint reach disk before the library does; a crash then deletes remote snippets from iCloud or brings deletes back.** `sync/job.rs:287-312` (`on_fetched`) and `:339-363` (`on_sent`). `transact` commits the library to memory and its write is debounced, but `commit_index` and `store_engine` write `index.json` and `engine-state.data` at once.
1. Mac B adds X; Mac A fetches it. The library is at revision 2 in memory, revision 0 on disk; the index (with X) and the advanced token are on disk.
2. Kudzu is killed, crashes, or the library write fails; it relaunches. The token is past X, so X is never fetched again.
3. `local_changes` sees X in the index but not in the library and sends a delete. X is gone from every Mac. A remote delete is the mirror case: it comes back as an upload.

Tauri saved the library synchronously, so its window was tiny. Fix: hold the merged index as pending and write the index and checkpoint only once `library.saved().durable >= revision` (a waker on `saved()`); push nothing while pending. At shutdown: stop the worker, drain the writers, then checkpoint only if the library is durable (PLAN §1.2 step 4; `mod.rs:12` and `shutdown` at `mod.rs:308-337` do it the other way round). Test a relaunch with the library as it is on disk.

## Majors

**M1. "No account" and "Restricted" are overwritten** (`job.rs:238`, `:416-432`). `sync_now` publishes `Syncing` whenever `fetch` returns `Ok`, and `on_failed` maps every error to `TEMPORARILY_UNAVAILABLE`. After `availability(NoAccount)` the next tick shows "Syncing…" for good; a failed fetch tells a signed-out user iCloud is temporarily unavailable. Tauri published Syncing only on WillFetch/WillSend. Fix: track availability in `Job`; while not `Available`, keep the status on ticks, Sync now and failures. On `Available`, fetch at once.

**M2. `SIGNED_OUT` is unreachable**, so sign-out loses Tauri's "Signed out of iCloud. Your snippets stay on this Mac." (`ports/cloudkit.rs:59`, `job.rs:247-261`, `status.rs:26`). Fix: `account_changed(generation, AccountChange::{SignIn, SignOut, Switch})`. SignIn: `Syncing`, no reset. SignOut: reset, restart, show `NoAccount{SIGNED_OUT}` until `Available`. Switch: reset and restart.

**M3. `shutdown` checkpoints and stops a store that isn't running** (`mod.rs:333-336`, `job.rs:101-108`): Dev, unentitled, Off or after a failed start. With sync Off, a stopped adapter returning empty bytes overwrote `engine-state.data` with 0 bytes, and the next start got `Some([])`. `halt` also stops without checkpointing first, so turning sync off drops newer state. In Dev the port is still reached, and unentitled CloudKit calls raise Objective-C exceptions. Fix: the worker owns checkpointing (a final shutdown work item); checkpoint only while running and before `stop`; empty bytes mean "no state" and never overwrite a file.

**M4. The checkpoint is pulled, not ordered with events, so the token can get ahead of the merges** (`job.rs:515-519`, `ports/cloudkit.rs:77`). CKSyncEngine delivers batch N, batch N+1, then a state update covering N+1. `on_fetched(N)` calls `checkpoint()` and gets the N+1 token; a crash before N+1 merges loses N+1. Fix: an ordered push, `CloudEvents::state(generation, bytes)`, queued with the other events and persisted after the preceding merges are durable (B1). The fake should emit it out of step with `fetched`.

**M5. Unacknowledged records are sent again on every turn** (`job.rs:450-485`). Create A, then B before any `sent`: A appears in two sends, and any edit or tick before the ack resends the whole in-flight set. Tauri pushed record IDs into CKSyncEngine's deduped pending set. Fix: define `send` as an upsert of pending by name, and track `in_flight: name → payload` in `Job` so only changes since the last send go out.

**M6. The tests mirror the happy path.**
- The failed-checkpoint restart test (`tests.rs:648-714`) reuses the in-memory `LibraryService` and never refetches from the old token; it passes with B1 present.
- Untested: `CloudSend.conflicts` and `requeue`; remote tombstones and local deletes; `zone_missing`; a tick from an earlier generation across an account change; a failed start then retry; `SIGNED_OUT`. "All eight statuses" never shows Unavailable in the main test.
- Fake gaps: one `failure` field fails start, fetch, send and checkpoint together; no synchronous events from inside `start`/`fetch`/`send` (re-entrancy); no model of behaviour after `stop`; no state event.

## Minors

1. `job.rs:318-325`: a non-final batch publishes UpToDate mid-fetch. Gate on `full_fetch`.
2. `mod.rs:401-409`, `job.rs:73-77`: every settings change (theme, hotkey) triggers a fetch and send. React only when `icloud_sync` flips.
3. NeedsUpdate never starts an update check (Tauri's `settle` called `updater::check(app, true)`). S40 reaction: NeedsUpdate → `updates.check(Background)`.
4. `job.rs:132-137`: `START_FAILED` says "Kudzu will try again", but nothing retries. Arm a retry timer.
5. `job.rs:250`: `on_account` ignores a failed index reset and starts anyway, so the old account's index applies to the new account. Stay in Error and retry the reset before `begin`.
6. `job.rs:454`, `:558`: `push` reads two library snapshots; an edit between them drops a record. Use one snapshot.
7. `status.rs:17`: `CONTAINER_ID` lives in `kudzu-app`; S37's Accept says names come from `kudzu_core::sync`. Move it.
8. `mod.rs:213-225`: a worker panic leaves the engine running. Call `cloud.stop()`.
9. Parity, not a regression: a local delete in flight plus a remote modify of the same record brings it back (Tauri did the same). Record as known behaviour.

Rules 4 and 9: no violations.

## Live checks for S71/S72

1. `kill -9` within a second of a remote add, and after a remote delete; after relaunch the add is still in iCloud and the delete stays deleted.
2. Sign out of iCloud while running: the signed-out sentence shows and stays after 4 minutes; sign back in: syncs with no duplicates.
3. Switch accounts: library kept, uploads to the new account after the first fetch, nothing from the old index remains.
4. A large first fetch in several batches: "Up to date" doesn't appear early; no starter snippets upload before it finishes.
5. Edit one snippet offline on two Macs, reconnect: newer `updatedAt` wins; no repeating conflicts.
6. Reset the Kudzu zone through iCloud settings: library kept, everything uploads again.
7. Turn sync off and quit: `engine-state.data` isn't empty; the next start resumes without a full refetch.
8. Many offline edits then reconnect: each record is sent once.
9. Unsigned and Kudzu Next builds with sync on, then quit: no CloudKit exception.
10. Several expansions: the usage record reaches the other Mac within about 12 minutes, not once per expansion.
11. If possible in CloudKit Development: `meta.syncVersion = 2` gives NeedsUpdate, stops sends, and runs an update check.

## Port decisions (orchestrator, binding on S33's fix and S37)

S33's fix owns `ports/cloudkit.rs`, `fixtures/cloudkit.rs` and the service. S37 builds the adapter to this contract and does not edit the port file.

1. **Account:** `CloudEvents::account_changed(generation, AccountChange)` with `enum AccountChange { SignIn, SignOut, Switch }`. The adapter reports `availability` after every successful `start` and on every change. S33 defines how `CouldNotDetermine` maps, matching Tauri, and documents it on the port.
2. **State:** remove `CloudStore::checkpoint()`. Add `CloudEvents::state(generation, bytes)`, delivered in order with the other events. The bytes are CKSyncEngine's serialized state (NSKeyedArchiver). Empty bytes are never sent. No events after `stop`. The service persists state only after the merges that precede it are durable.
3. **Send:** `send(batch)` upserts the adapter's pending set by record name; the latest payload wins; a delete replaces a pending save; it is idempotent; an empty batch means "send what is pending now". `sent.saved` carries the server-returned payload. The service also tracks what is in flight and sends only changes.
4. **First launch:** `start(None)` means no saved state, so the adapter queues the zone save.
5. **Zones:** `zone_deleted` comes from fetched database changes filtered to the `Kudzu` zone; `zone_missing` comes from a send's `ZoneNotFound`. In both cases the adapter queues the zone save. The service re-derives resends from its index; the adapter never re-adds `requeue` names itself.
6. **Conflicts:** `CloudSend.conflicts` is only for `ServerRecordChanged`; the adapter replaces its cached record (change tag) with the server copy before reporting. `UnknownItem`: drop the cached record and list the name in `requeue`.
7. **Failures:** the adapter swallows transient codes (NetworkUnavailable, NetworkFailure, ServiceUnavailable, RequestRateLimited, ZoneBusy); `QuotaExceeded` maps to `QuotaExceeded`; everything else is `Permanent`. An `Err` from `fetch` or `send` means the request could not be issued at all. Never forward `localizedDescription`.
8. **Lifecycle:** `stop` is safe without `start`, idempotent, and never touches CloudKit when unentitled. The service never calls the port in Dev builds or after an Unavailable start.
9. **Threading:** port methods run on the sync worker; events may be delivered synchronously from inside them and only enqueue. Only the `Kudzu` zone is reported, never another zone in the container. `full_fetch` is set on `DidFetchChanges`.
10. **Names:** `CONTAINER_ID` moves to `kudzu_core::sync` with `ZONE_NAME` and `RecordKind`.
