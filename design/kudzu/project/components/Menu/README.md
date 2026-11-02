# Menu

A floating menu (sort, debug, insert tag, select options): a glass panel of items with an optional title and separators.

Consumer provides `title` and `items`: `{label, checked?, icon?, shortcut?, active?, danger?}` or `'-'` for a separator. Fill `glass-strong`, `shadow-float`, `radius-lg`; items are 28px, `radius-sm`, hover and keyboard focus `vine-soft`; the check is `vine-ink`.
