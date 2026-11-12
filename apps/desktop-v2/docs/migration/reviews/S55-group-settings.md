# S55 review: group settings and app picker

Independent review by Claude Opus 5.5 of `refactor(desktop-v2): add group settings and the app picker` (`4cc36a1d`). The orchestrator checked B1 (`main/group_settings.rs:466-473`, `:517-529` with `IoFailure` comparing only `kind`) and M2 (`:500-504`, `insertion: None`) before acting.


**Verdict: fixes needed.** 1 blocker, 8 majors, 10 minors.

Paths: Rust files are under `apps/desktop-v2/crates/kudzu-ui/src/` unless they carry a crate prefix. Tauri files are under `apps/desktop/`.

## What was run
- `cargo test -j 2 -p kudzu-ui main::tests::group_settings`: 14 of 14 pass. `cargo test -j 2 -p kudzu-ui widgets::app`: 5 of 5 pass.
- With `--nocapture`, Rune prints: `duplicate sibling key "app-picker-scrim" under "kit-root/group-settings-layer/group-settings-dialog" … the later one is renamed "app-picker-scrim#dupN"`.
- Headless runs used the `check-headless.py` sandbox profile, writing only to a scratch folder. `--list-scenes --window main` has no `group-dialog`. `--tree` and `--snap` for `main` exit 0 in light and dark, and the default scene has no dialog or picker nodes.
- **Headless cannot reach the dialog.** There is no scene, and nothing in production code sets `frame.group_settings_id`; only the tests do (`main/tests/group_settings.rs:61`, `:346`). B1 and every open-picker finding below therefore come from reading code and the tests, not from a reproduction.

## Blocker

**B1. A persistent write failure never shows an error.**
- **Code:**
  - `main/group_settings.rs:470-473` records `error_at_submit` from `saved()`.
  - `main/group_settings.rs:517-529` hides any error equal to it.
  - `PersistError::Io(IoFailure)` compares only the `io::ErrorKind` (`kudzu-app/src/error.rs:5-9`).
  - Every failed attempt publishes the same `SaveState { error }` (`kudzu-app/src/persist/writer.rs:79-88`).
- **Tauri:** `components/app/main/group-settings.tsx:41-42` closes without waiting. FEATURES §7 and D5 are the reason S55 must show the error.
- **Failure:**
  1. The library folder turns read-only, or the disk fills.
  2. Any earlier library write (an editor change, an import, a sync merge) has already failed, so `saved()` is `{durable: R5, error: Io(PermissionDenied)}`.
  3. The user renames a group and presses Save, which creates R7 with `error_at_submit = Some(Io(PermissionDenied))`.
  4. Every retry (`src/compose.rs:73`, 5 s) fails with an equal value, so nothing ever shows.
  5. The dialog stays open with no message, and Save is inert (M1).

  Once a write has failed, the next one usually fails too, so this is the common failure path, not an edge case. The only failure test makes a healthy writer fail (`main/tests/group_settings.rs:320-339`).
- **Fix:** store `s.svc.library.saved().revision()` at submit and treat an error in any later `SaveState` revision as this save's failure. Simpler: if `saved.error` is already `Some` at submit, show it at once. Add a test that starts with the writer already failing.

## Major

**M1. Save does nothing while a save is pending, fields stay editable, and a later close drops newer edits. Cancel does not undo.**
- **Code:**
  - The guard at `main/group_settings.rs:456-458` returns early whenever `pending` is set, including after an error.
  - `main/group_settings.rs:513-515` dismisses as soon as R7 is durable.
  - `dismiss` (`:447-453`) forgets the pending save.
  - `transact` commits in memory before the write (`kudzu-app/src/library/mod.rs:105-133`).
- **Failure:**
  - With a 250 ms debounce (`src/compose.rs:72`), or during error retries, the user edits again and presses Save. Nothing happens. When R7 becomes durable, the dialog closes and the later edits are gone.
  - After an error, Cancel closes the dialog. The change stays in memory and is written when the disk recovers, and nothing in this step reports a later failure.
- **Fix:** while pending, disable the form and show a saving state. Alternatively, let a second Save submit the current draft and wait for the newer revision. After an error, say the change is kept and retrying, or offer a revert.

**M2. Every Save erases the rule's per-app insertion override.**
- **Code:** `main/group_settings.rs:500-504` builds the rule with `insertion: None`. The test asserts the loss (`main/tests/group_settings.rs:306`).
- **Tauri:**
  - `AppRule` has `insertion?` (`lib/types.ts:8-12`), and `Group.rule?: AppRule` (`:35-43`).
  - The dialog saves the whole draft (`group-settings.tsx:26`, `:41`).
  - The picker keeps the rest of the rule (`{ ...draft.rule!, bundleIDs }`, `:88`).
  - A fresh rule is built only when the mode segment changes (`:77`).
- **Core:** `kudzu-core/src/matching/index.rs:93-101` uses the inherited rule's `insertion` as the top-precedence override (RUL-4).
- **Failure:**
  1. A group comes in through JSON import, an agent edit, or sync with "Only in Terminal, insert by Type".
  2. The user only renames the group.
  3. The override is gone, and Terminal falls back to its paste default.
- **Fix:** keep the loaded `AppRule` in the draft and change only `mode` and `bundle_ids`. Reset `insertion` only when the mode changes, as Tauri does. Turn the test into a preservation test.

**M3. Return in the name field does not save.**
- **Code:** `main/group_settings.rs:225-245` sets no `on_submit`.
- **Tauri:** `<form onSubmit>` (`group-settings.tsx:36-43`), with the name field autofocused (`:46`).
- **Failure:** the user types a name and presses Return. Nothing happens.
- **Fix:** `.on_submit(|st, _| save(st))`; rune-kit has `TextInput::on_submit` (`rune-kit/src/text_input.rs:289`). Optionally add `El::default_button()` on Save (`rune/src/el/focus.rs:177`) for Return when no field is focused.

**M4. The app list cannot scroll, is not fitted to the window, and overhangs the panel.**
- **Code:**
  - `widgets/app_picker.rs:219-229` puts `max_h(288)` and `clip_rounded` on the one column that holds the search field and every row.
  - There is no scroll viewport.
  - Children don't shrink unless they opt in (`rune/src/el/layout.rs:534`), so rows past 288 pt are clipped.
  - The list is placed by a raw `absolute` offset (`:221-224`), so nothing flips or clamps it.
- **Tauri:** only the list under the input is capped, and it scrolls: `CommandList` is `max-h-72 … overflow-y-auto` (`components/ui/command.tsx:99`; `app-picker.tsx:73-74`).
- **Failure:**
  - With a normal number of installed apps, the rows below the cut are visible to no one, and the only way to reach them is to type.
  - By the fixed heights in `view` and `footer` (`main/group_settings.rs:160-161`, `:368-395`), the list starts about 85 pt above the panel's bottom edge. Most of its 288 pt therefore overhangs the panel and can run past a short window.
- **Fix:**
  - **Sure fix:** wrap the rows (not the search field) in a scroll viewport capped at 288, with `min_h(0)`.
  - **For placement, try** `El::anchored_fit` with `popover::fit_scroll`, or rune-kit `PopupList` (PLAN §1.4 maps `command` to `PopupList` + `TextInput`). Both fit to the window and are layout-time anchored. Neither is proven under a `.modal` panel: if the list goes inert there, file the gap and tag it.

**M5. The picker search is not focused when the list opens, and it has no arrow or Enter handling.**
- **Code:**
  - `widgets/app_picker.rs:188` only flips `open`.
  - The search field's `on_key` handles only Escape (`:238-245`).
  - Rows have only `on_press` (`:283-290`).
  - The tests click into the search field before typing (`main/tests/group_settings.rs:382`, `:408`).
- **Tauri:** `<Command loop>`, `CommandInput`, and `CommandItem onSelect` (`app-picker.tsx:72-73`, `:86`). The exact cmdk and Radix behaviour (arrow and Enter selection, focus on open) goes to the S71 checks.
- **Failure:** the user presses Add app, types "mail" and presses Enter, and nothing is selected. Whether Tab reaches the rows depends on the macOS keyboard-navigation setting (`rune/src/runtime.rs:958-959`).
- **Fix:** focus the query when the list opens. Keep an active index driven from the search field's `on_key` (↑ and ↓ with wrap-around, Enter toggles the active row, Home and End). `PopupList`'s `active` and `on_choose` give this.

**M6. "Add “id”" toggles, so pressing it twice removes the id it says it adds.**
- **Code:** `widgets/app_picker.rs:258-264` calls `toggle` and then `set_open(true)`, and never clears the query. The custom row stays up while nothing matches (`:217`, `:253-254`).
- **Tauri:** `toggle(query), setQuery("")` (`app-picker.tsx:77`).
- **Test gap:** the test clears the query by hand afterwards (`main/tests/group_settings.rs:446-447`), which hides this.
- **Fix:**
  - clear the query through the `edit` accessor after adding;
  - make the custom row add-only;
  - assert the cleared query in the test.

**M7. Icons are drawn from 16 and 18 pixel bitmaps, so they are blurry on Retina.**
- **Code:**
  - `CHIP_ICON = 16` and `ROW_ICON = 18` (`widgets/app_picker.rs:14-15`) are passed as `size` to `Apps::icon` (`widgets/app_icon.rs:53`).
  - The catalogue draws the icon at "exactly `size`×`size` pixels" (`kudzu-system/src/apps.rs:177-178`).
  - The clipboard asks for 64 px file icons (`kudzu-app/src/clipboard/media.rs:47`).
- **Tauri:** `app_icon_png(&path, 64)` (`src-tauri/src/commands.rs:443-444`).
- **Fix:** request one pixel size (64, or points × backing scale) for both chips and rows. That also halves the catalogue reads.

**M8. Reuse: fixed keys and unscrolled layout reads will break S51 and S61.**
- **Code:**
  - The keys `app-picker`, `app-picker-add` and `app-picker-list` are fixed (`widgets/app_picker.rs:141`, `:190`, `:220`), and the list is placed from `cx.layout` of them (`:193-194`).
  - Rune resolves a bare key to the first matching node in tree order (`rune/src/runtime/layout_read.rs:325-330`, through `key_index.rs:104-114`, `:137-140`).
  - `Cx::layout` ignores scrolling (`layout_read.rs:169-197`, the doc comment).
- **Failures:**
  - **(a)** Settings → Apps shows two pickers on one pane (`components/app/main/settings.tsx:610` and `:647`). Opening "Add an app" places its list under "Exclude an app".
  - **(b)** The group dialog opened over an editor whose snippet has a rule (`components/app/editor/behavior.tsx:162`) reads the editor's Add button.
  - **(c)** In a scrolled Settings or editor pane, the list is off by the scroll distance.
- **Fix:**
  - Take an id in `AppPicker::new` and derive every key from it.
  - Anchor at layout time as in M4, or read `cx.window_layout`.
  - Add an `add_label(…)` builder: the field is `pub` but has no setter, and all three Tauri call sites set it.
  - Move "Escape closes the list" into the picker. Today only `group_settings` and `main/keys.rs:34-37` handle it, and only while the dialog is open.

## Minor
- **m1. Duplicate sibling key.** Both scrim pieces use the key `app-picker-scrim` (`widgets/app_picker.rs:334`, `:342`). The Rune warning above confirms it. Fix: `-top` and `-bottom`.
- **m2. Stale first frame, and no `open` function.**
  - The draft loads in `sync` (`main/group_settings.rs:107-110`).
  - Rune runs `poll` before queued input (`rune/src/runtime/frame.rs:438-465`).
  - So an opener that sets `group_settings_id` from a press handler (the S42 context menu, the S43 follow-up "Group settings: path"; see `docs/migration/reviews/S43-command-menu.md:15`, `:21`) commits one view with the previous group's name, rule and chips, and no field focus.
  - Fix: add `pub fn open(s, id)` that calls `load_draft` immediately.
- **m3. One thread per icon.**
  - `Task::spawn` is a thread per call (`kudzu-app/src/tasks.rs:26-47`; `kudzu-app/src/apps/mod.rs:59-84`).
  - An open list with an empty query requests every installed app at once (`main/group_settings.rs:570-582`, `widgets/app_picker.rs:81-85`).
  - The work is off the UI thread, but it starts hundreds of threads at once. Use a bounded worker, or request only the rendered rows.
  - The caches grow only with the number of installed apps, sizes and added custom ids. They are never evicted, which is acceptable at that size.
- **m4. Workarounds not tagged.** None of the three Rune workarounds carries a `RUNE-GAP(#n)` header (PLAN rule 6), and PLAN §3 lists no Escape or popup-in-modal gap.
- **m5. No gallery scene or matrix row.** `app_icon` and `app_picker` have neither (`docs/migration/PARITY_MATRIX.md:88` says each composition has one; PLAN §1.4 lists both). The open picker is covered by no landmark test or reference.
- **m6. Chip and button visuals differ from Tauri.**
  - Chips have no `shadow-raised` and no `max-w-40 truncate` (`app-picker.tsx:47-49` vs `widgets/app_picker.rs:151-178`).
  - The Remove button is 24 pt; Tauri's is `size-5`, 20 pt.
  - Add app lacks the muted text and the dashed ring (`app-picker.tsx:67` vs `:183-190`).
  - Rows have no hover or active highlight, and long names or ids overflow the 288 pt list.
- **m7. Accessibility.** Rows set no role, label or checked state (`widgets/app_picker.rs:283-304`), so VoiceOver can't tell which apps are chosen. The error text (`main/group_settings.rs:133-140`) is not announced.
- **m8. Presses with the list open.** The scrim pieces sit above the Save, Cancel, switch and segment controls (`main/group_settings.rs:166-168`), so the first press there only closes the list. Where the list overhangs the panel (M4), there is no picker scrim. A press beside the list hits nothing inside the modal, and Rune dismisses the whole dialog (`rune/src/runtime/input/pointer.rs:197-203`), discarding the draft. Compare with Tauri in S71.
- **m9. Save overwrites concurrent sync edits.** Save writes `is_enabled`, `insertion` and `rule` from the draft even when untouched (`main/group_settings.rs:497-504`), so a sync edit made while the dialog is open is reverted. Tauri does the same. `parent_id` and `sort_index` are taken fresh, which is better than Tauri.
- **m10. Nits.** The `monogram` doc says "or the bundle id" (`widgets/app_icon.rs:108`), but the function returns only the first letter.

## What holds
- **Landmarks.** The 13 dialog landmarks in the test equal the S02 `metrics.json` rows for `group-dialog`, in both themes. The test checks them within 2 pt, together with the popover fill, the 18 pt radius and the `muted/50` footer. These checks are driven by the reference, not copied from the code.
- **Copy and order match Tauri:**
  - the title and description;
  - "Insert by" and its hint;
  - "Apps" and its hint;
  - the segment labels, including the ellipses;
  - On and Off;
  - Add app;
  - the search placeholder;
  - "No apps found.";
  - "Add “id”";
  - "Remove {name}".
- **Name.** Trimming and the blank fallback live in `update_group` (`kudzu-app/src/library/groups.rs:75-79`).
- **Inherited versus own.** Default stores `insertion: None` and All apps stores `rule: None`, so both inherit. An explicit `Automatic` survives a save: it is loaded at `:553` and written at `:499`, and no segment is checked, as in Tauri.
- **Rule mode switches.**
  - Allow ↔ Deny keeps the chosen ids.
  - All apps clears them. Tauri behaves the same, because coming back from All apps rebuilds the rule with `[]`.
- **Save behaviour.**
  - A durable write closes the dialog.
  - A synchronous `MissingGroup` shows "Group not found" (tested).
  - A double press is guarded.
  - A group deleted by sync while the dialog is open fails the same way.
- **Bundle-id rule.** It is the ASCII `[\w-]+(\.[\w-]+)+`, applied to the trimmed query.
- **Picker behaviour.**
  - Search covers names and bundle ids, case-insensitively.
  - A custom id is offered only when nothing matches.
  - Checking an app keeps the list open.
  - A missing app shows its bundle id and a "C" monogram (tested).
- **Icons.**
  - Catalogue reads run on workers and decoding on Rune's image workers.
  - There is one read per (id, size); a test proves the request returns before the catalogue answers.
- **Escape and key guards.**
  - Escape in the name field closes the dialog.
  - Escape in the search field closes only the list.
  - The `keys.rs` guards and the composing, editing and blur hooks are wired.
  - ⌘N while typing creates no snippet (tested).
- **Placement inside the dialog.** A view that reads a rect re-runs in the same commit when that rect moves (`layout_read.rs:183-187`). So the first open, a chip wrap and a resize settle inside the dialog. The problem is reuse (M8).
- **Split scrim inside the panel.** The two pieces cover the panel above and below the chip row, the list draws above them, and clipped rows can't be hit (`rune/src/el/visuals.rs:451`). The exception is the overhang (m8).
- **Reuse boundary.** `app_picker` and `app_icon` contain no group-specific code and are generic over the state type.
- **Safety.**
  - No `#[allow]`.
  - No new dependencies.
  - No logging.
  - `unwrap` only in tests.
  - Nothing blocks the UI thread.
  - Error strings are fixed copy with no user text.
- **Size.** `main/group_settings.rs` is 586 lines.
- **The implementer's shared edits are as described:**
  - `MainServices.apps`;
  - the state and its wake;
  - the `lifecycle.rs` sync line;
  - the `frame.rs` call;
  - the `keys.rs` guards.

## Live checks for S71
Blocked until S42 or the S43 follow-up opens the dialog.
1. Open the dialog from the sidebar menu and from ⌘K, group A then group B. The first frame shows B's values and the caret is in the name field.
2. Rename and press Return to save (after M3). Make the library folder read-only and press Save: the error appears. Repeat with a write already failing before the dialog opens (B1). Restore the folder: the dialog closes by itself, and no edit made after Save is lost (M1).
3. Picker keyboard, side by side with Tauri:
   - whether the query is focused on open;
   - ↑ and ↓ with wrap-around, Enter, Home and End;
   - Tab, with macOS keyboard navigation off and on;
   - Escape closes the list first, then the dialog.
4. With 100+ apps installed, scroll to the last app. Check that the list stays inside a 900×560 window, and that it follows the Add button when chips wrap and when the window is resized.
5. Icons are sharp on Retina next to Tauri, and an uninstalled custom id shows a monogram.
6. With the list open, press Save, the switch or a segment: compare one press against two with Tauri. Also press beside an overhanging list and outside the panel: which layers close?
7. Compare search results and their order with Tauri for "vsc", "chr" and part of a bundle id.
8. A group with a `rule.insertion` from JSON keeps it after a rename, and expansion in that app still uses it (after M2).
9. Rename or delete the open group from another Mac. Save then keeps the draft name, or shows "Group not found".
10. VoiceOver: the dialog's name and description, and each row's name and checked state.
