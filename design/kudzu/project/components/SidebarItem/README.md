# SidebarItem

A row in the sidebar (and in the settings section list): icon, label, optional count or shortcut.

Consumer provides `icon`, `label`, optional `count`, `shortcut`, `active` and `onClick`. The active item is a solid `glass-selected` pill with `shadow-control` and a `vine-ink` icon, sitting on the glass; there is no full-color selection.

- Labels are sentence case ("Recently used", "Getting started").
- Counts are `caption`, `ink-muted`, tabular.
- Settings is the last item, in the sidebar footer, with ⌘, as its shortcut; choosing it switches the window into settings mode.
