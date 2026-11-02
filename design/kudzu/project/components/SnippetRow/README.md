# SnippetRow

One snippet in the list pane: name, favorite star, group and use count on top; trigger chips and a one-line preview of the content below.

Consumer provides `name`, `triggers` (array), `preview` (the expanded text, plain), optional `group` (show it only in cross-group views: All, Favorites, Recent), `uses`, `favorite`, `selected` and `onClick`.

- Selection is a `vine-soft` fill with `ink` text; hover lays `surface-hover`. No full-color selection, no separators between rows: spacing separates them.
- The star is `bloom`, 12px, and appears only on favorites.
