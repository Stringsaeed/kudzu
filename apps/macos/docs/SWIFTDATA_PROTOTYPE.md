# SwiftData persistence prototype (Step 2)

Status: local checkpoint accepted after independent Astra review and its P2
correction. 16 targeted disk/schema tests pass. Live CloudKit is unverified.
Scope: `Packages/KudzuKit/Sources/KudzuServices/Persistence/` and
`Tests/KudzuServicesTests/LibraryRepository*Tests.swift`. Nothing in `Store/`, `Cloud/`,
`KudzuApp` or their tests is touched. The running apps are not routed into this code.

## 1. Interface sketch (written before the code)

### What callers exchange

Only existing Sendable Core values cross the boundary: `Library`, `Snippet`, `Group`,
`UsageRecord`. SwiftData `@Model` classes and `ModelContext` never leave `KudzuServices`.

```swift
/// One explicit owner of the SwiftData store. A `@ModelActor`: its `ModelContext` lives on the
/// actor's serial executor and never crosses it. Every mutation is "take a Library value,
/// run the caller's LibraryEditing closure on it, apply the difference to rows in one
/// transaction, publish the new value".
@ModelActor
public actor LibraryRepository {
    /// Opens (or creates) the store described by `configuration` and loads the first snapshot.
    public static func open(_ configuration: LibraryStoreConfiguration) async throws -> LibraryRepository

    /// The last committed library, readable synchronously from any thread (lock-protected copy,
    /// updated only after a successful save). This is the seam for the synchronous host callbacks.
    public nonisolated var snapshot: Library { get }

    /// Every committed snapshot, in commit order, for AppModel/Engine to observe.
    public nonisolated var changes: AsyncStream<Library> { get }

    /// Reads rows and rebuilds the Library value (used at open and by tests that want to bypass
    /// the cached snapshot to prove what is on disk).
    public func loadLibrary() throws -> Library

    /// The mutation entry point. `body` is exactly what `DataStore.editLibrary` callers pass today
    /// (LibraryEditing.*). A throw from `body`, or any failure while applying or saving, leaves the
    /// rows, the disk and the snapshot as they were.
    @discardableResult
    public func edit<T: Sendable>(_ body: @Sendable (inout Library) throws -> T) throws -> T

    /// Replaces the stored rows with `library` (id-keyed upsert + delete of absent ids), in one
    /// transaction. Running it again with the same input is a no-op: no extra rows.
    public func importLibrary(_ library: Library) throws

    /// The engine's usage counter (the one call that comes from an engine thread today).
    public func recordUsage(snippetID: UUID, deviceID: String, charsSaved: Int, now: Date) throws
}

/// Where the store lives and whether it mirrors to CloudKit. There is no `.automatic`.
public enum LibraryStoreConfiguration: Sendable {
    /// On-disk store, CloudKit mirroring off. Tests use this with a scratch directory.
    case local(url: URL)
    /// On-disk store mirrored to the private database of the *named* container. The legacy
    /// `iCloud.sh.saeed.kudzu` identifier is refused: its records are a custom schema.
    case cloudKit(containerIdentifier: String, url: URL)
}
```

### Rows (Services-internal)

Three `@Model` classes, one per independently synced record. Every attribute has a default or is
optional, there is no `#Unique`, no relationships (parent/group membership is a plain `UUID?`
attribute, as in the Core value types), so nothing depends on another record having arrived.

| Row | Attributes |
| --- | --- |
| `PersistedSnippet` | `id: UUID`, `nameUTF8: Data`, `triggersJSON: Data`, `contentUTF8: Data`, `formattingJSON: Data?` (external storage), `groupID: UUID?`, `fireModeRaw`, `caseModeRaw`, `insertionRaw: String?`, `ruleJSON: Data?`, `isFavorite`, `isEnabled`, `createdAt`, `updatedAt`, `listOrder: Int` |
| `PersistedGroup` | `id`, `nameUTF8: Data`, `parentID: UUID?`, `sortIndex`, `isEnabled`, `insertionRaw: String?`, `ruleJSON: Data?`, `listOrder` |
| `PersistedUsage` | `snippetID`, `deviceIDUTF8: Data`, `count`, `charsSaved`, `lastUsedAt: Date?`, `listOrder` |

`listOrder` keeps the observable array order (`createSnippet` inserts at the front,
`duplicateSnippet` at `index + 1`, groups and usage append). Nested values (`triggers`,
`Formatting` with its spans, links and image data URLs, `AppRule`) are stored as the JSON that
Core's own `Codable` writes, and decode through the same code
that reads `library.json`. A payload that does not decode is an error naming the row and field;
it is never replaced by a default.

### Why this executor shape fits the current callers

- `LiveBackend` commands are already `async`: they `await repository.edit { LibraryEditing.… }`.
- `AppAgentHost.library()` and `LiveEngineHost.formRequested` are synchronous and run on socket
  and engine threads: they read `repository.snapshot`, a lock-protected value copy. No
  `ModelContext` is touched on those threads.
- `LiveEngineHost.expanded` runs on an engine thread, never inside the event-tap callback (the
  engine posts it after insertion). Integration hands the counter to the actor with
  `Task { try await repository.recordUsage(…) }` and logs failure, exactly as it logs today's
  `saving library.json failed`. Database work therefore stays off the tap thread and off the main
  thread.
- `AppAgentHost.editLibrary` is synchronous and *writes*. Integration can either keep a small
  bridge that blocks the socket thread on the actor (a semaphore around a `Task`, as the socket is
  not the main thread and the actor never waits on the socket), or make `Agent.Host.editLibrary`
  async. That decision is Step 3's; both keep the rows on the actor.

### What is deliberately not here

No second context, no fake CloudKit, no account model, no dedupe policy (two rows sharing an `id`
is reported as an error, not resolved), no settings/form-memory/clipboard rows (they stay local
JSON files), and no change to the running app.

## 2. Implementation and evidence

Four Services/Persistence files implement the models, codec, configuration and actor.
Three new Services test files provide disk tests, schema checks and fixtures.
No dependencies were added.

The inherited edge test exposed truncation of persisted String content at an
embedded NUL. Arbitrary names, content and device identifiers now use binary UTF-8.
Invalid UTF-8 throws an error identifying the row and field. Enum raw values stay
strings. Nested JSON preserves formatting source, spans, links and image data URLs.

Byte-only edits must commit even when Swift canonical Unicode equality says they
are unchanged. The no-op check combines Core equality (including exact dates)
with JSON byte equality. Dates stay native Date attributes; the existing JSON
interchange format rounds some dates to seconds. Usage identity combines snippet
UUID with base64 of device UTF-8, preserving distinct NFC/NFD device identifiers.
Duplicate incoming snippet/group/usage identities and unsupported format versions
are rejected. Repeated imports add no rows.

The duplicate trigger expectation was corrected to [";all3", ";ALL22"]:
the existing ;ALL2 trigger occupies ;all2 case-insensitively. Core was unchanged.

The tests use the frozen Core Fixtures/models/library.json read-only and explicit
edge values. They cover full-field import/reopen, array order, hierarchy, precise
dates, NUL and NFC/NFD bytes, large image payloads, LibraryEditing create/save/
duplicate/move/group mutations, delete/Undo restore, per-device usage, repeat
import, rejection, failure after staged inserts/updates/deletes, retry, corrupt
payloads, duplicate stored identities and snapshot publication. Injected throws
prove rollback; they do not simulate power loss or prove production crash recovery.

Caller sketch (not integrated):

~~~swift
let repository = try await LibraryRepository.open(.local(url: scratchStoreURL))
try await repository.importLibrary(exportedLibrary)
let created = try await repository.edit {
    LibraryEditing.createSnippet(in: &$0, draft: draft)
}
let synchronousRead = repository.snapshot
for await committed in repository.changes {
    // Deliver this Core value to AppModel / Engine on its appropriate executor.
}
~~~

Prefer making socket mutations async in Step 3. Any blocking bridge first needs
proof it cannot run on the main actor or event-tap thread. Usage callbacks enqueue
actor work with explicit error handling. ModelContext stays actor-owned.

## 3. Isolated live CloudKit route (not executed)

The proposed fresh container is iCloud.sh.saeed.kudzu.library. This does not claim
the container exists. The code refuses legacy iCloud.sh.saeed.kudzu before opening
a store and never auto-discovers a destination.

External blocker: no fresh container, app association or matching provisioning
profile is configured for this prototype. Use a separate signed macOS test host
and scratch store on each Mac. Configure iCloud/CloudKit and remote notifications
for that container, matching signing/profile and Development environment.
Do not change the running release/Dev apps, entitlements or stores.

Follow [Apple's schema initialization and capability instructions](https://developer.apple.com/documentation/swiftdata/syncing-model-data-across-a-persons-devices).
Use the three prototype model types in its DEBUG-only Core Data initialization;
unload that container before opening SwiftData on the store URL. Confirm the
Development schema in CloudKit Console. Production promotion is a later release
operation.

The host explicitly opens:

~~~swift
let repository = try await LibraryRepository.open(.cloudKit(
    containerIdentifier: "iCloud.sh.saeed.kudzu.library",
    url: scratchDirectory.appendingPathComponent("Library.store")
))
~~~

On Mac A import a synthetic Library with stable IDs. On Mac B, using the same
test iCloud account and a separate empty scratch store, open the same named
container and periodically reload() until records arrive. Compare IDs, UTF-8,
dates, ordering, formatting/images and usage. Edit on B, reload A and verify;
delete on A, reload B and verify. Repeat offline/reconnect and process restart.
Record actual export/import events, errors, URLs, account scope and observed
values. Timeout means the live gate failed. Never seed B by importing A again.

The prototype publishes local commits and explicit reloads only. It has no
remote-import observer or account-lifecycle integration. Step 3 must prove how
remote imports refresh the actor context and snapshot, and prevent stale cached
Library edits from overwriting remote changes. Duplicate mirrored IDs are
reported, not repaired. Concurrent ordering/edits, account switching/isolation,
asset transfer, minimum macOS runtime and Production schema are unproven.
Two local contexts do not establish cross-device sync.

## 4. Reproduction and scope

Commands from the worktree:

~~~sh
swift test --package-path apps/macos/Packages/KudzuKit --filter 'LibraryRepositoryTests|LibraryStoreConfigurationTests'
swift test --package-path apps/macos/Packages/KudzuKit
swift build --package-path apps/macos/Packages/KudzuKit
~~~

All three commands passed. Targeted run: 15 tests in 2 suites.
Full-package results are recorded per test executable in
/private/tmp/kudzu-swiftdata-full-tests.log; build log:
/private/tmp/kudzu-swiftdata-build.log; targeted log:
/private/tmp/kudzu-swiftdata-targeted.log.

Base HEAD: a8b96551d98ce18e4cf292f6e404adbea7b5e91f, plus the existing uncommitted
native migration and prototype. Runtime: macOS 27.2 (26B123), Apple Swift 6.4.
This is not a macOS 14 runtime claim. Source hashes accompany the result report.

Step 2 local persistence is proven by these tests. Live CloudKit remains open.
Current DataStore/CloudSync/Core/Backend/UI and their tests
were not modified. No live store writes/import, app launch/quit, production cloud,
entitlement/profile, commit, push or PR operation was performed.

## 5. Independent review correction

Astra's review (`/private/tmp/kudzu-swiftdata-prototype-review.md`) conditionally
accepted the local checkpoint with one P2: passing persisted properties through
an inout comparison helper dirtied unchanged rows. Twenty usage-only edits
advanced an untouched snippet's SQLite revision from 4 to 24.

Root changed all 29 codec assignments to compare through a reference key path
before invoking the setter. The regression
`applyingUnchangedValuesLeavesTheContextClean` failed before the correction and
now passes for unchanged snippet, group and usage rows; a real content edit
still stages exactly one row. All 16 targeted tests passed. Recompiling Astra's
actual-source probe confirmed the snippet revision remains 4 after those 20
usage updates, and identical apply leaves zero changed models.

Logs: `/private/tmp/kudzu-swiftdata-dirty-rows-red.log`,
`/private/tmp/kudzu-swiftdata-dirty-rows-green.log`, and
`/private/tmp/kudzu-swiftdata-dirty-rows-probe.log`. The earlier 927-test full
package run predates this correction. No live-sync evidence is implied.
