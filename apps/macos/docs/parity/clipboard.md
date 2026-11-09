# Clipboard parity

Native behavior and layout still need verification in a running window; the statements below describe the implementation and existing rule tests.

| Area | Status | Notes |
|---|---|---|
| Preview limits and highlights | Matched (tested) | 60,000 UTF-16 units, 1,500 code lines, merged highlight ranges; row titles capped at 200 UTF-16 units. |
| Full text | Matched (tested) | Excerpt completeness uses UTF-16 length. |
| Ordering and timeline | Matched (tested) | History suppresses pinned-first while searching; picker never pins first. Timeline falls back to copied time. |
| History selection | Native | SwiftUI `List` uses `Set<UUID>` selection; macOS supplies Command-click and Shift-click selection. |
| Clear history | Implemented | History options menu has a pinned-items checkbox; the destructive alert confirms the selected scope and calls the existing clear API. SwiftUI `Alert` does not support a checkbox in its actions, so the checkbox is in the menu. |
| Retention and usage | Implemented | Options menu updates the existing setting; footer shows retention and disk usage; welcome names retention and picker shortcut. |
| Secret preview | Implemented | Content is replaced by a focusable cover until hovered or focused, unless Show secrets is enabled. |
| Color preview | Implemented | Swatch shows contrast samples on white and black; format copy buttons remain visible. |
| Picker detail | Implemented | Shared detail view uses compact heading and facts; history keeps its timeline and formats. |
| Filters and row presentation | Native choices | Filter menu retains counts; row density and always-visible copy controls differ from web. The checkerboard uses 7 pt cells. |
| Keyboard and feedback | Implemented | Picker chords, history keys, timed toasts and undo are implemented. |

## Build note

`swift test` for `KudzuAppTests` is blocked by unrelated Settings compile errors outside clipboard ownership; a temporary copy with those bypassed also encountered unrelated Insights compile errors. Existing rule tests cover truncation, highlights, pin bulk, secret overrides, UTF-16 excerpt completeness, pinned ordering and timeline anchoring.
