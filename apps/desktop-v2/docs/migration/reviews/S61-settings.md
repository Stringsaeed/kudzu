# S61 review: settings frame, General, Expansion and Apps

An independent review by Grok 4.6 of the four S61 commits, `717a410e` to `38466e64`. The orchestrator checked M1 against `main/state.rs:84–94` and Tauri's `settings.tsx:44–52` before acting on it.

**Verdict: fixes needed.** No blockers. `cargo test -p kudzu-ui settings`: 33 passed.

## Majors

**M1. Two nav labels don't match SET-1** (`main/state.rs:90–91`; Tauri `components/app/main/settings.tsx:48–50`).
- **What:** the nav says "Agents" and "Data & Sync". Tauri, the PLAN Accept and the reference landmarks say "AI agents" and "Backups & data". The headings use the same labels, so they're wrong too.
- **Why the tests miss it:** they check only the element keys, and never test wrapping Down from About.
- **Fix:**
  - Use "AI agents" and "Backups & data".
  - Assert all eight labels in order.
  - Test that Down from About goes to General.

**M2. Appearance, Insert by and the per-app methods are button radios, so ↑ and ↓ leave the pane** (`settings/general.rs:82–104`, `settings/expansion.rs:41–76`, `settings/nav.rs:71–86`, `main/keys.rs:76–80`; Tauri `components/app/segmented.tsx:15`, `:48–54`).
- **What:** focus Dark and press Down. The pane switches to Expansion instead of selecting Light.
- **Fix:**
  - Build these groups with `widgets::selection_pill::Segmented`, threading `Cx` through General and Expansion as Apps does.
  - Handle ↑ and ↓ in `nav::handle_key` only while focus is inside `settings-nav`.
  - Test that Down on `settings-theme-dark` stays on General and selects Light.

**M3. Never-expand is a home-made chip row, and opening the picker duplicates it** (`settings/apps.rs:40–63`, `:118–121`, `:132–169`; Tauri `settings.tsx:608–610`, `:647–655`, `app-picker.tsx:47–63`, `:76–78`).
- **What:**
  - Clicking Exclude shows both the home-made chips and the `AppPicker` chips, plus a second Exclude button.
  - Add puts a second Add under the insertion rows.
  - Exclude doesn't clear `query`.
- **Why the tests miss it:** they call `choose()` directly, so they never drive the picker or typed bundle ids.
- **Fix:**
  - Use one `AppPicker` for the never-expand list and one for Add, with two ids once S55's M8 keys land.
  - Reset `query` on Exclude.
  - Test clicking, typing a bundle id, Escape, and removing a chip.

**M4. The shortcut recorder can't record punctuation, and no test presses a key in it** (`settings/general.rs:133–162`; `kudzu-core/src/keys.rs:119–144`, `:247–259`; Tauri `lib/keys.ts:67–79`; FEATURES SET-3).
- **What:** `record_key` maps only letters and digits from `Key::Text`.
  - Ctrl-⌥-; and `,` `.` `/` `[` `-` commit nothing.
  - On Dvorak, letter chords store the typed character, not the physical key.
- **Fix:**
  - Map punctuation to the named codes `is_named_code` already lists, and use the physical key where Rune can provide it.
  - Test pressing chords: a letter, `Semicolon`, a bare F-key, and a lone modifier, which must be rejected.

**M5. The Pause test proves nothing** (`tests/settings.rs:62–65`; `settings/expansion.rs:27`).
- **What:** it clicks twice and asserts `paused == paused`. The Dev rig starts paused, so a click that does nothing still passes.
- **Fix:** click once and assert `paused == false`. Optionally check the fake engine as well.

## Minors

1. **Plain text where Tauri uses keycaps.** The recorder's idle text and the Pause and Quick Search hints are plain text. Tauri uses `<Shortcut spoken />` and keycaps (`settings.tsx:115`, `:197`, `:258`). Use `Shortcut::accelerator` and `::spoken`.
2. **No input limit on the prefix field.** It is clamped only after composition and by the service. Tauri sets `maxLength={4}` (`settings.tsx:269`).
3. **Permissions shows no "Opening…" and no open error.** See `permission-setup.tsx:18–44`. This is a SET-2 gap; S61's Accept doesn't name it.
4. **The collision and taken tests never assert the heading "That shortcut is taken".**
5. **The App group landmark is untested.** The reference is `[486,616,616,206]`.
6. **`frame.rs` builds the settings view twice** (`main/frame.rs:140–149`).
7. **Exclude doesn't reset `query`.**
8. **App names sort with `to_lowercase()`.** Tauri uses `localeCompare` (`settings.tsx:602`).
9. **The Good-to-know titles aren't emphasised** (`font-medium text-foreground`, `settings.tsx:289–296`).
10. **"Shortcut updated" is inline text.** Tauri uses `toast.success` (`settings.tsx:207`).

## What holds

- **Frame.** Eight panes in reference order, a 192 pt nav, a 48 pt heading and a body capped at 680. Region rects are within 2 pt, including at 900 × 560, in both themes.
- **Permissions.** Set up and Review, with the three status copies.
- **Shortcut recorder.**
  - "Press shortcut…", and Escape keeps the current shortcut.
  - The collision and taken copy, and rollback through `HotkeySlots::set(…, shell.registrar())`.
  - `hotkey_error` is shown, and the capture motion expires.
- **General controls.**
  - The iCloud switch, with a clean `sync_status` seam for S63.
  - Appearance.
  - The login item, with "login item unavailable".
  - The Dock, through `set_dock_icon`.
- **Expansion controls.**
  - Pause goes through `toggle_paused`.
  - The prefix is capped at 4 UTF-16 units.
  - Insert by is wired to the engine config.
- **Apps.** Rows sort by name, Add defaults to Paste, and remove works.
- **Pane stubs.** The five stubs are clean `view` seams for S62 and S63.
- **Hygiene.**
  - IME is checked in the nav, the recorder and the prefix field.
  - Waits use watches.
  - Ports come from `Kudzu`.
  - No `#[allow]`, no logging and no dependency change. Every file is under 600 lines.

## S71 live checks

1. **Native effects:** the Dock icon and `NSApp` appearance after changing the General switches.
2. **Recorder:** Ctrl-⌥-; with `,` and `/`, a bare F-key, and a Dvorak letter. Space focuses the recorder.
3. **IME:** in the prefix field, and while the recorder is armed.
4. **Apps picker:**
   - Exclude and Add placement after scrolling.
   - A typed bundle id, and the copy for an invalid one.
   - Escape during composition.
   - Removing a chip.
5. **Arrow keys and labels:** arrows on Appearance, Insert by and a per-app method, against pane cycling. Check the labels "AI agents" and "Backups & data".
6. **Spoken shortcuts:** the recorder, the Pause hint and the Quick Search hint.
7. **Permissions:** "Opening…" and a failed open, and Review against Set up when access is already granted.
8. **Pause on a Dev build:** one click resumes it.
9. **Login item:** the failure copy where the port is really unavailable.
