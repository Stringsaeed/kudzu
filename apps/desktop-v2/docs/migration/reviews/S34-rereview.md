# S34 re-review — engine safety fixes

Independent re-review (Codex) of the five S34 safety commits (`df15c36a..ad4fde78`). Verdict: **block**. Fixed by the ten commits from `refactor(desktop-v2): bind explicit insertions to their target before rendering` to `test(desktop-v2): hold the tap-callback budget in an optimized build`; each commit body names the test that was red first.

## Blockers

1. **A target change during rendering could be adopted as the intended target** (`form_flow.rs`, `explicit.rs`). The engine checked the target, rendered (which can read the clipboard and wait), then captured whichever PID was frontmost after rendering. Fix asked: bind `Live` to the resolved target before rendering and revalidate after.
2. **A delayed or missing blur could still forgive typing in the target** (`interruption.rs`). Focus leaves the form without a click, a key reaches the target, the user returns and submits before the blur arrives: that key precedes the submit stamp and is accepted; a blur that never arrives does the same. Fix asked: reconcile focus boundaries through submission and fail closed when the engine can't establish that all reports arrived.
3. **Undo bypassed the re-checks** (`match_flow.rs`). After the first check, undo posted every cursor move, Backspace and retyped character without another check. Fix asked: carry `Live` through undo and check before each press.

## Majors

4. **Suppression could hide cancellation from an in-flight sequence** (`match_flow.rs`, `insert.rs`). While suppressed, clicks didn't invalidate the generation, and sequence steps after a submitted form's deletion dropped the form check.
5. **Single checks didn't cover multi-press output** (`insert.rs`, `keys.rs`): `{{key}}` counts, typed chunks, recovery and separator presses.
6. **D24 and D17 still lost part of an active trigger without notice** (`insert.rs`, `replace.rs`): the withheld completing key on cancellation, cancellation before the first Backspace, a sequence stopped right after deletion.
7. **Equal timestamps and event identity weren't resolved** (`interruption.rs`): strict `>` accepted any key with the submit event's stamp; no way to say the event was unavailable.
8. **A window-server lookup ran inside the tap callback under the engine lock** (`match_flow.rs`, `apps.rs`), unmeasured against the 0.2 ms p99 budget.
9. **Mouse routing fell back to a window that might not receive the click** (`tap.rs`): "can handle" zero with a Kudzu raw under-pointer window was treated as safe.
10. **The synthetic-event marker is spoofable** (`match_flow.rs`, `keys.rs`). Accepted as D25, documented at the marker.

## Live checks added to S71

Click-through Kudzu windows and a closing window at the session tap; click-path callback p99 under window-server load; equal or unavailable event stamps and delayed focus reports; interrupting multi-press undo, recovery and submitted-form sequences.
