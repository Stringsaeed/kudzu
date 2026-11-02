# Icon

Kudzu's own 16px line icons: 1.5px strokes, round caps and joins, drawn on a 16-unit grid.

Consumer provides `name` (one of `Kudzu.Icon.names`), optional `size` (12, 14 or 16; 28 only for the empty-state mark) and `label` when the icon carries meaning on its own. Icons take `currentColor`, so color them through their parent: `ink-muted` at rest, `ink` on hover, `vine-ink` when the thing they mark is active, `bloom` only for the favorite star.

- Do pair an icon with a text label in the sidebar, menus and settings rows.
- Do use `star-fill` in `bloom` for favorites, and nothing else in bloom.
- Don't mix in SF Symbols or another set; add a new glyph to this set at the same weight instead.
