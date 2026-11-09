# Form parity

Source: `apps/desktop/components/app/form/form-panel.tsx`. The panel chrome (440 pt wide, radius 16, caret placement, 160…680 clamp) belongs to the shell.

- `form-panel.tsx` `FormPanel` — matched. One view, `FormView`, bound to `AppModel.formRequest` (`FormPayload`). No second form model.
- Fields — matched. Text is a `TextField` (`.kzInput`), multi-line is a `TextEditor` (`.kzTextarea`, fixed height, the web's `rows={3}`), choice is a menu `Picker` (`.kzSelect`) instead of a hand-drawn `<select>` and chevron. `kzSelect` sizes the pop-up to its label, so a choice does not stretch to the web field's `w-full`. Labels sit above the controls, as in the web panel; this is not a settings `Form`.
- Remembered values — the form uses the payload's `initialValues` unchanged, as the web panel does. The backend computes these values through `FormMemory.initialValues` before sending the request.
- Keys — matched. Esc cancels (`cancelForm`). Return submits, including on a choice. Return inside a multi-line field inserts a newline; ⌘Return submits. Tab is the system focus order, first field focused when the request appears, and the footer says "Next field".
- Live preview — matched. An immediate render from the in-memory library, replaced by `renderPreview` 50 ms after the values change (nested snippets and the real clipboard). The well is capped at 160 pt (`max-h-40`).
- Header — matched: snippet name, or "Fill in snippet", and "into {app}" when `targetApp` is set.
- Footer — matched: Tab, Esc, and the iris Insert button with the Return keycap.
- Content size — matched. The view reports its height through `resizeForm`. The native panel is already the vibrant surface, so the web's extra 24 pt of non-vibrant padding is not added (`FormLayout`, cap 640). The shell clamps to 160…680.
- Header drag region — not matched here. `FloatingPanel` does not drag from the background (`isMovableByWindowBackground = false`); that is a shell behaviour for T12.
- Empty request — the web renders nothing. The native view shows "Nothing to fill in" so a panel opened before a payload is not a blank surface.
