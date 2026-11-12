# S64 review: Insights

This is an independent review by Codex Sol of `refactor(desktop-v2): add Insights` and the follow-up `refactor(desktop-v2): expose the clock port on Kudzu`. The orchestrator checked M1 against `insights/mod.rs` and m6 against `kudzu-core/src/fmt.rs` and `lib/stats.ts` before acting on them.

**Verdict:** fixes needed. There are no blockers. All 13 Insights tests pass.

## Majors

**M1. The headline tiles go stale after the arrival animation** (`main/insights/mod.rs:118`, `view.rs:121`; Tauri `insights.tsx:38`, `:66`).
- **Problem:** once the visit has landed, the tile `LiveText`s re-snap only when WPM changes. A new expansion, a synced usage row, or a change to which snippets are enabled updates the lists and subtitles, but not the four headline numbers. Tauri shows the current value once its animation has finished.
- **Fix:** refresh all four values on every relevant data change after landing.
- **Test:** finish arrival, then change usage and enablement without touching WPM. Check the totals on screen right away.

**M2. Rows miss Tauri's border box, and the layout test misses vertical drift** (`view.rs:225`, `:311`; `tests/insights.rs:317`; Tauri `insights.tsx:107`, `:144`, `components/ui/item.tsx:37`).
- **Problem:** Tauri's `Item` has a transparent 1 pt border, so a Most used row is 42 pt where ours is 40 pt. The difference adds up over ten rows. The test checks only x positions and widths.
- **Fix:** match the row's full box.
- **Test:** assert every content landmark's full rect, in both themes, including the lower rows and Never used.

**M3. No Insights motion gate, and the timing assertions are loose** (`tests/insights.rs:260`, `:272`, `:286`; Tauri `insights.tsx:28`, `:41`, `:116`; `globals.css:121`).
- **Problem:** the arrival test checks one frame partway through, then runs 80 more. It never pins:
  - the 560 ms end of the count-up;
  - the 480 ms bar duration;
  - each `60 + i×40` ms delay;
  - the proportional bar widths.

  A much slower count-up would still pass. `tests/motion_widgets.rs:94` gates only the selection pill and the shake.
- **Fix:** add Insights motion-gate recordings, with exact virtual-clock assertions for:
  - the count-up;
  - every bar's delay and duration;
  - Reduce Motion;
  - later changes being instant.

## Minors

1. **(m4) The calendar fallback always uses UTC** (`insights/data.rs:17`, `:131`; Tauri `lib/stats.ts:77`). A usage more than 30 days old, near midnight UTC, shows the wrong day outside UTC.
   - **Fix:** take the local offset from the host's time zone, which can be injected; fixtures stay in UTC.
   - **Test:** both sides of midnight, and the exact 30-day boundary.
2. **(m5) The Never used count shows the capped number** (`data.rs:107`, `view.rs:368`; Tauri `insights.tsx:158`, `:161`). With 25 eligible snippets, Tauri shows "Never used 25" with 18 chips; ours shows 18.
   - **Fix:** keep the total separate from the list of 18.
   - **Test:** more than 18 eligible snippets.
3. **(m6) Rounding differs from JavaScript `toFixed`** (`kudzu-core/src/fmt.rs:173` `to_fixed`, repeated in `insights/count_up.rs:47`; Tauri `lib/stats.ts:56`, `:62`). Rust's `format!` rounds exact ties to even; `toFixed` rounds them up.

   | Value | Ours | Tauri |
   |---|---|---|
   | 1,250 | 1.2k | 1.3k |
   | 12,500 | 12k | 13k |
   | 2.25 h | 2.2 h | 2.3 h |

   - **Fix:** make `fmt::to_fixed` match `toFixed`. This is S24 code, and S64 is what exposed it.
   - **Test:** add the exact halfway cases to the core format tests.

## What holds

- **Aggregation** goes through the core `usage_by_id` and `totals`, and matches Tauri on:
  - device rows and per-row clamping;
  - orphan rows;
  - `chars / 5 / max(1, WPM)`;
  - counting active snippets by each snippet's own flag.
- **Sorting.** Stable sorts keep Tauri's tie-breaks, and the limits are 10, 6 and 18.
- **Screen content.**
  - The tiles, empty states and all three navigation paths work.
  - The slider uses Tauri's range and default, and saves straight away.
  - The 880 pt cap and the 1.4 : 1 columns are in place.
- **Arrival animation.**
  - It uses quartic easing, and Reduce Motion skips it.
  - A return visit replays it, which matches Tauri: `main-app.tsx:194–205` unmounts Insights.
- **Code hygiene.**
  - "Now" comes from `MainServices.clock`.
  - There is no blocking I/O on the UI thread.
  - There are no new dependencies, no logging and no `#[allow]`.

## S71 live checks

- **Live updates and settings.**
  - Real expansions refresh an Insights screen that is already open.
  - WPM survives a restart.
- **Navigation.**
  - Keyboard and VoiceOver reach every control.
  - Clicking a chip or a row selects the right snippet.
- **Side by side with Tauri.** Compare:
  - the arrival animation and return visits;
  - Reduce Motion;
  - scrolling;
  - the layout at the minimum window size.
