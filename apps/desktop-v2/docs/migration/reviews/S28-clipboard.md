# S28 review — clipboard service

Independent review of S28 (Claude Sonnet). Fixed in `refactor(desktop-v2): S28 review fixes for capture order, locking and the agent API`.

**S28 review.** The privacy markers, the ignored-apps list, the 0700/0600 modes and the secret override surviving a recopy and a relaunch are correct. I read the code and ran `cargo test -p kudzu-app clipboard` in the implementer's worktree: 24 tests passed. The findings below come from reading, not from tests. Paths are under `apps/desktop-v2/crates/kudzu-app/src/clipboard/`.

## Blocker
None.

## Major

1. **A copy reads the whole pasteboard before any skip check.**
   - `mod.rs:306-316` calls `board.read()` first. It then checks `own`, the private markers, the frontmost app and the ignored apps. Today's app skips on `is_own_change`, recording-off and ignored apps before it reads. §1.2 line 217 requires reading "only when the count changed and the change is not ours".
   - Your own copy-back of a 64 MiB image gets read in full and thrown away. A password-manager copy is read into memory before it is dropped.
   - Fix: resolve `frontmost()` and the ignored list before `read()`. Add `Pasteboard::is_own(change_count)` to the port, and also take `last_change` from `snapshot.change_count`.

2. **I/O runs under the state mutex, and the hot read paths take that mutex.**
   - `change()` holds `state` while the closure writes the blob and thumbnail, with four fsyncs and up to 64 MiB (`mod.rs:191-203`, `346-363`). `remove_with` writes and fsyncs the journal under it (`mod.rs:233`). `collect` unlinks up to 10,000 files and rewrites the journal under it (`mod.rs:244-277`).
   - `watch()` also takes `state` (`mod.rs:177-182`). So do `history_text`, `text`, `copy`, `media` and `search`. `history_text` is on the expansion path, so `{{clipboard history}}` can stall behind a large capture.
   - Fix: store a `Watch<History>` clone in the service so `watch()` never locks `state`. Write blobs before taking the lock; only the ingest and submit need it.

3. **The cleanup journal is redundant, stores clipboard text, and can stop the service opening.**
   - `Store::clean` (`storage.rs:135-152`) already restores live entries from trash, deletes any file in `items/` or `thumbs/` whose entry is gone, and empties `trash/`. That covers everything `recover_jobs` does (`storage.rs:112-134`).
   - `cleanup.json` holds full `ClipEntry` values, including 2,048-character excerpts. A secret the user just deleted stays in it until cleanup succeeds, and indefinitely if a move keeps failing. Each delete, clear, prune and collect rewrites and fsyncs the whole file.
   - A corrupt journal returns `Decode` (`storage.rs:119-120`), so `open()` fails on every launch. Nothing moves it aside, unlike `history.json`.
   - Fix: drop `recover_jobs` and keep the pending-cleanup list in memory only. If you keep the journal, store ids only and treat a decode failure as empty.

4. **Search returns cloned entries, which breaks the 5 ms budget.**
   - `query()` returns `Vec<ClipEntry>` cloned (`search.rs:40-47`). With an empty query and filter `All` that is about 5,000 clones of 2 KB excerpts and 64 timings on every call. That cannot meet "5,000 clips ≤ 5 ms p95" (PLAN §1.7), and S58 and S60 call it per keystroke.
   - Fix: return ids or indices into an `Arc<History>` snapshot, with the revision, or return `Arc<ClipEntry>`.

5. **`AgentClip` is too thin for S32, which would have to bypass redaction.**
   - `AgentClip` is `{id, kind, excerpt}` (`search.rs:12-16`). The existing `list_clipboard` and `get_clipboard_item` tools also return copiedAt, copyCount, pinned, length and lines (withheld for secrets), app, files, image, color and a `withheld` reason. `get_clipboard_item` returns the full redacted text, up to `MAX_ITEM_CHARS`.
   - The service offers no agent-safe full text or agent-safe `history_text`. `text(id)` and `history_text` return raw text, including secret entries. Today's `older_copy` filters `!is_secret()`.
   - S32 would have to read `watch()` and redact again, which spreads rule 9 across two crates.
   - Fix: add `agent_item(id) -> Option<AgentItem>` with redacted full text, the metadata above and a withheld reason, plus `agent_history_text(n, current)` that skips secret entries.

6. **File icons pass a path to a bundle-id API.**
   - `MediaKind::Icon(i)` calls `apps.icon(e.files.get(i)?, 64)` (`mod.rs:517-520`), but `AppCatalog::icon` takes `bundle_id` (`ports/apps.rs:6`). The old `app_icon_png(path)` rendered Finder icons for files.
   - S35 can't implement this faithfully, and S30's icon cache would be keyed by file path.
   - Fix: add `FileIcons::file_icon(path, px)` or equivalent to the ports. S35 also needs `read()` to keep marker types for every item, even when their data is empty or promised.

## Minor

1. **`clean()` never removes the temp file of a live entry.** `storage.rs:142-145` keys on `file_stem()`. For `<uuid>.tmp-<uuid>` that stem is `<uuid>`, so a leftover from a crashed write of a live entry is kept forever and counted by `usage()`. `cleanup.tmp-*` in the root is never scanned. Fix: delete any file whose extension is not `kzc` or `png`, and sweep the root.

2. **A recopy of an image or file rewrites the blob under the lock.** `mod.rs:351-356` rewrites a blob of up to 64 MiB on every recopy. Today's app skips this for an unchanged image (`mod.rs:178` there). Fix: skip when `Repeated` and the blob exists.

3. **A capture can be counted twice or lost.**
   - `mod.rs:293-295` stores the earlier `change_count` as `last_change`, not the count of the snapshot it read. A copy landing between the two calls is read twice, so `copy_count` and the timeline gain a phantom copy.
   - A failing `prune()?` or `read()?` also drops the copy, because `last_change` has already moved. Runtime errors are discarded with `let _ =` (`runtime.rs:55-62`) except `collect`'s. Fix: advance `last_change` only after success, and publish a capture error like `storage_error`.

4. **After a failed capture write, the new blob is left behind.** A failed thumbnail write or index submit leaves a new entry's blob on disk until launch (`mod.rs:346-363`). Fix: remove it on the error path.

5. **`collect()` clones the cleanup list on every 300 ms tick.** `mod.rs:249` clones all pending jobs, up to 5,000 entries, every tick until the index is durable. A stuck move also retries every tick with no backoff. Fix: return early when no job is at or below the durable revision, and back off after an error.

6. **The agent search cache builds both scopes eagerly.** `search.rs:29-33` clones the entries twice and runs the secret scanner over about 10 MB on every revision, even when only local queries run. Fix: build each scope lazily, and share the entries through an `Arc`.

7. **`prune()` and `change()` clone the whole history on every capture.** `remove_with` clones before it knows anything expires (`mod.rs:217-221`), so each capture clones about 5,000 entries twice. Fix: check for work first, e.g. `History::needs_prune`.

8. **`history_text` reads blobs just to compare.** `mod.rs:391-410` reads the whole newest blob on every call and may read the pasteboard. Fix: compare `fnv1a(b"t", current)` with `newest.hash` and skip the blob.

9. **A long entry flagged as a secret still shows its head to agents.** When `is_secret()` is true only because of text beyond the excerpt, `agent_query` returns the first 2,048 characters, because it ignores `e.secret` (`search.rs:57-74`). The `ClipEntry.secret` doc says "withheld from agents". Today's app behaves the same; withholding is the safer choice.

10. **Rescanning for new secret rules only reads excerpts.** `rescan_secrets` ignores the blob, so a long copy with a secret past 2,048 characters is never newly flagged after `SECRET_RULES` changes. Fix: rescan long entries from the blob at launch.

11. **Deleted data can stay on disk until the next launch.** `shutdown()` runs no final `collect` after the writer drains, so a clear or delete made in the last 0.9–5 s keeps its blobs and thumbnails until the next launch. Deleted items also sit in `trash/` until then; today's app does the same.

12. **Small paste, copy and open gaps.**
    - `copy()` returns `TooLarge` when a blob is missing but the entry is under 64 MiB (`mod.rs:474`); `Missing` or a "can't be read" error would be accurate.
    - `copy()` returns `Missing` after the board write if the entry was deleted in between.
    - `PickSession::paste` blocks for up to about 850 ms. Document that S60 must hide the panel first and call it on a worker, as today's app does.
    - `DeletedClips` has no accessors for S58's toast and secret-title rules.
    - `usage()` walks the directories on every call, so cache it per revision.
    - `Store::open` creates directories at the default mode before it narrows them to 0700.
    - A corrupt-index launch deletes every blob in the same launch (`clean`), which makes the quarantined index useless. Today's app does the same.

13. **Rule 7.**
    - `mod.rs` is 568 lines, over the 400 target; `PickSession` and `media` could move to `pick.rs` and `media.rs`.
    - `ClipboardService::open` takes eight arguments and needs `#[allow(clippy::too_many_arguments)]`; group the ports into a struct.
    - `storage.rs:61` and `:72` use `unwrap()` outside tests.

**Verdict:** Privacy and the main data-integrity ordering hold. Blobs are written before the index, and cleanup waits for the durable revision, so the index never points at a missing blob. I would send it back for majors 1–6 before S32, S35, S58 and S60 build on this API.