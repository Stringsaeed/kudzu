# Parity backlog

Every known difference between the native app and the web UI. The goal is an empty list: each
item gets fixed here, or in Rune (as an issue upstream) when the app can't reach it. An item
leaves this file only when it matches.

## Kit: controls

- [ ] Dark raised controls: the 1 pt rim reads ~4/255 darker (Rune draws outer shadows under a
      transparent border; CSS clips them).
- [ ] Switch thumb `scale-x-[1.2]` while pressed stretches as a pill, not an ellipse.
- [ ] `↵` is drawn as an icon because Rune's fallback font renders U+21B5 small.
- [ ] `FormattedText`: per-link `title` tooltips; inline images are letterboxed square.
- [ ] Controls in a `memo` don't show their pressed look (press is tracked per id outside state).

## Kit: overlays

- [ ] Tooltips don't open on keyboard focus or close when the trigger is pressed (needs a focus
      and press hook on a foreign trigger).
- [ ] Submenus: Radix's diagonal pointer grace is a 100 ms rest instead.
- [ ] A right-click outside an open menu doesn't dismiss it (Rune's modal only takes primary presses).
- [ ] Popovers trap Tab; Radix's popover is non-modal.
- [ ] Toasts: swipe to dismiss and Alt+T (focus the toaster) aren't ported; timers use the wall
      clock, so headless runs don't auto-dismiss deterministically.
- [ ] AlertDialog `text-balance` wrapping is approximated.

## App-wide

- [x] Dark-mode text read heavier than Chrome's: every window now uses `FontSmoothing::Off`, the
      Rune mode for `-webkit-font-smoothing: antialiased` (re-verify in the screen ports).

## Editor

- [ ] Caret blinks with a fade, not CodeMirror's hard on/off.
- [ ] Favourite star burst lacks the −24° turn (Rune sequences animate scale and opacity only).
- [ ] Chip editor / insert menu scroll inside a short window instead of overflowing the screen.
- [ ] Kit `FormattedText`: inline images in the preview don't grow the line height.
- [ ] Kit `Command`: ~4 pt less padding above the search field than shadcn's.

## Settings, insights, onboarding

- [x] Onboarding glow: Chrome sizes `ellipse farthest-corner` with farthest-side proportions;
      the port used closest-side. Fixed; the profile matches the web pixel for pixel on the
      centre row.
- [ ] Onboarding vertical rhythm is ~5 pt tighter than the web.
- [ ] Gradients aren't dithered (Chrome dithers); faint banding on large soft gradients.
- [ ] Check marks start at 50% scale (Rune's floor) instead of 40%.
- [ ] Insights bars reveal left-to-right instead of stretching horizontally.
- [ ] Kit Button has no spinner slot; busy buttons use an empty label plus a spinner.
- [ ] Focus doesn't return to the import button after an import (no focus-by-key API).
- [ ] Permission companion finds its NSWindow by title and polls on a thread.

## Library

- [ ] Sidebar nav is one Tab stop with arrow roving (Rune can't focus a plain element from code).
- [ ] A count remounted unchanged (unfolding a section) doesn't pop.
- [ ] Match underlines can't be offset.

## Clipboard

- [ ] Keep history submenu row can't show its trailing "1 week" (kit `SubMenu` needs a hint).
- [ ] Copy-timeline dots don't grow on hover (no hover scale in Rune yet).
- [ ] The large preview's scrim doesn't blur the native sidebar vibrancy behind it.

## Client logic (`kudzu_core::client`)

- [ ] `localeCompare` beyond Latin scripts is approximate (no ICU collator).
- [ ] Dates and times are en-US only (the web used the system locale through `Intl`).
- [ ] Clipboard excerpts heavy with emoji are treated as full text (`inline_full_text`); a bug in
      both apps, kept for parity until both are fixed.

## Shell (from the backend review)

- [x] Floating panels and the permission companion join every Space, as Tauri's
      `visible_on_all_workspaces(true)` did (`CanJoinAllSpaces` set in `ui/mod.rs`).
- [x] Login item: `~/Library/LaunchAgents/<Name>.plist` written exactly as tauri-plugin-autostart
      2.7 / auto-launch 0.6 wrote it (label, empty `AssociatedBundleIdentifiers`, canonical exe,
      `--hidden`); tested byte for byte.
- [x] A form window closed any way other than Esc/↵ cancels its request, so the engine is never
      left suppressed.
- [ ] Permission companion refreshes status when it regains focus, as permission-center.tsx did
      (handed to the settings port).
- [ ] Permission companion retries the draggable-icon install once its NSWindow exists (handed to
      the settings port).
