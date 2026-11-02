# Design system additions

These glyphs were drawn for the app in the design system's style, so they can be added back to `design/kudzu/project/`. Nothing here is generated from the design system; the Swift source in `Packages/KudzuUI` is kept identical to these files by tests.

- `kudzu-menubar-permissions.svg`: 18pt template glyph for missing Accessibility or Input Monitoring. It has the outlined leaf, as in the paused glyph, plus a filled warning triangle with a knocked-out exclamation in the free bottom-right corner. Ink #000.
- `kudzu-menubar-secure-input.svg`: 18pt template glyph for when Secure Input blocks expansion. It has the outlined leaf plus a small padlock in the same corner. Ink #000.
- `icons/*.svg`: 17 new 16px icons on the 16-unit grid, with 1.5 strokes and round caps and joins: warning, error, alert, check-circle, minus-circle, shield, lock, pause, play, copy, cursor, grip, external, text-plus, list, folder-minus and inherit.
  - `icons/ICONS-additions.js.txt` has them as entries for the `ICONS` map in `components/bundle.js`.
  - Regenerate the folder with `python3 scripts/design/export-icon-additions.py`.
