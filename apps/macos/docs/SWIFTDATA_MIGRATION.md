# SwiftData migration

Status: preservation and inventory complete; the bounded local persistence
prototype is implemented. Fable 5.1 started it; after a Claude API rate limit the
user moved the same implementation task to Sol 6.1 high, which completed it.
The implementation run passed 15 targeted tests, 927 package tests, and the
package build; root verified its logs and hashes. Astra conditionally accepted
the local checkpoint with one P2 fix. Root repaired the codec's inout writeback,
added a failing-then-passing regression, and passed 16 targeted tests. Astra's
actual-source disk probe now keeps an unchanged snippet at revision 4 across
20 usage-only edits (previously 4 to 24). The local checkpoint is accepted under
that condition. Steps 3+4 source implementation is now complete and pending
independent review: 832 package tests, Swift 6 build, release validators, and
universal ad-hoc Release compile passed. Root verified all 388 source manifest
entries and both executable architectures. An external-data clone diagnostic is
unresolved; integration acceptance and live CloudKit remain open. Astra then
reproduced six P2 integration/migration defects. Root repaired those exact cases,
proved and fixed the admitted-socket shutdown edge, restored local-state coverage,
and latched restart-required status. The repaired snapshot passes 846 package
tests and a fresh universal ad-hoc Release compile with strict signature checking.
Astra accepted the bounded local integration/migration checkpoint after 16 fresh
focused tests and independent probes; all 391 source hashes matched. The asset
diagnostic and named runtime/live gates remain open. Final verification is active.

The user approved replacing custom persistence/sync with SwiftData and automatic
CloudKit sync, using a one-time copy, verify, and switch migration. The native UI,
editor, matching/expansion engine, and template domain remain in place. The first
checkpoint is executable prototype evidence, not another architecture comparison.

## Completion checklist

| Step | Expected result | Status |
| --- | --- | --- |
| 1. Preserve and inventory | Verified source archive; private data safety copies; explicit list of state to preserve | Complete |
| 2. Prototype | Real disk-backed SwiftData transactions and reopen tests using existing domain values; CloudKit-compatible schema; separate live-sync proof | Local checkpoint accepted after Astra's P2 correction; live sync gate remains |
| 3. Integrate | One persistence owner in Services; existing Backend and Core value boundary; automatic CloudKit sync; obsolete controller removed | Local source accepted by Astra; 846 tests and universal build pass; live CloudKit unverified |
| 4. Migrate | Backup, import, reopen, compare, then switch; crash/retry does not duplicate or erase source data | Local implementation accepted by Astra; original data retained; real-data cutover not performed |
| 5. Verify and cut over | Independent Astra review, package/build checks, real app/expansion checks and live CloudKit proof | In progress; asset lifecycle, live CloudKit, UI and release gates open |

The implementation is saved as a local checkpoint on `migration/native-macos`.
No push, release, production cloud deletion, or live-store cutover has been
performed. Old sync review findings describe the implementation being replaced;
closing that controller's review is no longer this migration's next milestone.

## Preservation evidence (2026-11-09)

- Source: `/private/tmp/kudzu-before-swiftdata-20261109T075206Z/`.
  Contains a source archive, SHA-256 manifest, tracked binary diff, Git status,
  and base HEAD. 909 paths recorded; 908 regular file hashes verified against
  the archive. Includes current tracked and nonignored untracked files; ignored
  build products are excluded.
- User-state safety copy:
  `/private/tmp/kudzu-userdata-before-swiftdata-20261109T075251Z/`.
  514 paths recorded, private directory mode 0700 and copied files mode 0600.
  Runtime socket/special files were excluded. SQLite stores were captured with
  SQLite's backup API and passed integrity checks.
- Both Kudzu and Kudzu Dev were running during the safety copy. Ordinary files
  were checked for changes during copying, but this is not a quiesced, globally
  consistent migration export. The production migrator must pause old writers
  and produce its own verified export before switching.

The production folder is `~/Library/Application Support/sh.saeed.kudzu/`.
The development folder is `~/Library/Application Support/sh.saeed.kudzu.dev/`.
An additional older managed store exists in `~/Library/Application Support/kudzu/`
(17 snippet rows, 2 group rows, 4 usage rows in its Synced store). It was preserved
separately; it must not be silently combined with the Tauri library or opened with
the new schema. Git commit `e4111f40` contains an earlier SwiftData implementation
and is a read-only reference, not current acceptance evidence.

## State inventory

| State | Existing owner / format | Migration treatment |
| --- | --- | --- |
| Groups, snippets, usage | DataStore; `library.json`; KudzuCore.Library | Import with stable IDs, hierarchy, ordering, all template/formatting fields, dates, modes and device-scoped counters preserved |
| Settings | `settings.json`; KudzuCore.Settings | Preserve all values; keep local configuration outside synced content unless deliberately specified |
| Remembered form values | `form-memory.json`; KudzuCore.FormMemory | Preserve locally, retaining snippet IDs |
| Clipboard metadata | ClipboardHistory / ClipStore; `clipboard/history.json` | Preserve existing local-only store |
| Clipboard payloads and thumbnails | `clipboard/items`, `thumbs`, `trash` | Preserve private files and restoration semantics; no cloud upload |
| Backups | `backups/` | Retain original backups and add a complete migration recovery copy |
| Device identity | `device-id` | Preserve; do not merge usage counters across devices |
| UI state and first-run markers | AppStorage/UserDefaults and small local files | Preserve current defaults domain and bundle IDs; do not rerun onboarding on migrated data |
| Permission grants / login / MCP registration | OS and existing app identity | Preserve bundle ID and established integration behavior |
| Legacy cloud index/checkpoint/account files | `_nativeSync` and/or `sync/` | Retain as recovery evidence; do not import them as authority for SwiftData sync |

The current JSON export contains only Library (groups, snippets, usage). It is not
a full application backup. No application credential vault was found in the
current Services layer; clipboard secret detection is local classification and
must not be mistaken for a credential migration. Release signing keys are outside
the app's user-data migration scope and stay untouched.

## Existing call boundary

`LiveBackend` owns DataStore and publishes immutable Library snapshots to AppModel
and Engine. LibraryEditing in Core implements shared mutation rules. UI commands
are async through Backend. Engine and AgentServer also have synchronous host
callbacks; the prototype must account for those callers without putting database
work in the keyboard event callback or letting ModelContext cross executors.

Target shape: SwiftData models and contexts stay inside Services. Callers exchange
existing Sendable Core values. One explicit owner serializes store access; changes
are published after successful saves. Editing drafts remain app-owned. JSON
import/export remains an interchange contract, not the new database authority.

## Bounded prototype contract

Use scratch directories only, with real SwiftData (not a replacement store).
Do not route the running app into it or edit old persistence/cloud files yet.

1. Sketch the narrow store interface and its executor ownership before coding.
2. Represent independently synced snippet/group/usage records; do not wrap the
   whole library in a single CloudKit record. Preserve nested formatting and
   embedded images losslessly. Give persisted attributes defaults or make them
   optional; use no uniqueness constraint unsupported by automatic CloudKit sync.
3. Exercise disk-backed import, update, delete/Undo restoration, and reopen using
   the real LibraryEditing rules. Verify literal expected values and preserved
   IDs, order, hierarchy, dates, app rules, formatting and usage.
4. Prove interrupted/repeated import leaves a recoverable source and does not
   duplicate a completed local import. Do not claim this proves concurrent
   multi-device CloudKit deduplication.
5. Prove rejected work does not leave a partial committed library. Prefer a real
   failing transaction/rollback observation over implementation-only assertions.
6. Add a precise isolated live-sync route for a fresh CloudKit container. Local
   schema construction, a fake account, and two local contexts do not prove live
   CloudKit synchronization or account switching.

Reuse useful ideas from earlier code only after checking against current Core
types. The earlier SnippetModel lacks the current formatting field, so copying it
verbatim would lose data.

## Cloud and migration policy

The legacy custom CloudKit destination is `iCloud.sh.saeed.kudzu`. A new
SwiftData-compatible destination is required; no new container has been created,
provisioned, or deployed yet. Keep a prototype's destination explicit and isolated.
No implicit discovery that might attach it to the legacy production schema.

One primary Mac seeds the new destination. Other Macs join it; any local-only
differences need preservation before cutover. Retaining source data is mandatory;
deleting old cloud records is not necessary to prove or perform local migration.

Apple references:

- [Automatic SwiftData sync and compatible containers](https://developer.apple.com/documentation/swiftdata/syncing-model-data-across-a-persons-devices)
- [ModelContext transactions](https://developer.apple.com/documentation/swiftdata/modelcontext/transaction(block:))
- [ModelActor isolation](https://developer.apple.com/documentation/swiftdata/modelactor)

## Verification record

- Isolated production-backup rehearsal: 16 snippets, 2 groups, 5 usage rows;
  reopen and JSON round-trip passed, all 329 original file hashes preserved and
  source backup unchanged. CloudKit explicitly disabled. Report:
  `/private/tmp/kudzu-backup-migration-rehearsal.json`.
- Fresh Debug build/strict signature check passed. Runtime UI checks are blocked
  by the Mac's existing agent-device session `kz`, including after the user
  reported freeing it. No foreign session was reused or closed.
- New-container provisioning is not available locally: project profile missing,
  none of 49 installed profiles authorizes the destination. Preview browser
  reached Apple sign-in; awaiting browser choice/login. No cloud state changed.

- Source snapshot: archive contents matched its manifest.
- Safety copies: JSON library/settings/form-memory parsed; copied files checked;
  additional SQLite backups passed integrity checks.
- Prototype implementation snapshot: 15 targeted tests, full package 927 tests,
  and package build passed; root checked its logs and source hashes. See
  `SWIFTDATA_PROTOTYPE.md` and `/private/tmp/kudzu-swiftdata-prototype-result.md`.
- Independent local review: `/private/tmp/kudzu-swiftdata-prototype-review.md`.
  Conditional acceptance with one P2, unnecessary dirtying of unchanged rows.
- Root P2 correction: 16 targeted tests passed. The new clean-context test failed
  before the repair; recompiling Astra's probe with the repaired production
  sources kept the snippet revision at 4 after 20 usage-only edits. Logs:
  `/private/tmp/kudzu-swiftdata-dirty-rows-{red,green,probe}.log`. The 927-test full
  run predates this two-file correction; the full suite will run on integration.
- Live CloudKit, GUI parity, external expansion, signed release: not verified for
  this new architecture.

Record further outcomes here by completed step. Test totals must name the exact
source snapshot and must not substitute for runtime cloud or UI evidence.

### Step 2 implementation verification appendix — 2026-11-09

Resumed by Codex following the user's request to pick up the stopped prototype.
Local persistence is now proven for the bounded scratch-store scenarios: 15
targeted tests, the full 927-test KudzuKit suite, and swift build passed.
The checklist remains parent-owned; independent review has not occurred.

The NUL truncation failure was fixed by binary UTF-8 attributes for arbitrary
text, with byte-sensitive no-op checks and strict decoding. Duplicate input
identities and unsupported format versions reject without committing partial data.
Core editing behavior was preserved; the duplicate-trigger test expectation was
corrected to match its existing case-insensitive collision rule.

See SWIFTDATA_PROTOTYPE.md for interface, executor seam, limits and the explicit
fresh-container live-test route. Exact commands, logs, environment, base HEAD and
source hashes are in /private/tmp/kudzu-swiftdata-prototype-result.md.
No current persistence/sync/backend/UI integration or real-data cutover occurred.
Live CloudKit, account behavior, remote-import refresh/stale-write handling,
minimum-OS runtime, GUI and external expansion remain unverified.

### Steps 3+4 implementation evidence — 2026-11-09

Services repository now owns backend library persistence; LocalStore owns local
settings/form memory/device identity. Old runtime JSON library/custom sync controller
and its protocol-specific tests were retired. Production copy/verify/activation
migration, async socket writes, ordered subscription and shutdown drain are wired.
Full synthetic package run: 832 tests passed (68 Services, 59 Engine, 80 Editor,
36 Design, 456 Core, 133 App). See SWIFTDATA_INTEGRATION.md and
/private/tmp/kudzu-swiftdata-integration-result.md for final build evidence and limits.
One Core Data external-data clone diagnostic appears despite passing edge-payload
assertions; it remains an independent review item. Fresh provisioning and live
CloudKit/account/asset/Production gates remain open. Checklist status is unchanged
and parent-owned; no real migration or live sync was performed.
