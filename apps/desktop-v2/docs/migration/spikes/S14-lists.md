# S14: virtual lists and in-app drag

2026-11-11. Rune `29aacb688883a9712bc27175bdafd99fc6edb421` (verified against the
provided read-only clone). Implementation lives entirely in `spikes/lists`.
No installed Kudzu, system settings, permissions or upstream issues were changed.

## Reproduce

See `spikes/lists/README.md`. Warm target was cloned with the exact requested
`cp -cR` command before the first build. Builds in this worktree were sequential.
Machine: MacBook Pro Mac14,5, Apple M2 Max, 32 GB, macOS 27.2 (26B123), internal
3024×1964 Retina display. Headless viewport 900×600, scale 1, 120 Hz simulated.
Other agents can contend for this machine; timings are observations, not guarantees.

## Source conclusions

- `El::virtual_rows` lazily builds the mounted window; `virtualize` eagerly receives
  every child on a commit. Use the former. Keys are stable ids; handler captures are
  ids, never indices. `virtual_rows_version` must change on a same-size reorder.
- `memo(id, selected_revision, builder)` caches a mounted row, not its durable
  selection. Real rows must include every visible datum in their revision.
- `scroll_into_view(id)` can locate an unbuilt row through the row source. Do not
  substitute `reveal_selected` or `scroll_key_to` (#549).
- `sticky_top` is bounded by its parent. A header that occupies its own virtual
  row vanishes when it unmounts. The spike's first item of each section wraps its
  header; this deliberately does NOT establish sticky headers for the whole section.
  The current clipboard reference uses ordinary virtual headers, not sticky ones.
  If sticky sections are required, use a section row containing its header/items,
  or a live overlay; remeasure before accepting either.
- rune-kit `List` owns eager `Vec<ListItem>` construction and filtering; it is not
  the hot-list primitive. `Tree` uses virtual rows and `TreeModel` handles keyboard
  selection; evaluate it for S42 rather than copying its state into the app.
- Drawn rect callbacks use `tx.live_only()` only for rect bookkeeping. Drag motion
  changes Live offset/opacity/fills and the hit-test result; none changes view output.
  The ghost's count is established by the Down commit. Up clears all live decoration.

## Proposed production dnd.rs API

The spike's `dnd.rs` is executable evidence, not a production implementation.
S42 should implement this small typed interface inside its owned `dnd.rs`:

```rust,ignore
enum Payload { Snippets(Vec<SnippetId>), Group(GroupId) }
enum Destination { Root, Group(GroupId) }
enum Acceptance { Accept, Reject }
struct Drop { payload: Payload, destination: Destination }
impl Dnd {
    fn begin_layout(&mut self, generation: u64);
    fn register(&mut self, generation: u64, target: Destination, rect: [f32; 4]);
    fn unregister(&mut self, target: Destination);
    fn begin(&mut self, payload: Payload, point: [f32; 2]);
    fn update(&mut self, point: [f32; 2], accepts: impl Fn(&Payload, &Destination) -> Acceptance);
    fn finish(&mut self, point: [f32; 2], cancelled: bool) -> Option<Drop>;
    fn cancel(&mut self);
}
```

Registration generation/unregister must remove stale or folded targets. Use drawn
window coordinates, intersect with viewport clips, deterministic deepest-target
precedence, and re-hit-test/revalidate on finish. A model/service validates group
self/descendant cycles and performs the move once. No domain mutation on Down. Snapshot
selected ids on Down, apply a 4 pt drag threshold, show name/count ghost as an inert
root overlay. Move uses Live bindings and `live_only`; commit only begin/end or a
change in actual view content. Cancellation includes Escape, lost pointer/window
focus and source disappearance. Add edge auto-scroll explicitly if required; the
spike does not implement it. S50 provides sources; S42 owns target lifecycle.

## Upstream comments for the owner

No issues/comments posted, as requested. Accept's “comment on #357” is supplied as
proposed text; upstream publication remains the orchestrator's job.

- Existing [#357](https://github.com/anomalyco/rune/issues/357), proposed comment:
  “Kudzu needs typed snippet/group drops onto sidebar groups, with count ghost and rejection.
  At 29aacb6, on_drag + drawn window rects + Live bindings supports stable target hover; native typed targets should replace this registry, including unmount/cancel/clip handling.”
- Existing [#546](https://github.com/anomalyco/rune/issues/546), proposed comment:
  “S14 exercises wheel-only scrolling separately from keyboard scroll_into_view on 5,000 keyed rows.
  Keep the zero-view-commit wheel budget distinct from commits required by selection/reveal.”
- Existing [#549](https://github.com/anomalyco/rune/issues/549), proposed comment:
  “Kudzu's End key needs to reveal item 4,999 before that row has ever been built.
  scroll_into_view(id) is our tested path; reveal_selected/scroll_key_to must gain equivalent index-row support before we can use them.”
- Existing [#203](https://github.com/anomalyco/rune/issues/203), proposed comment:
  “Kudzu replaces the result slice on filter changes, including same-count replacements with different ids.
  S14 confirms correctness with a bumped version; key enumeration remains O(n), so this is not a proof of constant-time replacement.”
- Existing [#547](https://github.com/anomalyco/rune/issues/547), proposed comment:
  “Kudzu uses explicit scoped row reveals inside a split library window.
  S14 has one vertical viewport and does not exercise sideways ancestor leakage; S41/S50 must recheck nested viewports before adopting the scoped path.”
- [#548](https://github.com/anomalyco/rune/issues/548) concerns text-field links,
  not virtual rows. No list workaround proposed.
- Existing [#551](https://github.com/anomalyco/rune/issues/551), proposed comment:
  “At 29aacb6, dragging 20 pt and releasing inside the source fires on_press as well as on_drag Up.
  S14 reproduced a press count of 2 instead of 1; our fallback suppresses that press and resets suppression on the next Down, while ordinary clicks still select.”

## Integration and evidence limits

The root workspace lock needs the seven-line new local-package entry supplied in
`spikes/lists/workspace-lock.patch`. It is outside S14 ownership, so it is not in
this commit. Gates can pass after Cargo generates this entry; a clean checkout's
`cargo metadata --locked` cannot pass until that entry is generated or integrated. This is a scope
conflict, not a new third-party dependency. No source from earlier Rune ports was
read or reused. Production type-to-filter/search ranking and clipboard actions are
S50/S58 work; replacement here exercises the framework invalidation path only.

## Decisions per question

| Question | Decision | Evidence / boundary |
|---|---|---|
| 5,000 sectioned, 64 pt memoised keyed rows | **go** | Initial mount builds fewer than 100 rows; every header is 30 pt and every item is 64 pt. Clip scene uses 58 pt. Header stickiness for an entire section is not established or required by the current reference. |
| Shift and command multi-select | **go** | Physical modifier input plus real pointer clicks test command toggle and Shift range; repeated Shift-Up preserves the anchor and advances the cursor. |
| Keyboard reveal of unbuilt rows | **fallback** | Real Tab → End reveals item 4,999, followed by repeated Shift-Up. Use explicit `scroll_into_view(id)`; #549 alternatives remain unsuitable. The keyboard selection commits are outside the wheel-only zero-commit budget (#546). |
| Slice replacement / filtering | **go** | 5,000 → 100, same-count replacement with different ids plus version bump, reordered results, id-based click handler, then empty results all asserted in Rune Runtime. Search focus/ranking belongs to S50/S58. #203's O(n) key scan on replacement still applies. |
| Drag selected rows to sidebar | **fallback** | Selected ids 1–3 drop together; accepted/rejected highlight, ghost text `3 selected`, 20 live-only hover moves, Escape cancellation and ordinary click all asserted. #551 reproduces without suppression (red) and passes with it (green); source rows call the small workaround. |
| Scroll budget | **go** for this spike | Full release Rune headless traces, 7 runs × 600 scrolling frames per scene. Exactly one initial view in every process, no wheel view commits. See measurements below. This is a representative two-text-line load, not certification of the future full S50/S58 rows or native display latency. |

Keyboard End initially failed with an empty selection before the list was a focus
stop; repeated Shift-Up initially stopped at two ids before the cursor was stored
separately from the anchor. Those failing assertions were retained through repair.
`target/s14-inside-drag.txt` and `target/s14-red-range.txt` preserve red output locally.

## Acceptance and gate

- Trace numbers: **PASS**, release, repeated medians/p95 and raw per-frame traces.
- API for `dnd.rs`: **PASS**, typed API and target lifecycle above; executable spike
  demonstrates its core mechanism. Production implementation remains S42's work.
- Comment on #357: **PASS under the task's override**, proposed two-line comment
  supplied above. **Not posted**, per the instruction to report rather than file.
- `cargo fmt --all --check`: **PASS**.
- `cargo clippy --workspace --all-targets -- -D warnings`: **PASS**.
- `cargo test --workspace`: **PASS**, 461 tests including the spike's two tests.
- `cargo xtask check-deps`: **PASS with Cargo-generated local-package lock entry**;
  raw `xtask check-deps` / `cargo metadata --locked` on a clean lock is **FAIL**
  until that entry is generated or integrated. `cargo xtask` can generate it first.
  The entry is intentionally excluded from this scope-limited commit.
- Pinned Rune `scripts/app-audit --strict spikes/lists`: **PASS**, no findings.

Owner action: integrate the local-package lock entry. Adopt the #357 registry and
#551 press suppression for S42, then recheck both against every Rune bump. If whole
section sticky headers are desired, decide their composition and remeasure it.
No architecture switch or new third-party dependency is needed for this spike.

## Release trace measurements

`quality.cpu_ms` from Rune's full headless renderer, including frame update and
encode/submit, excluding GPU initialization and readback. Samples cover t=1,000
through <6,000 ms (600 frames per run); each process runs 840 frames total.
`RUNE_COMMIT_BUDGET_MS=0.000001` prints exactly one initial commit in every log;
the view counter independently reports exactly one view. Thus every run has
**zero commits during wheel scrolling**. The trace is simulated time, not real
trackpad/display scheduling. Raw JSONL/logs remain in `target/s14` (41 MB), while
`spikes/lists/evidence.json` preserves all run summaries in the commit.

| Run | Library median ms | Library p95 ms | Clipboard median ms | Clipboard p95 ms |
|---|---:|---:|---:|---:|
| 1 | 0.1489 | 0.2690 | 0.1706 | 0.3015 |
| 2 | 0.1478 | 0.2838 | 0.1665 | 0.3029 |
| 3 | 0.1486 | 0.2652 | 0.1689 | 0.3037 |
| 4 | 0.1491 | 0.2621 | 0.1723 | 0.3096 |
| 5 | 0.1502 | 0.2698 | 0.1758 | 0.2961 |
| 6 | 0.1455 | 0.2739 | 0.1667 | 0.2983 |
| 7 | 0.1478 | 0.2722 | 0.1675 | 0.2870 |

library: median of medians **0.1486 ms**, median of p95s **0.2698 ms**; worst run p95 **0.2838 ms**, against **4 ms p95**.

clips: median of medians **0.1689 ms**, median of p95s **0.3015 ms**; worst run p95 **0.3096 ms**, against **4 ms p95**.

The independent release Runtime-only trace also reports zero view commits in
all seven runs (median 0.0592–0.0698 ms; p95 0.1905–0.2050 ms). These smaller
numbers exclude renderer encode; the full renderer numbers above decide the budget.
The earlier debug timings are not used for the performance verdict.
