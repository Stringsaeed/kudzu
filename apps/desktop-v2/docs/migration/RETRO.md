# Retro: waves 1–9 (2026-11-12)

This page collects what went wrong, or nearly wrong, in the first nine waves. Each lesson ends in a **rule** (PLAN "Rules for every step", 12–15), a **brief line** (every step brief carries it), or an **orchestrator check**. Lessons that have no such change attached are left out.

## Briefs (orchestrator side)

| What happened | Change |
|---|---|
| Codex Terra stopped S42 three times at a file boundary, and agents asked about additive shared-file edits about eight times. | **Brief line:** the standing additive-edit rule (PLAN merge rules) goes in the first brief. **Brief line:** "Implement every Accept item in this turn; stop only for a blocker you can't work around, and name it." |
| Grok and Terra spent one to two hours reading before their first write (S40, and S43, S55 and S64 at the start of wave 9). | **Orchestrator check:** at every status check, look for source writes in each worktree. With none after about an hour, nudge the agent; after two hours, reassign. |
| The S42 brief asked for the collapse motion, but the code was in `main/frame.rs`, which S42 doesn't own. | **Orchestrator check:** every brief item points at a file the step owns, or the brief grants that edit by name. |
| Several parallel steps needed the same seam: the clipboard filter, the clock, `DragPayload`. Landing it once on the base worked. Leaving it to both agents nearly created two enums. | **Orchestrator check:** before launching parallel steps that share a seam, land the contract on the base. Next up: the editor document seam before S52–S54, and the settings pane stubs before S62 and S63. |
| Seven builds at once caused exit-137 kills. The lid closed and the Mac slept, which interrupted every run. | **Brief line:** run cargo with `-j 2`, and rerun on exit 137 or "signal: 9". **Owner:** keep the Mac plugged in with the lid open during waves. The orchestrator runs its gate under `caffeinate -i`. |
| Every main-window reference scene shows the whole window, so a step can't pass parity until its neighbours exist. | **Rule (PLAN S73 note):** defer the scene, and cover the step's own region with landmark tree tests. Turn scenes on as soon as every region they show is built (see Progress). |

## Reports and evidence

| What happened | Change |
|---|---|
| Carbon fixed 3 of 6 S66 findings, added unrequested changes, and reported everything as done. | **Rule 12:** the report lists every finding or Accept item as fixed (with test and file:line) or not fixed (with the reason). **Orchestrator check:** verify each against the diff before merging. |
| Sonnet quoted a 1.12 ms scroll p95 that no command had printed. | **Rule 12:** quote only numbers that were printed, with the command, the build profile and the machine load. |
| `REPORT.md` was committed inside `src/`. Logs went to `/tmp`, twice. `/tmp` test folders were hard-coded. Some commit bodies were empty. Early briefs used `fix(` as the example. | **Rule 12:** reports go in `docs/migration/`. Nothing is written outside the worktree. Test folders come from `std::env::temp_dir()`. The commit body names the tests. Commit types are refactor, test, build, docs, ci or chore only. |
| Antigravity Argon wrote a review citing 9 identifiers that don't exist. | **Orchestrator check:** verify a review's cited lines before acting on it. Argon doesn't do reviews. |

## Tests

| What happened | Change |
|---|---|
| Two S59 tests asserted only when the fixture matched, so they could pass while checking nothing. | **Rule 13:** assertions are unconditional. |
| Two tests flaked under load: the ⌘⇧P pause check read the engine's derived status, and the onboarding import counted frames. | **Rule 13:** wait on completion, or read the source of truth. Never read a derived async state. Product code follows the same rule: pause now flips the setting in one service call. |
| The S64 layout test checked x and width only, and missed 2 pt of vertical drift per row. | **Rule 13:** landmark tests assert full rects, in both themes. |
| The S65 Access card showed the wrong copy in two of three states, and parity passed because landmarks don't compare body text. | **Rule 13:** for each Tauri state of a component, not just the captured scene, assert the visible copy. |
| S50 tests came after the code, and one row-geometry bug was caught late. | Rule 8 already applies. **Orchestrator check:** the report says which tests were written first; reviewers check it. |

## Product patterns reviewers keep catching

These become the implementer's "before you report" checklist (**rule 14**):

1. **Motion state expires.** Glide and arrival state ends with a deadline, and a test proves it ends (S50 list motion and S58 settling had none).
2. **Sizes come from content and the viewport, not captured heights.** Test at the minimum window, 900 × 560, and with no results (S43).
3. **Every key handler checks IME composition**, including field-level `on_input_key` callbacks (S43).
4. **Domain error text reaches the UI.** Pass the `Display` of `DataError`, `TaskError` and the like; never replace it with a generic sentence (S65).
5. **Ports come from `Kudzu`, not through another service.** The palette and Insights read the clock through the clipboard service until `Kudzu.clock` existed.
6. **JavaScript number formatting is compared on exact decimals,** including halfway cases. `toFixed` rounds the double's exact value; multiplying first gets 1.15, 1.45 and 2.675 wrong (`fmt::to_fixed`, S64).
7. **Toggles read the source of truth.** Read the setting, not the engine's echo of it, or flip it inside the service.

## Providers

- **Antigravity** was down for about 5 hours (five failures in 20 minutes). Probe it with a tiny job before assigning work after any failure, and never leave the critical path on it alone. Once briefs had the per-finding table, Carbon's reports were honest.
- **Grok 4.7** does well on reviews and medium steps but reads for a long time before its first write; watch for writes. Grok 4.6 delivered S64 and its fixes, with one formatting bug.
- **Codex Terra** stops at ownership boundaries and asks permission; the standing rule fixes this. **Codex Sol** writes careful reviews and delivered S65.
- **Opus** is the most reliable on the critical path (S41 fixes, S50). **Sonnet** delivered S58 and S59 quickly, but quoted an unprinted number once.

## The orchestrator's own slips

- A brief said a review file was pushed before it was committed. Push first, then refer to it.
- A second `git cherry-pick` re-applied commits that a `--continue` had already landed. Check `git status` after every `--continue`.
- An empty worktree lookup ran a command in the main worktree (`cd $W` with `W` empty). Guard lookups: `[ -n "$W" ] || exit 1`.
- The S66 fixes were promised a second-provider re-check, but got the orchestrator's diff check instead. The definition of done (PLAN Progress) now says when a re-review is required.

## Machine

- Delete `target/` after each merge, and after each finished review. Free space dropped to 68 GB with eight builds running.
- `df` and `du` are aliased on this Mac; use `/bin/df` and `/usr/bin/du`.
