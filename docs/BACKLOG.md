# Backlog

Follow-ups found while reviewing the M0 and M1 branches. These are not scheduled yet, and each one notes the milestone it should land in.

## Engine (M2)

- **No pause after triggers.** The spike uses a listen-only tap and a 30 ms "abort if you keep typing" heuristic. In the Direct build, use an active tap (Accessibility): when a trigger fires, hold incoming keystrokes, insert, then replay them. Fast typists must never see a dropped or garbled expansion. The App Store build keeps listen-only as long as the sandbox forbids active taps.
- **Remove the `Kudzu/Spike/` folder** once `ExpansionMatcher` and `TemplateRenderer` are wired into the app.
- **Use the right delete count for whole-word matches.** Pick swallow-and-re-emit or `deleteCountIncludingSeparator`, depending on the tap mode (see the `TriggerMatch` docs).

## Editor (M2)

- **Limit trigger length to 64 characters**, the `KeyBuffer` capacity. Longer triggers can never fire.
- **Show `TriggerConflicts` and template diagnostics inline.**

## Templates

- Accept bare numbers for string attributes (`{{field name=qty default=5}}`). Today the whole tag becomes literal text.
- Pass the root snippet's ID into the renderer, so a self-reference is caught before it expands once.

## Interchange

- Replace `TemplateTagFormatter`'s private template scanner with `TemplateParser` and `TemplateSerializer`. It only exists because the layers were built in parallel.

## Sync (M4)

- If SwiftData doesn't post `NSPersistentStoreRemoteChange` for CloudKit imports, call `KudzuStore.noteRemoteChange()` from `NSPersistentCloudKitContainer.eventChangedNotification`.
- Regenerate `deviceID` when the store is restored from another Mac's backup (for example on a hardware UUID mismatch).
- **Before the first production CloudKit schema deploy:** review every field name in `KudzuSchemaV1`, because the schema is additive-only after that.

## Distribution

- **Decide whether the App Store build is viable** once the M0 manual tests are done (`docs/compat.md`). The open questions are whether the sandbox allows Accessibility for caret placement and whether it allows sending keystrokes to other apps.
