# AppShell

The one Kudzu window: a glass sidebar, the list pane on `surface`, the detail pane on `surface-raised`. Settings is a mode of this window, not a second window.

Consumer provides `sidebar` (SidebarSections), `sidebarFooter` (the Settings SidebarItem), `list` (ListHeader, SearchField and rows) and `detail`. Widths come from `size-sidebar` and `size-list`; every pane's header row is `size-titlebar` tall. The window itself has no fill, so the sidebar's backdrop blur reaches the desktop, as a macOS sidebar does.
