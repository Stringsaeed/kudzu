# Editor

The content editor for a snippet: an Insert menu and a hint bar on top, the user's text below at reading size, with dynamic tags shown inline.

Consumer provides `value` as an array of strings and `{tag}` objects (`['Hi ', {tag:'{clipboard}'}, '!']`), optional `minHeight` and `readOnly`. Body text is the `content` style; tags are `code` in `vine-ink` on `vine-soft`, `radius-xs`.

- Tags are atomic: they select and delete as one unit.
- The hint names the shortcut with a Kbd: "Type / to insert a tag".
