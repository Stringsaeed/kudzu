# IconButton

A square, borderless button holding one icon, for toolbars and pane headers.

Consumer provides `icon`, `label` (required: it becomes the tooltip and accessible name), optional `menu` (adds a caret and means it opens a Menu), `active` (a toggled state: `vine-soft` fill, `vine-ink` icon) `size` (`md` 30px | `sm` 24px) and `tone="bloom"` (only for the favorite star).

- At rest the icon is `ink-muted`; hover lays `surface-hover` and turns it `ink`.
- Group related icon buttons in a ToolbarGroup when they float over content.
