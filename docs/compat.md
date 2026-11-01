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

Follow [M0 manual tests](M0-spike.md#run-the-manual-tests). Use separate fresh snapshots for the two builds because they share a bundle identifier. Record Input Monitoring, AX trust, Post Event, Secure Input, and global hotkey results alongside app-specific failures.
