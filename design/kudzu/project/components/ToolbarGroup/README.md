# ToolbarGroup

A small glass capsule that floats icon buttons over the detail pane: the one place glass appears inside the content area.

Consumer provides IconButtons as children, and may separate clusters with `<span class="kz-toolbar-sep">`. Fill `glass-strong`, edge `glass-edge`, `shadow-glass`, backdrop blur `blur-glass`, corners `radius-lg`.

- Keep it to 2–4 actions about the selected item (favorite, debug, more).
- Never nest glass in glass: no ToolbarGroup inside a Menu or on the sidebar.
