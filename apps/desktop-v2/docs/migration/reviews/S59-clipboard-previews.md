# S59 review: clipboard previews

This is an independent review by Grok 4.7 of `refactor(desktop-v2): add clipboard previews, metadata and secret handling` (`d7c3b026`).

**Verdict:** fixes needed, with no blockers. Concealment holds, but four Accept items are not met: the reveal, the 200-file preview, the large preview's size, and two tests. Running `cargo test -j 2 -p kudzu-ui clipboard` gives 58 passed, 0 failed.

## Majors

**M1. A revealed secret stays unselectable and out of the accessibility tree** (`clip/secret.rs:79–80`, `:100–110`; `clip/list.rs:169–186`, `:267`; Tauri `secret.tsx:36–40`, `:60–64`; FEATURES CLP-16).

The body is always built with `group_opacity(0.0, 1.0)`, `a11y_hidden()` and `inert()`. `hover_group_focus` changes only the paint. Tauri's text becomes visible, selectable and part of the accessibility tree on hover, or on keyboard focus (`group-focus-visible`). Row titles have the same paint-only hide; on hover, Tauri adds the title to the tree.

- **Failure:** Tab to a concealed preview of `hunter2`. The password paints, but it can't be selected, and VoiceOver still reads only "Hidden: … Hover or focus to show."
- **Fix:** rebuild the revealed state without `inert` and `a11y_hidden`. For the preview, do this on hover and on keyboard focus; for the row title, on hover. The covered state stays non-selectable and silent.
- **Test:** focus `clip-concealed`, then assert the text isn't inert and its accessible name is the clip text.

**M2. A 200-file preview leaves the on-screen icons grey and starts 200 threads** (`clip/content/loader.rs:16`, `:116–123`; `main/clipboard/mod.rs:293–309`; `kudzu-app/src/tasks.rs:110–128`; Tauri `clip-content.tsx:247–250`).

The media cache holds 96 entries and evicts slots that are still `Loading`, and dropping a `Task` doesn't cancel it. `track_active` requests `Icon(0..200)` once and never again.

- **Failure:** copy a folder of 200 files. The first 104 rows, the ones on screen, stay grey squares. Up to 200 threads start at once. Coming back to the clip doesn't request the icons again.
- **Fix:** keep the icons the open clip and the visible rows still need, or ask for any missing key those views show. Bound the workers, so 200 threads never start in one turn.

**M3. The large preview isn't min(920, 90%) by 86% of the window** (`clip/large.rs:37–46`; `widgets/kit_ext/dialog.rs:113–139`; `main/clipboard/view.rs:372–377`; Tauri `clipboard-view.tsx:602–625`; FEATURES WIN-2).

The dialog draws a visible title, the sentence "Large preview. Press Space or Escape to close." and a Close button. It caps only the width. The body is capped at the content card plus 58, minus 120, not at 86% of the window. Tauri makes the title and description `sr-only`, and caps the dialog at `86vh` × `min(920px, 90vw)`.

- **Fix:**
  - Keep the title and description out of the visual tree.
  - Cap the panel at min(920, 90% of the window width) by 86% of the window height.
- **Test:** assert both dimensions.

**M4. Two Accept tests can pass without checking their item** (`main/tests/clipboard_preview.rs:152–167`, `:252–266`).
- `many_files_stop_at_two_hundred` joins 230 URLs into one pasteboard item. `classify` reads one path from it, so `if files.len() > 200` never runs.
- `code_shows_…` asserts only inside `if kind == Code`, and never checks the line gutter.

- **Fix:**
  - Build 230 separate file-url items, and assert the note and that row 200 is absent, with no `if`.
  - For code, assert the kind, the note, and that the gutter contains `1` and `1500` but not `1501`.

## Minors

1. **Space closes the large preview even while text is selected** (`main/clipboard/keys.rs:99–107`). Tauri ignores Space inside `.selectable` (`clipboard-view.tsx:610–612`).
2. **Timeline dots use `El::help`, the native tooltip** (`clip/preview.rs:150`). PLAN §1.4 says to use the styled `Tooltip`.
3. **An empty `timings` list still draws one dot** at `copied_at` (`clip/timeline.rs:26–28`). Tauri draws dots for `entry.timings` only.
4. *(Fixed already, by the S58 fixes `d486b169`: `www.` links get `https://`.)*
5. **The single-image well stays empty until `File(0)` or `Thumbnail` is ready** (`clip/content/files.rs:92–110`). Fall back from the file, to the thumbnail, then to nothing, as `clip-content.tsx:243` does, and test the fallbacks.
6. **Some tests skip what they claim:**
   - The colour test calls `copy_value` itself instead of pressing "Copy HEX value".
   - The link test doesn't check the host.
   - The image test waits only for the request.
7. **The app badges are untested:** `app_badge` (`clip/preview.rs:227–237`) and the tile badge (`clip/tile.rs:121–136`).
8. **The row-hover test** finds the dots and the title in the tree, but never moves the pointer or reads the opacity.

## What holds

- **Concealment and secrets.**
  - While covered, the preview's label carries no content, and the body is inert and at least 112 pt tall. Dots are clamped to 8–18.
  - Show secrets, "Not a secret?" and "Hide as a secret" all work, each with an Undo toast.
  - Deleting a secret says "Deleted a secret".
- **Text and code.**
  - Text is selectable, search-marked, loaded off the UI thread, and cut at 60,000 characters with the count of what's left.
  - Code is mono, numbered, and cut at 1,500 lines.
- **Links and colours.**
  - Link and email copy matches Tauri, and no thumbnail is fetched.
  - Colour values and contrast use the same OKLCH matrix and luminance thresholds as `clip-content.tsx:141–152`.
- **Images and other data.** Image alt text matches, and the original is preferred over the thumbnail. "Other" data says every format is kept.
- **Metadata.** The metadata rows, the Formatted badge, the formats expander and the timeline (hidden when compact) are present.
- **Loading.**
  - Full text is one `Task` per `(id, hash)`, and `show` drops the previous one.
  - The 5,000-clip scroll test passes with no view commit.

## S71 live checks

Carry forward the S58 list, and add:
- **VoiceOver on a concealed preview**, before reveal and after hover and Tab. While covered, it speaks only "Hidden: …". After reveal, the text is readable and selectable.
- **Concealed row titles.** Hovering one paints the title, and VoiceOver reads it. Keyboard focus on the row doesn't reveal it, matching Tauri.
- **Secret actions.** Show secrets, "Not a secret?" and "Hide as a secret", each with Undo. Delete one secret, then several.
- **Copying a value.** Copy a HEX value and confirm a new history row appears, side by side with Tauri. Compare OKLCH and the contrast chips for a transparent colour.
- **Images and files.**
  - A single image file, including one over 80 MiB.
  - A folder of 200 files: the rows on screen show Finder icons after scrolling away and back.
- **Large preview.** At 1180×760 and at 900×560, side by side with Tauri: the size, no second title, Space with text selected, then Escape and the arrows.
- **Smaller details.** The source-app badge, the timeline tooltip, the still shimmer and the checkerboard.
- **Load and logs.** Scroll 5,000 clips with a heavy preview open. Logs and toasts never contain clip text.
