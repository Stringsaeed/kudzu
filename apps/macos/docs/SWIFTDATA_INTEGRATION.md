# SwiftData integration

## Final interfaces and executor ownership

Services owns `LibraryRepository`, its SwiftData models and serial model executor.
Core continues to own Sendable Library/Snippet/Group/UsageRecord values and editing rules.
`LocalStore` owns settings, form memory, device identity and BackupStore; its library
accessor reads the repository's committed snapshot, never a JSON mirror.
LiveBackend awaits repository edits. A single subscription installs committed values
into Engine and publishes them to AppModel. Engine usage enters a tracked serial
queue drained after stopping input at shutdown.

The socket transports async wire String requests and responses. A request-local
synchronous Agent.Host is constructed inside a repository edit; its dynamic JSON
never crosses an actor boundary. Existing Core wire serialization remains in place.

Before starting services, bootstrap holds an exclusive migration lock, copies the
complete private data directory and defaults into recovery evidence, imports the
legacy Library into a separate local SwiftData destination, reopens and compares,
and atomically writes a local activation receipt. Originals remain untouched.
An active receipt prevents any subsequent legacy reimport. Secondary join explicitly
skips seeding but retains the same recovery copy. Failed startup is retryable.

Automatic mirroring explicitly names iCloud.sh.saeed.kudzu.library. Local development
and tests use .none. Account checks and Core Data CloudKit events report actual
state; a local save/reload cannot imply network completion. Configuration changes
requiring a container reopen take effect on restart and are reported as such.

## Evidence and limits

Implementation and verification results will be appended below. No live app,
user-data or CloudKit operation is authorized in this run.

## Implemented cutover

`LiveBackend.open` bootstraps off the main actor after SingleInstance has claimed
ownership (including its existing Tauri handoff). LibraryMigration additionally
holds an advisory exclusive migration lock. No old library/background writer is
started. Startup failures show Retry/Quit and retain original sources. The receipt
is required to reopen an active store; a missing active store never becomes empty.
An unrecognized destination is refused. Only a candidate carrying this migrator's
pending marker may be rebuilt on retry. A successful receipt prevents reimport.

The recovery directory is private (0700, files 0600), includes the original private
directory contents and defaults domain, and has SHA-256 hashes. Original files stay
in place. The separately preserved `Application Support/kudzu` store is never opened.
Tests pass nil defaults domains and synthetic paths; no user defaults are accessed
by those migration tests. Clipboard storage, backups, device ID and form memory stay
local. Version/pre-update backups continue through LocalStore.

The repository is now an explicit Swift actor with a fresh context per operation,
rather than the prototype's long-lived @ModelActor context. Refresh before edit
observes external committed rows. Delta assignments compare the command's base
Core values, preserving fields it did not change; absent rows are deleted only if
they belonged to that base. Explicit importLibrary is replacement semantics and is
used for initial migration. Normal imports/restores run Core Coordinator inside a
serialized repository edit, including their pre-import/pre-restore backup.
The accepted read-before-set key-path correction and clean-context regression remain.

`NSPersistentStoreRemoteChange` triggers actor reloads, with failures published as
errors. An initial-inclusive subscription is registered synchronously under the
snapshot lock; publication and initial delivery share that lock. LiveBackend feeds
its single stream to the engine/publication queue. AppModel protects initial state
from overwriting a library event received while getState awaited. Shutdown stops
engine input, drains tracked usage and async socket requests, awaits backup work,
then flushes local state before accepting application termination.

MCP wire Strings enter the repository. A request-local synchronous Host owns all
`[String: Any]` values on that actor. Save errors are returned through the existing
MCP error serializer. Socket worker threads wait for their serialized async request;
there is no main-thread semaphore bridge. The old synchronous transport initializer
remains for Core host/socket contract tests. Production uses the async initializer.

Ordering is listOrder followed by stable identity. Duplicate IDs pause edits and
write a private `library-conflicts-<UUID>.json` containing every decoded payload,
including repeated IDs. No mirrored record is selected or deleted automatically.
Recovery is explicit: preserve the store and sidecars, restore a verified store
backup, or resolve the duplicate identities in the recovery export and import it
into a clean destination. This is an operator recovery path, not a conflict UI.
Pure Core.Sync and its tests remain as reference and for existing Core JSON helpers;
the custom Services controller, checkpoint/journal/account-acceptance runtime is gone.

## Verification — 2026-11-09

All tests use real synthetic temporary disk stores and production APIs. The final
full-package run passes 832 tests: Services 68, Engine 59, Editor 80, Design 36,
Core 456, App 133. This includes the accepted clean-context regression; migration
stage interruption/retry, raw-source/blob preservation, unknown-destination refusal,
no reimport after newer edits, secondary non-seeding behavior, real second-context
external edits, backend CRUD/delete/Undo/import/export/backups and save-error retry,
async repository MCP, actual async Unix socket mutation/reopen, queue drain, and
local-only status honesty. Fault tests inject throws; they do not prove power-loss
or disk-full behavior. Second-context tests prove local isolation, not CloudKit.

Retired test files with their implementation: Services DataStoreTests,
LegacySyncMigrationTests, NativeSyncBenchmarkTests, CloudSyncTests, SyncTestSupport,
NativeSyncDocumentTests, NativeSyncAcceptanceTests, NativeSyncRepairTests; App
NativeSyncPrivacyTests and DurableLibraryReloadTests. Their obsolete controller,
JSON revision, fencing and checkpoint contracts were removed. New tests cover the
new production boundaries; totals are not preserved using compatibility shims.
The earlier accepted package had 927 tests (928 with root's added regression).

Commands and logs:

```sh
swift test --package-path apps/macos/Packages/KudzuKit
# /private/tmp/kudzu-integration-full-tests.log
swift build --package-path apps/macos/Packages/KudzuKit
# /private/tmp/kudzu-integration-build.log (Swift 6)
python3 apps/macos/scripts/release/test_release_validation.py
# /private/tmp/kudzu-integration-release-validation.log (3 tests)
xcodebuild -project apps/macos/Kudzu.xcodeproj -scheme Kudzu -configuration Release \
  -derivedDataPath /private/tmp/kudzu-integration-release \
  -destination 'generic/platform=macOS' ARCHS='arm64 x86_64' ONLY_ACTIVE_ARCH=NO \
  CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual CODE_SIGNING_ALLOWED=YES \
  PROVISIONING_PROFILE_SPECIFIER='' build
# /private/tmp/kudzu-integration-release.log
```

The Release command compiles an ad-hoc universal app; it does not package a DMG,
read a signing profile/secret, notarize or launch an app. Source hashes are in
`/private/tmp/kudzu-integration-sources.sha256`.

One SDK diagnostic remains visible in the passing edge-payload tests:
`CoreData: Failed to clone external data reference ... .interim ... NSCocoaErrorDomain Code=4`.
The exact scratch paths and message are retained in the full-test log. Tested payload
and reopen assertions pass, but the diagnostic's cause is not established here.
External asset lifecycle deserves independent review and live validation; these
results do not qualify CloudKit asset transfer.

## Automatic mirroring and remaining gates

The configured destination is explicitly `iCloud.sh.saeed.kudzu.library`, still an
unprovisioned candidate. Dev/tests/ad-hoc builds remain local. Signed entitled
release builds select automatic private mirroring at open when enabled. Turning
sync on/off requires restart; status reports that requirement. Account notification
and NSPersistentCloudKitContainer event observers publish real framework state.
No local reload/save ever publishes upToDate. The status action is now “Check iCloud
status”; it rechecks account availability, not a fictional manual sync operation.
Apple references: [automatic SwiftData mirroring](https://developer.apple.com/documentation/swiftdata/syncing-model-data-across-a-persons-devices)
and [Core Data CloudKit events](https://developer.apple.com/documentation/coredata/nspersistentcloudkitcontainer/eventchangednotification).

Fresh container creation/association, matching profile, signed Development schema
initialization, two-Mac seed/join and concurrent changes, account switching/isolation,
offline/reconnect, large asset transfer, minimum macOS 14 runtime and Production
promotion are external gates. No claim of live delivery, account isolation or signed
release readiness follows from local tests. Account changes currently report restart
required and rely on framework account handling; live account-transition semantics
must be reviewed before shipping. No cloud records or portal state were modified.

Steps 3+4 source is ready for independent review, with the diagnostic and live gates
above explicitly open. Root owns checklist acceptance. UI drafts/IME/editor/engine
algorithms and frozen fixtures were not changed. No app launch/quit, real data access,
commit, push, PR, deployment or new dependency occurred.

## Independent integration review repairs

Astra's `/private/tmp/kudzu-swiftdata-integration-review.md` withheld local acceptance
for six reproduced P2 cases. Root repaired them against failing-then-passing tests:
post-save refresh errors no longer reject committed commands; unrelated edits do
not resurrect externally deleted rows; stale agent sockets are omitted from
recovery; release bootstrap preserves the prerelease directory; symlinks are
refused without mutating their targets; and damaged/uninitialized active files
are refused while valid initialized empty stores remain readable.

The migration uses verified recovery bytes and checks source stability before
activation. Post-save refresh failures remain separately observable. The queued
two-client shutdown schedule also reproduced discarded admitted work and was
fixed while retaining explicit access revocation. Local state round-trip/default
coverage was restored and restart-required status is latched.

The repaired snapshot passed 846 package tests (Services81, Engine59, Editor80,
Design36, Core456, App134), a fresh universal ad-hoc Release compile and strict
signature checking. Logs: `/private/tmp/kudzu-integration-repairs-full.log` and
`/private/tmp/kudzu-root-integration-repairs-release.log`. Source manifest:
`/private/tmp/kudzu-integration-repairs-sources.sha256` (391 Swift files).
Detailed repairs/proofs: `/private/tmp/kudzu-swiftdata-integration-repairs.md`.

The asset diagnostic remains unresolved: it also occurs with retained stores,
so cleanup alone does not explain it. Ten retained stores passed fresh-process
reopen with exact bytes/dates/formatting; attempts to change staging did not
remove the diagnostic and were discarded. This is qualified local fidelity
evidence, not proof of harmlessness, general asset lifecycle or live CloudKit
transfer. Focused independent recheck and all named live gates remain open.
