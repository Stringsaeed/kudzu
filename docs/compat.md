# App compatibility

Leave a cell empty until tested. Record `pass`, `fail`, or `blocked`, plus a short note or evidence link. For panel placement, record `AX selection`, `AX element`, or `mouse`; only `AX selection` proves caret placement.

Test date:
macOS version and hardware:
Kudzu commit and signing identity:
Keyboard layout and input method:
Displays and arrangement:
TCC state and fresh-account/VM procedure:
App versions:

| App | Direct typed insert | Direct paste insert | Direct caret panel | App Store typed insert | App Store paste insert | App Store caret panel |
| --- | --- | --- | --- | --- | --- | --- |
| TextEdit | | | | | | |
| Notes | | | | | | |
| Safari | | | | | | |
| Chrome | | | | | | |
| VS Code | | | | | | |
| Slack | | | | | | |
| Terminal | | | | | | |
| iTerm2 | | | | | | |
| Xcode | | | | | | |
| Messages | | | | | | |

Follow the engine manual test below and the [M0 platform notes](M0-spike.md). Use separate fresh snapshots for the two builds because they share a bundle identifier. Record Input Monitoring, AX trust, Post Event, Secure Input, and global hotkey results alongside app-specific failures.

## Engine manual test

Use disposable documents, with only one build running. Grant Input Monitoring and
Post Event in Settings; grant Accessibility for the Direct build's active tap.
Wait about one second for permission changes. The App Store target stays listen-only.

1. Create a snippet in the main window with trigger `;kz` and content `Kudzu works! 🌱`.
2. In TextEdit, type `;kz` fast followed immediately by more text. In the Direct
   active mode, nothing should be lost or reordered. Repeat with two consecutive
   triggers and with a trigger typed during a long paste.
3. Change the snippet to whole-word mode and type `;kz.`. Expect exactly
   `Kudzu works! 🌱.`, retaining one period. Also test space, Return and Tab.
4. Immediately press backspace after an expansion. Expect the original typed
   trigger (and its separator for whole-word mode). Repeat after waiting a second
   to check that a usage-save notification does not disarm undo.
5. Create a `{{cursor}}` snippet, including emoji and combining marks after the tag.
   Verify typing lands at the tag and backspace undo removes the entire expansion.
6. Create a long snippet with forced paste. Copy a recognizable string, expand,
   then paste elsewhere after one second to verify clipboard restoration. Repeat
   while making a new copy during the restore delay; the new copy must survive.

Also check pause/resume, Secure Input, app switches, clicks, navigation, permission
revocation/regrant, snippet edits, nested snippets (including clipboard tags),
case propagation, app rules, and New Snippet from Selection with AX and Copy fallback.
Palette hotkeys and caret/form panels are retained adapters and are not wired yet.

Listen-only mode cannot suppress keys: continued typing may race deletion or paste.
The one-second active-mode safety valve cancels pending work and releases held input;
after a timeout or focus change, ordering relative to the destination's own work
cannot be guaranteed. CGEvent posting has no delivery acknowledgement. Dead keys,
IME composition, autocorrect, application-specific grapheme deletion/arrow behavior,
and clipboard Copy/paste timing require manual verification.


The Copy fallback watches clipboard changes for up to 300 ms; the platform cannot
identify which app caused a change. Avoid copying elsewhere during that window.
A posting failure after trigger deletion can leave a partial edit; events have no
acknowledgement or transactional rollback. The timeout runs on the main actor and
cannot interrupt a synchronous OS call (for example, reading a huge clipboard).
