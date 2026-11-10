# Migration plan review

Reviewed 2026-11-11. Application source: `d5299ec074a3e84dee97ec2864caa3a5e0b24bd5`. Rune source: `29aacb688883a9712bc27175bdafd99fc6edb421`, verified in the supplied clone. `PLAN.md`, `FEATURES.md`, `RUNE_FIT.md` and `CRATES.md` below mean the files beside this report. `Rune:` paths are relative to that clone. Other paths are repository-relative.

This is a source and plan review. No build, installation, test suite, application launch, permission change, issue submission or release operation was performed. Behavioral counterexamples below are source-derived, not claimed live reproductions. The only file written is this report.

## Blockers

### B1 — Headless adapter selection runs before Rune publishes headless state

**Plan:** §1.3, lines 217, 262; S40, lines 875–878.

**Problem:** `compose.rs` selects fixtures with `headless_run().is_some()` before constructing the windows. That returns `None` inside the `run_windows_with_args` factory. Consequently a tree/snapshot invocation can construct production adapters, open real data directories, start clipboard capture or attempt native service initialization. A CI scene is not isolated by the mechanism specified.

**Evidence:** `Rune:crates/rune/src/tooling.rs:234–269` calls `factory(&displays)` before entering the headless renderer. `tooling/headless.rs:89–104` publishes the headless context later, before state initialization. `tooling/headless_run.rs:190–192` explicitly says a `run_windows` factory runs too early to see it. S40 constructs `Ports::macos`/fixtures and `Kudzu::new` before windows.

**Fix:** Resolve an explicit launch mode before constructing services, or move service construction into a shared initialization point where Rune has published the context. Include listing-only invocations, which return before that context is published. Add a process-level acceptance check for `--tree`, `--snap`, `--list-windows` and `--list-scenes`: native adapter constructors must be unreachable, no socket/login/clipboard activity occurs, and sentinel real data is unchanged. Do this in S01/S25, before accepting any headless evidence.

### B2 — Save acknowledgment and quit ordering cannot deliver the promised failure recovery

**Plan:** §1.2 lines 194–210, 244; §1.3 line 253; §1.6 line 358; S25, S51, S55.

**Problem:** The plan combines cheap synchronous mutations, one deferred persistence writer, and mutation `Result`s that supposedly report disk failure. It never defines which revision has actually reached disk. S51 can therefore mark a draft saved before the writer fails; S55 cannot know when to close. Quit compounds this: `Shell::quit` shuts services down before calling Rune quit, while editor drafts are window-local and flush on close. The system-quit path is also too late for asynchronous draft saves. Retrying a writer alone does not preserve a draft after navigation or process exit.

**Evidence:** `PLAN.md:135–150,197,205–210,244,253,926,962`. `Rune:crates/rune/src/app.rs:422–431` requires synchronous survival work in `on_quit`; `window/app_events.rs:13–20` runs quit hooks before close hooks. `app.rs:663–665` says `hide()` does not invoke the close callback. Thus Cmd-Q hiding windows is not a draft-flush hook either. The old failure is real: `apps/desktop/components/app/editor/snippet-editor.tsx:80–97` clears dirty before the save completes.

**Fix:** Specify revisioned save acknowledgments and distinguish accepted in-memory state from durable state. Keep failed/in-flight drafts owned beyond an editor remount. Define one shutdown order: stop accepting new work, transfer/flush UI drafts, drain accepted mutations and persistence, stop native producers safely, install a verified update, then exit. Normal quit can prepare asynchronously; the unavoidable system-quit hook needs a documented synchronous finalization path. It must not join a worker waiting on the UI loop: Rune shortcut registration has exactly that hazard (`shortcuts/mod.rs:125–128`). Add delayed-write, disk-failure, edit-during-save, navigation, hide, Cmd-Q, tray Quit, system quit and immediate-relaunch tests.

## Major findings

### M1 — The library loses its transaction boundary, and a latest-value watch is not a change journal

**Plan:** §1.2 lines 135–176, 193–197, 213; §1.6 lines 386–388; S26, S29, S32, S33, S37.

**Problem:** Removing the MCP host's lock-held edit is reasonable only if its atomicity moves into the library service. The specified CRUD methods do not preserve one transaction for collision checks, group-path creation and snippet creation/update. Similarly, merging a remote snapshot and then calling `replace` can overwrite an intervening UI edit. Separate locks do not solve either race. The reaction table additionally requires `Origin` and deletion history, while `Watch` retains only the latest snapshot and its wakers may only enqueue work. A Local edit followed by a Sync replacement before a queued reaction runs loses the earlier origin unless pending effects are retained elsewhere.

**Evidence:** The existing atomic MCP operation clones, validates and commits under the store lock at `apps/desktop/src-tauri/src/agent.rs:162–171`; tool policies run against that same library (`crates/kudzu-core/src/agent/tools.rs:630–745`). Existing remote application merges library and sync index under locks (`src-tauri/src/cloud/apple.rs:516–538`). The plan removes the former closure and proposes whole-library replacement, without a revision precondition or transactional merge API. S25's watch tests cover unlock/unsubscribe, not coalesced origins or concurrent commits.

**Fix:** Put atomic domain operations in the library owner: validated agent patches, import merges and remote changes against the current revision. Keep protocol DTOs in the agent adapter. Return a committed revision and maintain pending sync/deletion effects as durable or explicitly drainable domain state; use `Watch` only for latest-state observation. Specify ordered engine-index publication and CloudKit index/checkpoint acknowledgment, including stale callbacks after account/enable changes. Test concurrent UI/MCP edits, duplicate trigger races, failed nested group creation, local-edit/remote-merge interleavings, coalesced notifications and restart after checkpoint failure.

### M2 — Form/session ownership and suppression do not preserve interrupted-form safety

**Plan:** §1.2 lines 138, 179–184; §1.3 lines 228, 250–253; S27, S40, S57, S71.

**Problem:** `Watch<Option<Arc<FormSession>>>` publishes a shared session, but `submit`/`cancel` are said to consume it and the window supposedly owns its lifetime. Those are different ownership models. Hidden Rune windows keep their state, so hiding does not drop a stored session or its suppression guard. More seriously, a form stays open on blur, yet a lifetime-long suppression guard prevents the copied engine from detecting typing in the target document. The plan promises FRM-4 without resolving that contradiction. Making panels nonactivating also changes the usefulness of a frontmost-process check.

**Evidence:** `FEATURES.md:123–128` requires untouched typed triggers and rejection of interrupted submissions. Current `src-tauri/src/host.rs:51–54` sets suppression for a form; `crates/kudzu-engine/src/macos/engine.rs:555–560,582–589` ignores outside clicks/typing while suppressed; `1052–1065` relies on `form.interrupted` to reject destructive submission. This is a preexisting source-level gap in the checklist's assumed guarantee, not proof of a new regression. Rune launcher panels explicitly do not activate the application (`Rune:crates/rune/src/window/spec.rs:104–110`), and `AppHandle::hide` retains state (`app.rs:663–665`).

**Fix:** Publish an immutable form descriptor/token; keep one consumable session owner or implement an explicit exactly-once terminal state. Separate suppression of matching from observation of target-document invalidation. Define focus/blur, hide, replacement, cancellation, failed presentation and Cmd-Q transitions, and explicit guard release. Prove: open typed form, blur to the target, type/click, return and submit causes no deletion; editing the form itself remains valid; stale tokens and duplicate submits do nothing; cancelled forms preserve the full trigger. Add this to the early panel/editor integration gate, not only S71.

### M3 — The declared dependency gate rejects the declared dependency set

**Plan:** §1.1 lines 91–107; S01, S20, S25; §2 row 30.

**Problem:** `kudzu-app` forbids `core-foundation*` anywhere in its dependency tree while depending on the copied core's default-feature `chrono`. On macOS that tree includes `iana-time-zone → core-foundation-sys`. This is an actual dependency contradiction, not an architectural preference. A literal ban on every path outside the workspace is also too broad if applied to downloaded Rune packages, whose manifests have their own internal path dependencies. Conversely, scanning source strings cannot prove arbitrary `build.rs` programs never read outside the workspace.

**Evidence:** `apps/desktop/src-tauri/Cargo.toml:68–75` enables Chrono defaults; `crates/kudzu-core/Cargo.toml:12` inherits it. `src-tauri/Cargo.lock:513–524,1722–1734` records the offending chain. Chrono's [feature documentation](https://docs.rs/crate/chrono/latest/features) confirms default `clock` enables `iana-time-zone`. `Rune:crates/rune/Cargo.toml` contains paths to sibling Rune crates.

**Fix:** Define the rule over first-party direct architectural dependencies, with target-aware treatment of approved transitive platform support, or disable Chrono clock features and move all clock/local-zone acquisition behind the port in the same step. Scope workspace-containment checks to first-party sources/dependencies; allow immutable registry/git sources. Call the source scanner a lint, and separately review build scripts. Add a fixture of the approved resolved graph so S01 proves the real allow-list passes, not just that three artificial bad edges fail.

### M4 — Steps cannot stay green within their own write boundaries

**Plan:** §4 waves/merge rules; S01, S20–S21, S25, S29, S32, S34–S35, S40, S54, S73.

**Problem:** The ownership and dependency schedule has build-breaking gaps. Empty module declarations do not supply types, method signatures or constructors to compile callers against, and union-merging module lines cannot repair an API change.

**Evidence and fixes:**

- S20 moves `types.rs` and matching/library APIs but owns only core. Existing engine imports `kudzu_core::types` in `library.rs:8`, `plan.rs:8`, `macos/engine.rs:22`. S34, which updates those callers, is a later wave. Move callers in the same commit or explicitly schedule a temporary compatibility export and its deletion.
- S20 owns/implements `library/usage`, then S21 implements its aggregation part. Define the public contract before either step. S25 builds `Kudzu::new` with fields for services not implemented until later waves; name a compiling staged constructor strategy rather than empty files.
- S29 moves `BackupStore`/coordinator APIs after S26 is accepted; identify every affected caller in S29's scope. S32 removes `core::agent` but owns neither core's `lib.rs` nor its README, both created by S20. Include those files in the removal unit.
- S40 promises real services and `Ports::macos`, but runs in parallel with S35/S37 and does not depend on S28, S29, S31–S33 or S36. Generate the wave order from the DAG and put real composition after all required adapters; a fixture-only shell is a separate earlier unit.
- S54 creates `library/image_compact.rs` after S26 owns the parent module, without an explicit declaration contract. S03 promises a design check added to the workflow but does not own that workflow. The append-only registry exception must be explicit in each affected step.
- S73 deletes the only every-branch push workflow; existing `ci.yml` pushes run on `main` only. With no PR in this plan, “CI is green on the branch” needs a dispatch/branch trigger explicitly included in the cut-over commit.

**Acceptance:** Each step must compile/test in its own checkout against only its declared prerequisites, before integration. Add a machine-readable step/dependency/owned-path manifest and check for same-wave intersections and missing prerequisite APIs. One branch remains the integration target; ephemeral parallel worktrees need an explicit integration owner.

### M5 — Cut-over precedes the independent parity gate and permits unresolved failures

**Plan:** S10–S15 acceptance; S71–S74; §5 risks 1, 5, 15; §6.

**Problem:** S73 deletes the reference app before S74 independently checks the 195 requirements. S74 accepts a gap with a follow-up, while S71 accepts failed cases/budgets recorded for later decisions and S72 allows reviewer-accepted visual differences. That does not implement “cut over when parity is reached.” Most spikes are complete when questions are answered, even if the answer is that a required interaction cannot work; only the editor has an explicit stop rule.

**Evidence:** `PLAN.md:628,637,655,673,1084,1093,1104,1123–1128,1138`. Native input safety, Cmd-Q, unsigned-versus-production behavior and a reviewer-approved visual deviation can all remain unsettled at deletion time. None of the current owner requirements grants blanket approval to relax parity.

**Fix:** Move the independent parity audit before S73. Give each spike a go/no-go outcome tied to named required behaviors and an approved fallback. Require zero unresolved safety/data-loss gaps and owner approval for each intentional product change before cut-over. Keep a short post-rename audit for paths, identity, packaging and registration. Preserve reference fixtures until that final gate passes.

### M6 — The release replacement omits CI keychain setup and its acceptance never requires notarized/Intel execution

**Plan:** §1.6 packaging; S70, S71, S73; §5 risk 15.

**Problem:** The plan names `codesign` and maps updater secrets, but does not assign the temporary keychain import/search-list/unlock/cleanup work formerly performed by Tauri's bundler. Local signing can pass while CI has no usable identity. S70 and S73 explicitly accept `SKIP_NOTARIZE=1`, despite the risk table claiming a full signed release gate. S71 asks for a production build but does not require the final notarized artifact to run on Intel or on the minimum supported macOS. A universal `lipo` result is not execution evidence.

**Evidence:** `apps/desktop/scripts/release/build.sh:5–11,60–75` delegates certificate import and app packaging to Tauri; `.github/workflows/build-macos.yml:77–91` supplies the P12 secrets. `PLAN.md:1072–1073,1082–1085,1112–1116,1152` never supplies the missing CI credential lifecycle. `apps/desktop/RELEASING.md` explicitly distinguishes Dev signing from notarization and Intel execution.

**Fix:** Assign the complete P12/keychain lifecycle to `xtask` or a release script, including restoration/cleanup on failure and profile/certificate matching. Add a nonpublishing release workflow or dispatch that builds the exact release candidate, verifies app and DMG signatures/notarization/staples, and archives evidence. Require installation, real insertion, update and relaunch on Apple Silicon and Intel, plus the supported OS boundary. Verify a v2-to-v2 update; no installed-build migration is needed. After the rename, retest MCP executable paths, `kudzu-dev` registration, login item and bundle resources, since S71 used `kudzu-next`.

### M7 — release-please's Rust strategy contradicts “only the root package is versioned”

**Plan:** §1.1 line 50; S01 line 547; S73 line 1113.

**Problem:** The Rust strategy is not a root-package-only updater when the manifest has workspace members. It collects member manifests and updates their versions too. The proposed `crates/*`/`spikes/*` patterns also need explicit validation: the inspected implementation attempts `${member}/Cargo.toml` rather than expanding a glob. Even with unexpanded globs, the explicit `xtask` member is considered. The fallback `simple` strategy's lone TOML extra-file entry does not specify how the root package entry in `Cargo.lock` changes.

**Evidence:** [release-please Rust strategy](https://raw.githubusercontent.com/googleapis/release-please/main/src/strategies/rust.ts), lines 43–96 and 115–118; `PLAN.md:50,547,1113`. This is current upstream source evidence; the exact action-resolved version still needs the planned dry run.

**Fix:** Choose and test the release versioning policy in S01/S70, not during deletion. Either intentionally version workspace members together or configure a supported root-only manifest/lock updater. Fixture-test the exact action version with the real workspace shape: root version, private member versions, lockfile, changelog and beta stamping. Keep the path isolation test: it is valid, as noted below.

### M8 — Several performance budgets measure the wrong event or omit the measuring implementation

**Plan:** §1.3 line 242; §1.7 lines 412–426; S11, S14, S21–S22, S71.

**Problem:** `RUNE_READY_DIR` does not mean application services are ready or main content is visible; it records the first native window's ready callback, including a hidden prewarmed panel. Hidden launchers do render changed state, contrary to §1.3. Counting `Drawn` is not counting only visible presents. `--trace` is a geometry/motion stream, not by itself a CPU profiler. The trigger timing row requires a fake poster seam, but no step owns its introduction. “No regression” is also undefined across a Tauri dev preview and a native release build, and seven launches do not define a cold-cache procedure or a p95 test.

**Evidence:** `Rune:crates/rune/src/platform/ready_signal.rs:1–7,34–42`; `window/stepping.rs:35–63`; `window/lifecycle.rs:237–261` refreshes hidden launcher frames. `frame/encoded.rs:32–45,71–78` separates encoded/submitted timing from presentation. `tooling/args.rs` describes `--trace` as per-frame rects/opacity. The source audit already lists virtual-scroll correctness/cost problems (`RUNE_FIT.md:231–249`), but S14 has no required fallback if zero commits is impossible at the pin.

**Fix:** Assign one benchmark harness before the spikes. Define app-services-ready, main-first-content, shortcut-received, encode-complete and visible-present markers separately. Record hardware, OS, scale/refresh, release flags, dataset, warm-up, sample counts and cold/warm procedure. Count visible and cached hidden frames separately; identify the idle scene and exclude caret/shimmer animation deliberately. Capture CPU timings via Rune's frame metrics/native instrumentation, not motion trace alone. Add the poster/clock seam and explicit approved outcomes for pin-level budget misses.

### M9 — The visual/fixture harness is specified after its consumers and is not deterministic end to end

**Plan:** S02, S04, S06, S25, S54, S72; §1.7.

**Problem:** Component and screen steps depend on parity machinery, but `xtask parity` is only built in S72. S02 captures product states, not a defined state matrix for every unused primitive file that S06 insists on comparing. Stable screenshots require more than a seeded JSON library: relative dates, scheduled UI work, async `Latest` completion and animation time must agree. App services use real threads/timers while Rune headless time is virtual. Bundled-font goldens are useful regression tests, but a Figtree-rendered golden cannot establish SF Pro text geometry within 2 pt. S54's byte-equal formatting criterion can accidentally require Rust PNG output to equal browser-canvas PNG output.

**Evidence:** `PLAN.md:399,407–409,563,603–604,739–740,953,1092–1093`; `Rune:crates/rune/README.md` headless tooling uses virtual time; `Rune:crates/rune/src/latest.rs` executes jobs on workers. Browser image compaction uses canvas encoding (`apps/desktop/components/app/editor/image-tools.tsx:13–40`), while S54 uses `image`. These encoders are not specified to produce identical bytes.

**Fix:** Build the reference/scene registry, landmark extractor and comparison runner with S02/S06. Inventory only used primitive states, or explicitly add a nonmutating gallery capture fixture. Specify fake wall time, timezone/locale, UUIDs, deterministic executor/completion delivery and service timers in scenes. Separate production-font parity capture from bundled-font regression goldens. Compare decoded image pixels/dimensions/budgets and semantic spans; keep exact serialized checks for fixtures that reuse identical embedded bytes. Specify how headless multi-window scripts use `--desktop`; selecting one retained window alone does not prove cross-window routing.

### M10 — Planned editor substitutions are treated as implementation choices before owner approval

**Plan:** §3 gaps 3, 5–6, 17; S13, S52–S54; §6 questions 1, 2, 4, 5.

**Problem:** Replacing an inline image with an alt-text pill and replacing click-to-configure with selection plus Return/Space are product changes. S54 mandates the image substitution although the owner question is unanswered. S13 only records what clicking selects; S52 never requires that one click opens the tag configuration as today. The same ambiguity affects accepting kit metrics and Popover instead of Sidebar material. “Plan wins” over research is not owner approval.

**Evidence:** `FEATURES.md:83–93`, current `README.md` editor/formatted-snippet contract; `Rune:crates/rune/src/edit/model.rs:14–27` has a pill mark but no image mark. `PLAN.md:486–487,654–655,935,953,1161–1165`. The plan also adds date-example offset correction and universally pruning backups, beyond the three failure fixes enumerated in question 4.

**Fix:** Put each visible/behavioral change in a decision ledger with owner approval and affected feature IDs. Make the approved interaction part of S13's executable gate. If image/chip parity is mandatory, demonstrate a contained Rune workaround before dependent editor work. A reviewer cannot waive owner requirements through the S72 exception column.

### M11 — “Keep ours” sometimes rejects a library for not implementing Kudzu policy rather than its reusable mechanism

**Plan:** §2 rows 3, 7, 14, 23, 27; §1.2 supporting primitives; CRATES concerns 17, 20–21.

**Problem:** Some retention decisions have concrete parity reasons, but others do not meet the owner's reuse rule. The existence of custom tool handlers, import-loss warnings or a no-Tokio architectural choice does not establish that a maintained protocol/parser cannot replace just the mechanism. The plan also introduces a worker pool, cancellation primitive, watch subscriptions and timer heap without a buy/build comparison. It retains hand-declared OS APIs even though its research explicitly says maintained bindings expose the same functions. Line count is not a maintenance comparison.

**Evidence:** `CRATES.md:391–398` acknowledges equivalent CGEvent/AX bindings; current declarations are at `apps/desktop/src-tauri/crates/kudzu-engine/src/macos/ffi.rs:89–134`. `rich/image.rs:7–57` contains a bespoke Base64 codec despite the existing `base64` dependency. For MCP, the [official SDK](https://docs.rs/crate/rmcp/latest) has generic async read/write and custom transports, so the Unix bridge is not a demonstrated blocker; it does introduce Tokio, which is a real cost. The [transport API](https://docs.rs/rmcp/latest/rmcp/transport/index.html) supports adapting byte streams. Tool policy can remain in Kudzu either way.

**Fix:** Before locking §2, compare the smallest reusable mechanism against the existing behavior fixtures and error/bounds contract. Record measurable adoption cost and an explicit maintenance owner for anything kept. Specifically evaluate an isolated MCP adapter/runtime, supported FFI bindings, Base64/CSV parsing beneath existing policy, and simpler standard-library scheduling before building an infrastructure mini-framework. This is a request for a decision and owner dependency approval, not an instruction to add dependencies during this review. The decision-by-decision assessment is below.

### M12 — Preserving test counts does not cover the native safety contracts being reshaped

**Plan:** S01, S27–S28, S34–S35, S71; §1.7 engine coverage.

**Problem:** The plan repeatedly uses 73 preserved engine tests as its safety net, then postpones the highest-risk protocol cases to a broad manual S71 checklist. Those tests include pure planning/queue tests and wrapper rendering tests, not the end-to-end pasteboard, tap and focus lifecycle. A human exercising “held-input replay” once cannot establish release on timeout, tap loss, cancellation and shutdown, or a second trigger during replay. Ordinary paste success does not establish compare-before-restore safety.

**Evidence:** `apps/desktop/src-tauri/crates/kudzu-engine/tests/engine.rs:1` explicitly never starts a live tap; `hold.rs:71–107` tests the queue in isolation. `macos/pasteboard.rs:15–28,164–190` implements delayed restore/global bookkeeping with no adjacent tests. `FEATURES.md:45–54,126–128` bundles numerous race/failure outcomes into single checkboxes. S35's real-adapter acceptance expressly excludes permission-requiring cases.

**Fix:** Assign a narrow safety harness before restructuring: controlled poster, pasteboard, monotonic clock and target/focus observations around the existing insertion coordinator. Assert the complete sequence of output and retained state for newer clipboard copies, unreadable flavors, consecutive pastes, failure/shutdown restore, held key-down/up replay and cancellation/deadline/tap-loss, unknown image delivery and form interruption. Keep live S71 proof for real application delivery and permissions; neither layer substitutes for the other. Do not make all platform APIs into traits solely for tests—put seams around these protocols.

## Minor findings

### N1 — The privacy invariant literally prohibits the application's intended storage

**Plan:** Rule 9, line 40; §1.6 lines 343–350.

“Never … persist … clipboard contents or snippet bodies” forbids `library.json`, backups and clipboard blobs. The intended restriction is telemetry/activity/logging, not authorized local product storage. `FEATURES.md:150–156,195,210` explicitly requires those stores. Rewrite the invariant by sink: local content storage, sync allow-list, MCP redaction, and content-free diagnostics. Test that an I/O error's `source()` does not leak content through logging.

### N2 — Global-state exceptions and lifecycle ownership are not explicit

**Plan:** §1.5; S34, S37; S25.

S37 forbids static CloudKit state, but S34 preserves mutable pasteboard statics (`macos/pasteboard.rs:24–28`) and other process-bound native state without stating which are intentional. The system pasteboard genuinely is process-shared, so arbitrary duplication is worse. Document one owner for restore/own-change bookkeeping, its serialization and shutdown order; make it injectable for the safety harness or explicitly justify a process singleton. Keep `Kudzu` at composition/shutdown only: do not pass the all-services registry into windows that currently specify narrow service bundles.

### N3 — Session sizes and reviewer policy are labels rather than executable scheduling constraints

**Plan:** §4 lines 506–510; S25, S28, S40, S71–S74.

58 steps was independently counted. The stated 2,000-line ceiling does not account for copied code, tests, fixture files or generated assets. S25's four concurrency/persistence primitives plus every port/fake, and S71's multiple target apps/layouts/two-Mac sync/release/performance/accessibility matrix, are not credible single bounded sessions as written. Every step names a reviewer and the difficult engine/editor/CloudKit steps receive high-effort models, but provider availability was not checked and a provider's display name is not a quality gate. Split these units by a finished behavior/proof; give the integration owner explicit responsibility for reviews and conflicts. Define what happens when a named model is unavailable instead of assuming a silent substitution.

## Feature-to-step trace

All **195** checkbox IDs appear in the plan's `Closes` lists, expanding ranges; none is textually unassigned. The following **72** checks were examined individually across all 18 domains. **Specified** means deliverables and observable criteria exist, not that implementation passes. **Incomplete** identifies an acceptance or architecture hole. **Changed** requires an explicit decision. Parentheses give the line in `FEATURES.md`; the referenced step contains the claimed implementation/acceptance.

| Check | Claimed step(s) | Review of deliverable and acceptance |
|---|---|---|
| EXP-5 (40), case propagation | S01/S34 | Specified by preserved matcher rules/tests. Keep punctuation, uncased scripts, leading digits and multi-letter uppercasing fixtures. |
| EXP-7 (42), offsets after case conversion | S34/S71 | Specified at pure-plan level; `plan.rs:134–192,343–370` covers expansion of offsets and image flavor. Live styled/cursor delivery still separate. |
| EXP-10 (45), clipboard restore | S34/S71 | Incomplete: no scheduled comparison/restore harness for newer copy, unreadable flavors, consecutive pastes and shutdown. M12. |
| EXP-12 (47), Backspace undo | S01/S34/S71 | Pure matcher/plan retention and live Backspace are assigned. Add interruption/cursor/action distinctions to the native case list; M12. |
| EXP-13 (48), image-delivery certainty | S34/S71 | Incomplete: pure `image_delivery` tests remain, but “formatted and image paste” does not require testing unknown AX delivery suppressing undo. M12. |
| EXP-14 (49), held-input replay | S34/S71 | Incomplete: queue tests are not tap/worker integration. Explicit FIFO, key-up, nested trigger and all release causes needed. M12. |
| EXP-15 (50), sequence cancellation | S34/S71 | Incomplete: broad EXP range names the check, without controllable focus/pause/posting/generation/deadline cases. M12. |
| EXP-16 (51), own-window suppression | S27/S40/S71 | Incomplete: retained windows and lifetime guards require explicit hide/blur/failed-show transitions. M2. |
| EXP-17 (52), layouts/IME/Secure Input interaction | S34/S71 | Specified layout matrix includes Japanese and Telex; verify physical copy/paste keys and Roman/composing modes, not just successful ASCII typing. |
| EXP-18 (53), permission/tap transitions | S34/S71 | Incomplete: “Secure Input” and permission checks do not enumerate active/listen-only upgrade, one listener and posting-vs-listening. M12. |
| EXP-19 (54), usage-only reload | S27/S34/S71 | Specified fake-engine no-op test; add an armed undo and partial trigger to show no invalidation, matching `engine.rs:360–373`. |
| TPL-2 (60), unknown/verbatim preservation | S01/S23/S52 | Specified: byte-exact unknown/verbatim source plus grammar round-trip, with real binding tests. |
| TPL-5 (63), clipboard history alignment | S28 | Specified: full blob lookup, disabled recording, skip-newest only on equality; test private/ignored current copies explicitly. |
| TPL-6 (64), selection capture | S27/S56/S71 | Capture-before-show is explicit. Incomplete selection ownership across palette-to-form and failed/cancelled capture; M2. |
| TPL-15 (73), source/formatting mapping | S01/S23/S54 | Specified semantic span/UTF-16/escape/chip round-trips; image encoder bytes need M9's distinction. |
| TPL-17 (75), render without actions | S07/S27/S32 | Specified preview badges plus retained MCP tests; assert copy also never executes keys/delays and diagnostics survive. |
| EDT-2 (80), autosave/external edits | S51 | Incomplete: explicit failed-save dirty flag is good acceptance wording, but cannot work without acknowledgment/navigation/shutdown ownership. B2. |
| EDT-5 (83), atomic chips/history | S23/S52 | Incomplete: atom/history/IME tests are named; spellcheck and standard field keymap from the bundled check lack explicit acceptance. Add them. |
| EDT-8 (86), click/configure/popover | S52/S53 | Changed/undecided: selection plus Return/Space is not proven one-click configuration. M10. |
| EDT-13 (91), marks/IME/undo | S23/S54 | Specified selection expansion, independent style undo, adjacent-image typing and composition gating. |
| EDT-15 (93), image compaction | S23/S54 | Specified dimensions, attempt/size limits and cancellation; editor appearance changed; byte-identical PNG expectation must be removed. M9/M10. |
| RUL-4 (103), insertion precedence | S01/S34 | Specified by preserved matching tests; preserve explicit Automatic and composing-method override at the adapter boundary. |
| RUL-9 (108), explicit insertion exception | S27 | Explicitly preserved; do not route it through typed-match eligibility. The owner's inconsistency decision still needs recording. |
| QS-1 (112), open/reset/capture/suppress | S27/S40/S56 | Incomplete lifecycle ownership and real-adapter ordering. M2/M4. |
| QS-5 (116), dismiss/focus/copy delay | S56 | Specified Escape/blur/260 ms rules; verify hide before insertion and target activation under the nonactivating panel. |
| FRM-1 (123), trigger remains until submit | S27/S57/S71 | Incomplete: nested discovery exists, but typed-form preparation/interruption must be tested as a protocol. M2/M12. |
| FRM-3 (125), remembered values | S27 | Specified precedence, smallest-UUID eviction and deletion pruning. Ensure pruning is not lost through coalesced watches. M1. |
| FRM-4 (126), stale/interrupted form safety | S71 | Incomplete and contradicted by the copied suppression path. Needs an earlier implementation and regression owner. M2. |
| FRM-5 (127), submit/cancel order | S27/S57 | Incomplete: submit order lacks durable-memory failure behavior and exactly-once session/guard ownership. B2/M2. |
| FRM-6 (128), size/no blur dismissal | S57 | Specified no blur dismissal; changed UI height ceiling: plan uses 680, current UI requests at most 640. Preserve 640 plus native clamp or approve the change. |
| LIB-7 (138), deletion undo | S26/S41 | Domain restores missing IDs and toast lasts 6 s; incomplete select-first-restored/deleted-editor-focus acceptance. Preserve the explicit no-form-memory-undo behavior. |
| LIB-11 (142), drag/reparent | S14/S42/S50 | Specified source/target/cycle/reject/hover behavior; S14 must become a go/no-go gate. |
| LIB-14 (145), main shortcuts | S41 | Incomplete: full action table is promised but acceptance scripts list only a subset. Add Cmd-L/E/D, favorite/enabled, deletion/input/modal and IME guards. |
| CLP-1 (150), capture/privacy gates | S28 | Specified all major skip conditions and timing; add private marker on a later pasteboard item and independent expansion pause. |
| CLP-5 (154), retention/pins/reuse | S01/S28 | Specified cap/age logic via copied tests and scheduled prune; verify pinned exemption and reuse without a new copy tick. |
| CLP-6 (155), media bounds | S28 | Incomplete: 64 MiB blob, 480 px and 80 MiB preview are explicit, but the 40 MiB single-file-thumbnail input cap is missing from acceptance. |
| CLP-7 (156), index/trash/privacy | S28 | Specified 0700, 900 ms, retry, corrupt-index, launch GC and trash Undo; writer/durability contract remains B2. |
| CLP-17 (166), secret masking | S07/S59 | Specified painting/tree concealment; add copy/selection/assistive-value checks so hidden text is not selectable or announced. |
| CLP-18 (167), overrides | S28/S59/S32 | Incomplete: persistence is claimed but S28 omits override survival across recopy/relaunch. Existing MCP tests cover override semantics; add the cross-layer lifecycle. |
| PKR-1 (173), disable while open | S30/S40/S60 | Incomplete: unregistering shortcut and refusing new open are explicit; disabling recording must also hide the currently open picker and release its guard. |
| PKR-5 (177), paste leaves clipboard | S28/S35/S71 | Copy fallback and target-gone error are explicit; add an assertion that this path never restores the old clipboard and preserves 800 ms/40 ms activation semantics. |
| MCP-1 (182), headless stdio | S03/S32 | Specified dispatch before GUI and offline listing; test executable path/identity after S73 too. M6. |
| MCP-5 (186), atomic edits | S26/S32 | Incomplete: removed edit transaction is not replaced with an equivalent service operation. M1. |
| MCP-6 (187), safe secret/format edits | S32 | Existing tests explicitly protect these fields (`agent/tests.rs:285`); retained tests are required, including exact unchanged content and no redaction-marker writes. |
| MCP-7 (188), output/search redaction | S32 | Specified by preserved tests (`agent/tests.rs:168,368,411`); no new generic expansion path may bypass the MCP render/privacy context. |
| MCP-8 (189), socket/reconnect | S32 | Specified live/stale sockets, 0600, no uncertain replay and access-off behavior. Keep agent transport shutdown ahead of final persistence drain. |
| MCP-9 (190), setup/resource/skill | S32/S62 | Setup strings and install states explicit; resource URI/content and final production/Dev executable launch need assertions. M6. |
| ACT-1 (195), ring/privacy | S27 | Specified 100 entries, monotonic IDs and no persistence; correct the overbroad global privacy rule. N1. |
| ACT-5 (199), initial-load race | S25/S63 | Incomplete: a snapshot watch can eliminate event merging, but must subscribe/read race-safely; no such acceptance test is named. Add an event between subscription and initial pull. |
| INS-4 (206), WPM/motion | S24/S64 | Specified bounds, default, persistence, timings and Reduce Motion. Deterministic clock needed for its motion gate. M9. |
| DAT-2 (211), import transaction | S29 | Parse/pre-backup gate is explicit; merge must use current revision and remain atomic against MCP/sync. M1. |
| DAT-11 (220), backups/restore | S29 | Specified naming/order/cadence and safety backup. Changed immediate prune-after-every-write policy needs ledger entry; define behavior when backup succeeds but library commit fails. |
| DAT-13 (222), first-run/quarantine | S26 | Legacy-directory copy intentionally omitted, consistent with no migration requirement. Quarantine failure must block overwrite/default save; add that failure test. |
| SET-3 (228), recorder/rollback | S24/S30/S61 | Specified physical chord rules/collision rollback. Rune provides actual runtime registration errors and recorder cancellation; test blur/Escape and same-process two-slot collision. |
| SET-12 (237), defaults/side effects | S26/S30/S61 | Incomplete: acceptance expects login/Dock side effects, but the composition reaction table omits login handling and transactional setting failure semantics. B2/M4. |
| ONB-2 (242), real permission readiness | S30 | Specified Accessibility + posting + attached tap, with pause/Secure Input separate and no synthetic grant. |
| ONB-3 (243), partial-import recovery | S30/S65 | Specified successful-folder skip, retained counts, net additions, retry and restored focus. |
| ONB-8 (248), permission guide | S15/S66 | Specified finite cycle and stop causes; headless overlay cannot prove native click-through/Spaces, so keep the live signed-bundle gate. |
| ONB-10 (250), restart marker | S30/S66 | Marker consumption and reopen specified. Native CloseRequested is distinct from UI close in current behavior; plan should retain that distinction explicitly. |
| SHL-4 (257), Cmd-Q versus quit | S10/S38/S40/S71 | Menu workaround has a concrete spike; draft/worker shutdown and failure-to-install-menu behavior are not safe defaults. B2/M5. |
| SHL-8 (261), updater state/feed | S31/S70/S71 | Specified phases, signature rejection, serialization and schedule. Add cross-artifact feed/signature fixture and complete release gate. M6/M7. |
| SHL-9 (262), deferred install/restart | S31/S36/S71 | Specified no auto-restart, quiet failures and needs-move. Install rollback and quit durability require B2/M6. |
| SHL-11 (264), Dev isolation | S03/S73 | New `next` identity is deliberate isolation; post-rename `dev` identity is not covered by pre-rename native evidence. M6. |
| SHL-12 (265), logs/universal release | S03/S70 | Logging tests are explicit; signed-only universal packaging does not establish notarization or Intel execution. M6. |
| SYN-2 (270), version/unknown keys/schema | S01/S20/S33/S37 | Specified unchanged payload/schema/version and copied merge tests; Production remains add-only. |
| SYN-3 (271), conflict/deletion/usage | S01/S20/S33 | Pure conflict rules retained; applying them to a stale snapshot is not equivalent to an atomic current-library merge. M1. |
| SYN-4 (272), checkpoint/initial fetch | S33/S37 | Initial-fetch/dedupe/record tags are named. Incomplete durability acknowledgment between library/index/native engine state. M1/B2. |
| SYN-5 (273), cadence/generation | S33 | Exact 12-minute crossing specified; stop/restart generation fencing from the checkbox is not explicit in tests. Add a stale scheduled callback case. |
| SYN-6 (274), account/zone changes | S33/S37/S71 | Reset/preserve-local/zone recreation specified; require stale prior-engine callbacks to be ignored and only the named zone processed. M1. |
| WIN-1 (288), native appearance/clipping | S38/S40/S71 | Native appearance/rounding APIs exist; theme changes and clipping need real-window evidence, not PNG-only acceptance. |
| WIN-2 (289), modal behavior | S43/S59/S72 | Sizes are explicit; S72 only checks images/landmarks. Add focus trap, Escape priority, outside dismissal and editor-range restoration scripts. M9. |
| WIN-3 (290), native panel behavior | S71 | Changed from focusable Tauri windows to nonactivating panels. Rune supports that change, but target focus/menu routing needs explicit parity acceptance. M2/M10. |

## Dependency decision audit

Rows refer to §2 of the plan. No dependency was added or approved by this review. Current library documentation was checked for the specific technical claims linked above; release dates/download totals in `CRATES.md` are not treated as proof of maintenance quality.

| Rows | Decision assessment and required evidence |
|---|---|
| 1, 5, 18, 19, 20 | Rune/kit, hotkeys, dialogs, URL open and app directories exist at the pin. Reuse is sound. Pin the source, state feature ownership (`native-file-dialog` belongs to Rune, `markdown` to rune-kit), and accept known limitations only through the gap process. Do not claim Rune handles raw clipboard preservation. |
| 2 | Keeping the secret policy is defensible: whole-item handling, typed redaction markers, placeholder exclusions and per-item overrides are Kudzu contracts. Candidate download totals and an assertion that every scanner is source-oriented do not prove no reusable detector exists. Keep the policy tests; evaluate adding vendor-pattern detection separately, without silently changing disclosure behavior. |
| 3 | Keeping MCP is conditional, not established by transport topology. The official SDK supports custom/byte-stream transports, but brings a runtime and conversion cost. Preserve redaction/write policy outside protocol handling; compare a small adapter against replay fixtures and include revision maintenance cost. M11. |
| 4 | Keeping the exact search algorithm has a concrete parity basis: `lib/search.ts:41–100` defines acceptance, scores and highlights. Rejecting typo-tolerant matching is justified for exact parity. “Already meets the budget” is unproven for an unbuilt Rust port; S21 must measure it. |
| 6 | `auto-launch` supports LaunchAgent mode and arguments in its [0.6 API](https://docs.rs/auto-launch/0.6.0/auto_launch/). Use the new mode API, isolate label/path by identity, and verify disabled/removal/no-relaunch behavior. A Homebrew-style maintenance plan should pin the dependency and test the generated plist; popularity is not sufficient. |
| 7 | A lock is not a reopen transport. Specify OS-held lock versus sentinel, stale-process handling, concurrent first launches and unbundled invocation. Launch Services is acceptable for a bundle if demonstrated; a forty-line estimate is not that demonstration. Do not reuse the optional MCP socket as the instance lock. |
| 8–12 | Blocking HTTP, signature verification/signing, semver and system tar divide mechanisms reasonably. Pin features and versions; specify HTTP deadlines/limits, Tauri key/signature envelope handling, archive-root validation, symlink/traversal policy, bundle identity and swap rollback. Signing and verification both passing against the same new implementation is weaker than an independent known fixture. |
| 13 | Keeping `log` and using a maintained size-rotating sink avoids a tracing rewrite. Verify exact byte threshold, count/name policy and content-free errors; “2 MB” must mean the current 2,000,000 bytes if exact parity is required. |
| 14 | Keeping the current ICU-pattern subset is defensible for output parity; full CLDR output changes locale behavior. The architecture should permit replacing the formatter later. Keep clock acquisition separate from pattern formatting; M3. |
| 15 | `csscolorparser` is a reasonable reusable parser; preserve classification separately. Fixture-test alpha, out-of-gamut/clamping and displayed rounding. Do not assume the candidate's color model automatically matches browser canvas conversion. |
| 16–17 | `image` reuses a library already in Rune, and `base64` is already present. Keep AppKit thumbnail support for HEIC/TIFF/PDF. Specify decoder allocation limits as well as compressed-file limits; compare semantic output rather than encoder bytes. Reconsider the second Base64 implementation in core. |
| 21 | Threads are not inherently simpler once a pool, cancellation, timers and subscriptions are rebuilt. Compare a minimal standard-library implementation with approved maintained options; avoid making “no Tokio anywhere” the premise that decides the MCP question. |
| 22 | Retaining the current YAML parser/emitter is a reasonable scope choice; document why its current pinned version is acceptable rather than claiming tests establish ongoing maintenance. Any update still needs import/export fixtures. |
| 23 | Bounded import conversion and loss reports should remain application policy; that does not rule out a maintained lexer/parser underneath. Separate RTF, HTML, Markdown, CSV and XML decisions. Evaluate limits/leniency/output against fixtures instead of dismissing parsers because they do not implement Kudzu spans. |
| 24–25 | Raw multi-item/flavor clipboard ownership and JSON/blob persistence have concrete product reasons. Standard text/image clipboard wrappers are insufficient. Keep the shared own-write/restore state and fix acknowledgment/transaction semantics; keeping JSON does not justify building general-purpose persistence infrastructure. |
| 26 | Keeping objc2 CloudKit bindings and the pure sync model is appropriate. Native delegate lifetime, stale callback fencing, checkpoint durability and account changes remain Kudzu responsibilities. No schema replacement is needed. |
| 27 | “Works and is tested” does not justify retaining equivalent unsafe CGEvent/AX declarations under the owner's rule. Research itself identifies compatible bindings; evaluate replacing the declarations while retaining TIS or other uncovered APIs. M11. |
| 28 | Small AppKit gap adapters are justified where pinned Rune lacks the API. Raw-pointer entry points need validated provenance/lifetime as well as a main-thread check. Different objc2 versions can coexist, but their wrapper types must not be cast across versions. |
| 29–30 | Reusing plist, typed errors, IDs, time and Unicode crates is reasonable. Avoid parallel hand-written codecs without an explicit bounded/lenient behavior requirement. Chrono's transitive platform support conflicts with the stated gate; M3. |
| 31 | No highlighting/file-watcher adoption is needed to preserve current behavior. Bundled syntax capabilities are not a reason to add a feature during parity work. |
| 32–33 | `create-dmg` and temporary Playwright are tools but still require the listed owner approval. Pin their versions, browser engine and capture environment. Use release assets rather than a moving Homebrew formula as the definition of DMG layout. S02 must declare its approval dependency in wave 1, not “wave 2 onward.” |

## Verified OK

- The proposed first-party crate arrows are acyclic. Direct calls into domain services and immutable UI snapshots are not inherently a re-created IPC/BFF. Narrow per-window bundles avoid a service locator if the all-services `Kudzu` object remains confined to composition. The missing transaction contracts are M1, not an argument for reviving Tauri commands.
- Rune provides heterogeneous windows in one loop, `WindowSpec::from_app`, `AppHandle::wake/update`, retained hidden state, keep-running behavior and reopen hooks (`Rune:crates/rune/src/tooling.rs:190–227`, `app.rs:446–460,630–763`). The exception for hidden launcher rendering is M8.
- Launcher panels are implemented and prewarmed; the old README claim that nonactivating panels are absent is stale (`Rune:crates/rune/src/window/spec.rs:104–110`, `platform/macos.rs:38–58,167–226`). macOS `Tray` really returns Unsupported; `SystemMaterial` really offers Glass/Popover rather than Sidebar (`tray.rs:91–98`, `system_material/mod.rs:18–24`).
- Rune's rich editing model supports pills/marks and composition state, but has no image mark (`edit/model.rs:14–27`, `edit.rs:372–385`). Using rune-kit controls first and limiting proven metric workarounds is a coherent reuse policy; proposed interaction changes still require M10.
- Runtime shortcut registration returns actual backend errors, and rune-kit supplies a recorder (`Rune:crates/rune/src/shortcuts/mod.rs:124–150`, `crates/rune-kit/src/shortcut_recorder.rs:138–173`). File-dialog and URL APIs exist; `file:` opening/reveal requires the stated adapter (`rune/src/open_url.rs:1–4`).
- Keyed virtual rows, row-version hints and id-based handlers match Rune's API contract (`Rune:crates/rune/src/el/virtual_rows.rs:42–67,91–98`). Exact overscan counts are implementation-specific: Rune mounts by viewports, so the plan's 8/10-row values should be treated as baseline performance intent, not a supported parameter.
- Current production and Dev defaults/paths are distinct; rejecting an unbundled binary as production and using a separate `next` identity are deliberate improvements consistent with isolation (`apps/desktop/src-tauri/src/store.rs:76–110`, plan §1.6). No legacy data migration is necessary under the owner's instruction.
- `apps/desktop-v2` is not matched by the `apps/desktop/` release path. The [release-please commit splitter](https://raw.githubusercontent.com/googleapis/release-please/main/src/util/commit-split.ts) matches a slash-terminated package prefix, and the current config names only `apps/desktop`. A dry run against the action version remains appropriate; hidden commit types are secondary, not the path-isolation mechanism.
- The existing updater manifest has version, notes, publication date and both Darwin platform entries pointing to the universal archive (`apps/desktop/scripts/release/latest-json.mjs:18–26`). Stable-only Homebrew and rolling-beta promotion are present in `build-macos.yml:93–109,123–145`; preserving them is explicitly assigned.
- CloudKit container/zone/type/payload preservation and Production add-only rules are correct (`apps/desktop/src-tauri/crates/kudzu-core/src/sync.rs`, `src-tauri/src/cloud/apple.rs:36–39`, `RELEASING.md`). The plan preserves the actual 12-minute usage threshold crossing and makes no unsupported schema-removal proposal.
- All 195 IDs are assigned and all 58 steps name implementers/reviewers. These are coverage/accountability facts, not proof that the steps' acceptance criteria are sufficient.
