# Palette parity

Source: `apps/desktop/components/app/palette/palette.tsx`. The panel chrome (760×480, radius 18, popover material, hide on blur) belongs to the shell.

- `palette.tsx` `Palette` — matched. Search field, Tab / ⇧Tab scopes, `#group` filter, ranked hits, live preview, ↵ insert, ⌘↵ copy, ⌘E edit, ⌘N create, ⌘1–9, Esc clears then hides, create-from-query row, target app shown from `AppModel.paletteTarget`.
- `palette.tsx` `Highlight` — matched (`PaletteHighlight`). Hit indexes are UTF-16 units, which is what the Swift searcher records.
- `palette.tsx` `KindIcon` — matched (`PaletteKind`): form, clipboard, date/time, otherwise a document.
- `palette.tsx` `ResultRow` — matched as a `List` row. Fixed 44 pt, favorite star, trigger chip, ⌘-number badge that lights up while Command is held. `Item` from KudzuDesign is the library row's metrics, so the palette draws its own row content inside the native list.
- Scope radiogroup — matched with `SelectionPill`, not a segmented `Picker`. The web's buttons are not in the tab order (Tab cycles the scope), and `SelectionPill` is the control KudzuDesign provides for this strip. A segmented picker would take focus and ignore that.
- `CommandList` — not used. It is the ⌘K command menu: no preview pane, no scopes, no `#group` filter, no usage ranking, no create row. The quick-search panel is a native field plus a `List`, not a second command menu.
- Footer hints — matched with `Shortcut` and the return glyph. "Insert into {app}" is the target-app chip; after ⌘↵ it reads "Copied".
- Live preview — matched. The active row renders through `renderPreview` after 40 ms, with the shimmer until it arrives. The form note ("Asks for …"), triggers, group path and used-count are the same.
- Empty states — matched: "No favorites yet.", "Nothing used yet.", "No snippets yet.", plus "Type to create one."
- Window blur hides the palette — not in this view. The shell's panel already resigns on focus loss. Dragging the panel is not a palette behaviour (the web window is a separate surface).
