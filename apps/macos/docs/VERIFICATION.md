# Native migration verification

Working-tree record, 2026-11-09. Local source acceptance is separate from live UI,
CloudKit, real-data cutover and release acceptance. Current checklist:
[SWIFTDATA_MIGRATION.md](SWIFTDATA_MIGRATION.md).

## Accepted source and local checks

- SwiftData owns the library; LocalStore owns device-local state. The obsolete
  JSON persistence and custom CloudKit acceptance runtime are removed.
- The actual package passes **846 tests**: Services 81, Engine 59, Editor 80,
  Design 36, Core 456, App 134. No replacement store/model shims. Retired tests
  describe contracts of the removed implementation.
- Astra accepted the bounded local integration/migration checkpoint after 16
  independent focused tests and real-code probes. All 391 Swift source/test
  hashes matched. All 36 frozen Core fixture hashes are unchanged.
- A fresh universal ad-hoc Release build succeeded, contains x86_64 and arm64,
  and passes deep strict signature verification. A fresh Debug build also
  succeeded and passes signature verification. These results do not prove
  launch, Intel execution, macOS 14 runtime, Developer ID signing or notarization.
- Earlier website/release-fixture checks predate this final runtime phase and
  do not establish live Vercel, CI or production release.

Evidence under `/private/tmp/`: `kudzu-integration-repairs-full.log`,
`kudzu-root-integration-repairs-release.log`,
`kudzu-native-runtime-verification-build.log`,
`kudzu-integration-repairs-sources.sha256`, and
`kudzu-swiftdata-integration-recheck.md`.

## Isolated production-backup rehearsal

The saved production-folder backup was copied to a private scratch directory.
The real migrator ran with CloudKit disabled and no defaults domain. It preserved
**16 snippets, 2 groups and 5 usage rows**, reopened the active database, and
round-tripped JSON export. All 329 original files in that copy retained their
hashes; the source backup itself was unchanged. The running production app and
its Application Support folder were not switched.

Report: `/private/tmp/kudzu-backup-migration-rehearsal.json`; driver/log use the same
basename under `/private/tmp/`. The older managed store in `Application Support/kudzu`
remains separately backed up and unopened by this migration; it was not merged.

## External-data diagnostic

The missing .interim clone diagnostic is directly present in retained-store and
live-deleted-store probe logs. Ten retained stores passed fresh-process reopen
with exact values/bytes/dates/formatting. Cleanup alone does not explain it;
neither harmlessness nor data loss is established. The inspected final 846-test
log does not itself contain a matching warning, correcting an earlier overbroad
attribution. Standalone logs retain the evidence that keeps this gate open.

These are large persisted image-string fixtures, not validated PNG or CloudKit
asset transfer. General asset lifecycle remains unqualified. No speculative
storage workaround was kept.

## Runtime checks

The earlier pre-SwiftData Dev build completed onboarding, Settings and palette
smoke checks. Its practice preview was not external insertion proof. It had no
Accessibility/Input Monitoring grants.

The rebuilt SwiftData app has not been driven. Native automation reports
DEVICE_IN_USE owned by session kz; this workspace lists no active session.
Retry after the user reported freeing it returned the same error. The foreign
session was not reused or closed. The earlier smoke flow is not evidence for
the rebuilt app; live GUI/external expansion checks remain pending.

## CloudKit and release gates

- Fresh explicit destination: `iCloud.sh.saeed.kudzu.library`. The new configuration
  refuses legacy `iCloud.sh.saeed.kudzu`.
- Project profile missing; read-only inspection of 49 installed profiles found no
  authorization for the fresh destination. Local signing identities exist, but
  that does not establish container/profile authorization.
- Preview browser reached Apple sign-in and was closed without changing cloud
  state. Browser selection/login is pending.
- Fresh Development container/app association/profile/schema, actual account
  isolation, primary/join across clients, offline/concurrent changes, real large
  image transfer and Production promotion remain unverified.
- Final live editor/form/drag/VoiceOver and external expansion, login/mixed-version
  behavior and a production-identity upgrade remain unverified.
- The earlier DMG predates SwiftData. Fresh packaging, signing/notarization/stapling,
  updater installation, actual CI and release settings remain separate gates.
  Tauri stays as the parity reference until the final cutover gates clear.

The implementation is saved as a local checkpoint on the long-running
`migration/native-macos` branch. No push, PR or release was created. Temporary
evidence needs archiving before it is treated as a durable delivery record.
