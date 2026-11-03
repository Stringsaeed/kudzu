# Kudzu — Plan

Decisions settled during planning (Nov 2026). Each section records what we chose and, where it wasn't obvious, why.

## Product

- **What:** a lightweight, full-featured text expander for macOS. Espanso-level power with a native Mac UI.
- **Audience:** public, free, open source. **License: MIT.**
- **Problems we fix in Espanso:**
  1. No visual editor (YAML editing)
  2. Search bar looks rough and isn't native
  3. Unreliable expansion in some apps (Electron, terminals, browsers): garbled or slow output
  4. Clunky forms
- **iOS:** later. iOS has no system-wide keystroke monitoring, so the iOS version will be an app plus a custom keyboard extension reading the same synced data. macOS work must keep that path open (shared core package, App Group-ready store).

## Identity

| | |
|---|---|
| Name | **Kudzu** |
| Bundle ID | `sh.saeed.kudzu` |
| iCloud container | `iCloud.sh.saeed.kudzu` |
| Repo | `github.com/Stringsaeed/kudzu` |

Name check: Sigil, Blurt, Steno, Voila and Tilde were rejected for conflicts with existing Mac apps or open-source projects. Kudzu had only minor, unrelated conflicts. A formal trademark search (USPTO/EUIPO) is still to do before the 1.0 release.

## Platform & stack

- Native **Swift + SwiftUI + AppKit**. **Minimum macOS 15 Sequoia**, chosen for SwiftData maturity and history tracking.
- **App shape:** menu-bar app (no Dock icon unless the main window is open), Raycast-style.
- **Project:**
  - `KudzuCore` local Swift package: engine, template parsing, models, import/export. No UI, fully unit-tested, reusable on iOS.
  - Thin macOS app target for UI, panels, permissions and the event tap.
  - **XcodeGen** for project generation, so contributors don't hit `.pbxproj` merge conflicts.
  - **GitHub Actions** for tests, signed and notarized releases, and Homebrew cask bumps.

## Distribution

All channels, from one codebase with two build configurations:

| | Direct (primary) | Mac App Store (best-effort) |
|---|---|---|
| Signing | Developer ID, notarized DMG | App Store, sandboxed |
| Updates | Sparkle | App Store |
| Homebrew | Own tap first, official `homebrew/cask` once notable | — |
| Espanso import | Reads `~/Library/Application Support/espanso` directly | Via an `NSOpenPanel` user-selected folder |
| Future shell/JS snippets | Full | Limited (Application Scripts folder only) |
| iCloud | Same container | Same container, so both builds sync together |

Known risk: App Review scrutinizes apps that monitor keystrokes. M0 must prove that event tap and event posting work inside the sandbox before we rely on that channel. An Apple Developer Program membership is available.

## Data & sync

- **SwiftData + CloudKit** automatic sync, **on by default**, user can turn it off.
- **Two model configurations:**
  - **Synced:** snippets, groups, app rules, global settings, per-device usage records.
  - **Local-only:** hotkeys, permission state, per-Mac preferences.
- **CloudKit modelling rules:**
  - All attributes are optional or have defaults, all relationships are optional, and there are no `#Unique` constraints.
  - **Duplicate detection** is done in the app: identical triggers created on two devices, or the same import run twice, are detected and merged.
  - **Usage stats are per-device records** that are summed for display. A single shared counter would lose increments under last-writer-wins.
  - The schema is **additive-only** after production deploy. Get the model right in M1, and never rename or remove fields.
- **Sync status UI** surfaced through the underlying Core Data/CloudKit event notifications.
- **Open-source builds** made without our signing are local-only, because the container is tied to our team.
- **Engine never queries the store per keystroke.** An in-memory trigger index is rebuilt on change notifications.
- **Backups:** daily local JSON snapshot plus one before every import. Keep the last 14, with one-click restore.

## Expansion engine

- **Detection:** a global `CGEventTap` keeps a rolling buffer and matches it against the in-memory trigger index.
- **Triggers:**
  - Any text is allowed. New snippets suggest a `;` prefix.
  - **Multiple triggers per snippet.**
  - Case-insensitive by default, with **case propagation** (`;Sig` → "Best regards", `;SIG` → "BEST REGARDS").
  - **Collision warnings** in the editor, e.g. `;s` shadowing `;sig`.
- **Firing:**
  - Default is **immediate**, as soon as the trigger completes.
  - Per-snippet **whole-word** mode fires on a separator (space, punctuation, return), won't fire mid-word, and keeps the separator.
  - Imported Espanso snippets keep their word setting.
- **Undo:** backspace immediately after an expansion reverts it to the trigger.
- **Insertion:**
  1. Delete the trigger with synthesized backspaces.
  2. For short plain text (< ~100 chars), synthesize keystrokes (`CGEventKeyboardSetUnicodeString`).
  3. For long or rich text, save the pasteboard, write the content marked transient/concealed (`org.nspasteboard.TransientType` / `ConcealedType`), press ⌘V, then restore the pasteboard.
  4. A per-app override forces type or paste.
- **Secure Input:** detect it and show it in the menu-bar icon along with the app responsible, rather than failing silently.
- **App rules:** allow/deny lists by bundle ID on **groups and snippets**, where the snippet overrides the group. Insertion-method overrides follow the same inheritance.
- **Default exclusions:** 1Password, Bitwarden, Keychain Access and the login window. Terminals stay enabled, with paste preferred.

## Templates (variables)

- Stored as a plain-text template syntax, for easy import/export and sync. Shown in the editor as **inline chips**, inserted with `/` or a `+` menu. Clicking a chip opens a popover with settings and a live preview.
- **v1 variables:**
  - **Date/time:** format presets plus a custom format, offset (±N days/weeks/months), locale.
  - **Clipboard:** plain text, optional trim of whitespace and newlines.
  - **Cursor:** where the caret lands after expansion.
  - **Nested snippet:** with cycle detection.
  - **Form fields:** text field, dropdown.

## UI

- **Window behavior:** the library uses a normal, resizable macOS window with native full-screen support. It stays on its own Space and does not float above other apps. The search palette appears only when explicitly invoked.
- **Settings:** a modal surface inside the main window, with the library/editor kept mounted behind it. Closing Settings restores the previous editing focus and selection.
- **Editing:** changes remain discardable until Save (⌘S) or navigation. Navigation saves valid pending trigger input and the current draft; failures retain the draft. Discard reloads the latest stored version without overwriting it. Normal quit is refused if any draft or unfinished trigger cannot be saved. Crash/force-quit recovery is not implemented.
- **Data operations:** manual backup, export, import and restore prepare pending edits before taking a snapshot. Deletion supports Undo/Redo with atomic local transactions and refuses replay over newer records or changed group contents. Failed undo/redo remains available as an explicit retry.

- **Menu bar:**
  - Pause/resume
  - Quick search
  - New snippet from selection
  - Open Kudzu
  - Status for permissions, Secure Input and sync
- **Main window (three panes):**
  - **Sidebar:** All, Favorites, Recently Used, Groups (nestable, drag to reorder), Stats.
  - **List:** name, trigger tags and use count, with search.
  - **Editor:** name, triggers as tags, the chip content editor, live preview, and collapsible Rules and Advanced sections.
  - ⌘N creates a new snippet.
- **Search palette:**
  - Global hotkey **⌃⌥Space** (configurable).
  - Fuzzy search over name, trigger and content, with ranking boosted by usage.
  - ↵ inserts into the previous app, ⌘↵ copies, ⌘E opens the snippet in the editor.
- **Forms:**
  - A floating panel at the text caret (found via the Accessibility API), falling back to the screen center.
  - Live preview of the final text.
  - Tab/⇧Tab between fields, ↵ inserts, Esc cancels and leaves the trigger as typed.
  - Fields remember their last values.
- **Onboarding (three steps):**
  1. Permissions (Accessibility / Input Monitoring). Each step deep-links to System Settings and detects the grant live, with no restart.
  2. Espanso import if detected.
  3. A practice field: type `;kudzu`.

  A revoked permission later shows a warning in the menu bar.
- **Stats dashboard:**
  - Per-snippet use count, last used and characters saved.
  - Estimated time saved, using a configurable WPM.
  - **The typed text itself is never recorded.**
  - No analytics or telemetry.

## Import / export

- **Import:** Espanso YAML (matches, vars, forms where mappable, word setting) and Kudzu JSON.
- **Export:** Kudzu JSON (lossless) and Espanso YAML (best-effort, so users can always leave).

## Scope

**v1:**
- Everything above, including iCloud sync and usage stats.

**v2:**
- Rich text and images
- Shell / JavaScript snippets
- Regex triggers
- iOS app + keyboard extension

## Milestones

| | Goal | Done when |
|---|---|---|
| **M0 Spike** | Prove the risky platform pieces | Event tap, keystroke and paste insertion, and caret-positioned panel all work in **both** a Developer ID build and a sandboxed build |
| **M1 Core** | `KudzuCore` package | Trigger matching, case propagation, word mode, template parser and renderer, SwiftData models (CloudKit-safe), dedupe, Espanso import, all unit-tested |
| **M2 Daily driver** | Usable app | Menu-bar app, onboarding, main window and chip editor, expansion working system-wide |
| **M3 Power** | Pro features | Search palette, forms, app rules, Secure Input indicator |
| **M4 Data** | Sync and safety | iCloud sync with status UI, per-device stats and dashboard, backups and restore, export |
| **M5 Ship** | Distribution | Sparkle, notarized DMG, Homebrew tap, App Store submission, CI release pipeline |
