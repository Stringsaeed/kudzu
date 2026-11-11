# S37 review follow-up

Base: `origin/t3/rune-migration-plan` at `a96b2d43`, including the final S33 contract
in `ports/cloudkit.rs`. The adapter alone changed; the port, fake and service did
not. The owner clarified two cases during implementation:

- Keep already delivered record batches and ordered state after a transient fetch
  error. Suppress only the terminal `full_fetch`; do not buffer the fetch.
- A zone-save failure attempted no records. Retain pending payloads, report the
  classified database failure, and retry the zone save on the next send. Transient
  zone-save errors remain CKSyncEngine's responsibility.

All code and test paths below are relative to
`apps/desktop-v2/crates/kudzu-system/src/cloudkit/`, except where stated otherwise.

## Red-to-green evidence

Each R test below was run against the old behavior, failed with the stated
observation, and passed after its repair. The commit body names all 13 tests.

| ID | Test and location | Red observation |
|---|---|---|
| R1 | `kudzu_zone_records_accept_a_concrete_user_owner` — `review_tests.rs:162` | A Kudzu zone with a concrete user owner was rejected. |
| R2 | `missing_zone_fetch_recreates_without_reporting_failure` — `review_tests.rs:97` | ZoneNotFound produced `Failed` instead of recreating quietly. |
| R3 | `send_completion_does_not_overwrite_a_delegate_conflict` — `review_tests.rs:64` | Two reports arrived: the delegate result followed by an empty Permanent report. |
| R4 | `transient_fetch_keeps_delivered_changes_but_has_no_terminal_result` — `review_tests.rs:113` | A third event falsely marked the failed fetch complete after its data/state events. |
| R5 | `restored_pending_records_are_removed_before_availability` — `review_tests.rs:28` | Restored record-change IDs remained in the engine pending set. |
| R6 | `worker_requests_drain_autoreleased_objects_before_returning` — `review_tests/pool.rs:36` | The native probe's destructor had not run when send returned (0 versus 1). |
| R7 | `empty_state_archives_are_dropped_without_failure` — `review_tests.rs:19` | Empty archive bytes became an error event. |
| R8 | `a_conflict_without_a_server_copy_is_permanent_and_discards_the_old_tag` — `review_tests/sending.rs:15` | Failure was None; the adapter requeued with its stale cache. |
| R9 | `deleting_an_already_missing_record_acknowledges_the_delete` — `review_tests/sending.rs:42` | The missing delete appeared in `requeue`, not `deleted`. |
| R10 | `a_send_failure_drops_all_unacknowledged_attempts_but_keeps_newer_and_unsent_work` — `review_tests/sending.rs:78` | A transiently failed record remained pending beside a summary quota failure. |
| R11 | `unknown_types_and_unparseable_names_never_reach_fetched_batches` — `review_tests/translation.rs:45` | Unparseable and type-mismatched names reached the service. |
| R12 | `unknown_send_results_do_not_acknowledge_or_fail_known_pending_records` — `review_tests/translation.rs:7` | An unknown record type produced a quota failure for a known pending name. |
| R13 | `permanent_zone_failure_retains_records_and_requeues_the_zone_on_the_next_send` — `review_tests/sending.rs:119` | The next send did not requeue the failed zone save (0 versus 1). |

Additional regressions passed when introduced; they were not represented as red
tests for behavior that was already correct:

| ID | Test and location |
|---|---|
| G1 | `acknowledgement_requeues_only_newer_work_and_never_a_losing_conflict_payload` — `review_tests/sending.rs:151` |
| G2 | `thousands_of_records_enter_the_engine_pending_set_and_native_batching_bounds_each_batch` — `review_tests/sending.rs:200` |
| G3 | `coalesced_fetch_calls_share_one_successful_terminal_batch` — `review_tests/fetching.rs:12` |
| G4 | `permanent_zone_fetch_failure_is_reported_once_despite_completion_overlap` — `review_tests/fetching.rs:42` |
| G5 | `a_transient_failure_before_any_delegate_event_is_silent_and_allows_retry` — `review_tests/fetching.rs:75` |
| G6 | `automatic_fetch_only_finishes_without_a_zone_failure_and_zone_deletion_is_separate` — `review_tests/fetching.rs:97` |
| G7 | `transient_zone_save_failure_keeps_pending_for_the_engines_own_retry` — `review_tests/sending.rs:247` |

## Review findings

| Finding | Evidence | Repair and implementation location |
|---|---|---|
| Blocker 1: owner filter | R1 | Match zone name only. This shared predicate is used for records, acknowledgments and zone events: `records.rs:31`. |
| Major 2: missing-zone fetch | R2, R4, G4 | Queue zone creation and emit no Failed for ZoneNotFound; record the unsuccessful fetch so no terminal success follows: `delegate.rs:160`, `fetching.rs:30`. |
| Major 3: completion overlap | R3, R4, G3–G5 | Send uses a None completion. Fetch coalesces active calls, confirms explicit success at completion, and suppresses errors already covered by delegate results: `requests.rs:21`, `requests.rs:51`, `fetching.rs:44`. |
| Major 4: autorelease pools | R6 | Pool native worker entry points, delegate callbacks and asynchronous completions; also pool entitlement reads and state coding: `requests.rs:9`, `native.rs:137`, `native.rs:184`, `native.rs:205`, `native.rs:218`, `native.rs:238`, `native.rs:258`, `delegate.rs:34`, `entitlement.rs:24`. |
| Minor 5: empty archive | R7 | Archive encoding returns its bytes; translation drops empty success instead of inventing InvalidData: `native.rs:258`, `delegate.rs:186`. |
| Minor 6: missing server copy | R8 | Choose Permanent, drop pending and the cached tag, and do not immediately requeue. The service may later submit a fresh record: `sending.rs:221`. |
| Minor 7: offline coverage | R1–R13, G1–G7 | Real local CKRecord/ID/batch objects plus the thin CKSyncEngine command interface exercise delegate decisions, completion overlap, restored state and queue_after_ack without opening iCloud: `engine.rs:9`, `review_tests/engine.rs:13`. |

## Contract decisions

| Decision | Evidence | Adapter behavior and location |
|---|---|---|
| 1. Drop conflict payload before reporting | Existing conflict regression and G1; valid-conflict withdrawal was already correct | Keep withdrawal before cache replacement/reporting; acknowledgments can requeue only current service-owned work: `sending.rs:229`, `sending.rs:32`. R8 repairs the missing-server-copy case. |
| 2. Drop failed record names | R10 | Drop every unacknowledged failed name from that native record attempt, including mixed transient/permanent failures. Preserve newer acknowledged work and unrelated queued records: `sending.rs:162`, `sending.rs:207`. |
| 2. Database-only clarification | R13, G7 | Retain record payloads because none was attempted; classify the failure and remember to requeue the zone on the next send. Transient zone errors are left to the engine: `sending.rs:267`, `requests.rs:51`. |
| 3. Missing delete is acknowledged | R9 | UnknownItem on a delete enters `deleted`; UnknownItem on a save still discards its cache and requeues: `sending.rs:193`, `sending.rs:253`. |
| 4. Drop restored record changes | R5 | Remove restored pending record changes before zone setup/availability. Keep database changes: `requests.rs:9`, `engine.rs:30`. |
| 5. Fetch completion and partial failure | R4, G3–G6 | Stream partial data/state; emit one successful terminal batch for coalesced explicit calls. Withhold terminal success and errors on transient failure. Zone deletion remains a separate event: `fetching.rs:16`, `fetching.rs:44`, `requests.rs:21`, `native.rs:92`, `native.rs:98`. |
| 6. Unknown records/count-only log | R11, R12 | Validate kind/name compatibility, filter unknown modifications/results/deletions, and give the log sink only an integer count: `records.rs:35`, `records.rs:68`, `records.rs:75`, `records.rs:93`, `native.rs:98`, `sending.rs:162`. |
| 7. Large sends use native batching | G2; the batching path was already correct | Submit all 2,000 tested names to the engine pending set and let the native pending-change batch initializer bound each batch; constructing a batch does not acknowledge it: `requests.rs:51`, `sending.rs:28`, `sending.rs:78`. |
| 8. Mid-run Unavailable | No new adapter defect or red test | Existing classification and forwarding remain: `errors.rs:43`, `wiring.rs:205`; stop still fences callbacks in `delivery.rs:61`. The unchanged S33 service regression `unavailable_is_permanent_until_relaunch` at `crates/kudzu-app/src/sync/tests.rs:1957` verifies process-lifetime policy in the workspace gate. |

## Verification and scope

All five requested gates passed after rebasing onto `a96b2d43`: formatting,
workspace Clippy with warnings denied, workspace tests, check-deps and the x86_64
kudzu-system check. The adapter suite has 40 passing offline tests and one ignored
signed/iCloud test. No lint or test suppression was added.

This follow-up's only edits outside Owns are `crates/kudzu-system/Cargo.toml` and
`Cargo.lock`, adding the existing approved workspace `log` dependency edge for the
required count-only diagnostics. No new package, schema, entitlement, port, fake,
service, or frozen application changes. The previously reviewed check-deps change
is untouched. Native entry points use the existing objc2 bindings; no binding gap
or upstream issue draft was needed.

Signed CloudKit delivery, account changes and memory behavior remain the S71/S72
checks in [HANDOFF.md](HANDOFF.md). No real CloudKit engine or container was created
by the executed tests. The completion ordering used here is documented by Apple's
[fetch completion API](https://developer.apple.com/documentation/cloudkit/cksyncengine-4b4w9/fetchchangeswithcompletionhandler%3A?language=objc).
