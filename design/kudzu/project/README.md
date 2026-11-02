Kudzu is a lightweight, native macOS text expander. The interface should feel like a quiet notebook: neutral paper, one living chartreuse, the user's triggers set in monospace as the only typographic flourish, and a thin layer of glass at the edges. It does not follow stock macOS styling; it uses its own type, its own icons, and its own controls, built in SwiftUI.

## Principles

1. **Quiet canvas, one living colour.** Neutrals do the work. The chartreuse accent marks only the thing that is active or primary right now: the selected row, the primary button, the focus ring, a toggle that is on. If two things on screen shout chartreuse, one of them is wrong.
2. **Glass only at the edges.** The sidebar, menus, popovers and the floating toolbar are glass. Content panes are solid. Never stack glass on glass.
3. **Triggers are the hero.** Every trigger is a TriggerChip in `trigger` (Geist Mono), with its prefix symbol in `vine-ink`. The snippet's name in the detail pane is set in mono too.
4. **Hairlines, not boxes.** Separate with spacing first, `line` second, shadows only for floating layers.
5. **One window.** Library, stats and settings live in the same window and the same three-pane layout. There is no separate Settings window.

## Content fundamentals

- Sentence case everywhere: "All snippets", "Recently used", "Import & export", "Show welcome again". Product name is "Kudzu".
- Second person, short, calm. Say what to do: "Choose a snippet to edit it, or press ⌘N to create one." No exclamation marks, no emoji in UI copy.
- Name shortcuts with Mac glyphs in a Kbd: ⌘N, ⌘F, ⌘, — never "Cmd+N".
- An ellipsis on any command that asks for more input before acting: "Restore…", "Import…".
- Counts are bare numbers in `caption` ("18"); units in lowercase ("40 wpm", "18 snippets").
- Show triggers exactly as typed (`:an`, `;sig`): never capitalise, translate or truncate them.

## Visual foundations

**Colour.** Two themes, Light and Dark, with identical token names; every pair is checked in both. Surfaces step from `canvas` (behind the window) to `surface` (list pane, settings pages) to `surface-raised` (detail pane, cards, buttons); `surface-sunken` is for wells (fields at rest, search, tracks). Text is `ink` or `ink-muted`; `ink-faint` is for disabled text and decorative marks only. Hover and pressed states are the translucent overlays `surface-hover` and `surface-pressed`, so they work on any surface. The accent comes in two tokens. `vine` is chartreuse and is only ever a FILL (primary button, toggle on); it is too light to read as text on white. `vine-ink` is the accent for text, icons, thin marks and focus: deep olive in light, the same chartreuse in dark. Text on a vine fill is `on-vine`, near-black in both themes. Selection is `vine-soft` with `ink` text, never a solid accent bar. `bloom`, the kudzu flower, is reserved for favorites. `warning` and `danger` always come with a word or icon.

**Glass.** A glass layer is three things: a translucent fill (`glass` for the sidebar at 84%, `glass-strong` for menus and the toolbar at 92%), a backdrop blur of `blur-glass` with 160% saturation, and a 1px `glass-stroke` highlight on the inside top edge plus a 1px `glass-edge` outside. The fill, not the blur, carries contrast, so text on glass stays legible over any wallpaper. The active sidebar item is a solid `glass-selected` pill with `shadow-control` and a `vine-ink` icon.

**Type.** Two families from one superfamily. UI text is Geist (`sans`), triggers and literal text are Geist Mono (`mono`). Both are OFL licensed; bundle their font files in the app. Default UI text is `body` (13/18); row titles and sidebar items are `body-strong`; the user's snippet content gets `content` (14/22) so their own words read comfortably; pane titles are `title`; settings page titles `title-lg`; numbers on Stats are `stat` with tabular figures.

**Space and size.** A 4px base: `space-1` … `space-12`. Pane headers are `size-titlebar` (52px) so they line up with the traffic lights. Controls are `size-control` (30px), text fields `size-field` (34px). The sidebar is `size-sidebar` (220px), the list pane `size-list` (300px), the detail pane takes the rest.

**Radii.** `radius-xs` for inline tags and keycaps, `radius-sm` for chips, small buttons and menu items, `radius-md` for buttons, fields, rows and sidebar items, `radius-lg` for cards, the editor, menus and the toolbar, `radius-xl` for the window, `radius-full` for toggles, badges and bars.

**Shadows.** None on content. `shadow-control` lifts buttons, the selected segment and the active sidebar pill; `shadow-glass` and `shadow-float` belong to glass layers; `shadow-window` is for mockups only.

**Focus.** Keyboard focus is a solid 2px `focus` ring (an alias of `vine-ink`) with a 2px offset; fields show it as a 2px `vine-ink` edge. It is at least 4.6:1 on every surface in both themes.

**Motion.** 160ms ease-out for colour changes, 180ms with a slight overshoot for the toggle knob, no motion on layout. Respect Reduce Motion.

## Layout: one window

- **Sidebar (glass).** Library: All snippets, Favorites, Recently used, Stats. Groups, with a + to add one. Settings is pinned to the sidebar footer with ⌘, as its shortcut.
- **List pane (`surface`).** A ListHeader with the title, count, sort and new-snippet actions; a SearchField that filters this list; then rows. In settings mode this pane lists the settings sections: General, Sync, Backups, Import & export, About.
- **Detail pane (`surface-raised`).** For a snippet: a floating ToolbarGroup (favorite, debug, more), the name as a mono title, Triggers, Content, Preview, then Rules and Advanced as Disclosures, and a one-line footer of dates and use count. For settings: a page title and SettingsGroups on `surface`. For Stats: StatTiles and UsageBars.

See the Window and SettingsWindow pages for the whole layout, live.

## Iconography

Kudzu has its own icon set, drawn for this system: 16px on a 16-unit grid, 1.5px strokes, round caps and joins, no fills except the favorite star. They live in the Icon component (`Kudzu.Icon.names` lists them). Use 16px in the sidebar, toolbar and settings, 14px in small buttons and menus, 12px inside chips and rows. Icons are `ink-muted` at rest, `ink` on hover, `vine-ink` when active. Don't mix in SF Symbols; draw a new glyph at the same weight instead.

## App icon and menu bar icon

The mark is a text cursor (I-beam) with two leaves sprouting from it: a few keystrokes grow into a whole passage. Both icons are in the Logos assets.

- **App icon** (`kudzu-app-icon.svg`, `kudzu-app-icon-1024.png`): the macOS 824 squircle on a 1024 canvas, a deep olive ground (`#2e3f17` to `#0d1208`), the cursor in off-white `#eef3e2`, the leaves in two-tone chartreuse around `vine`. Use the 1024 PNG wherever the app is shown in a page or deck; never redraw it, recolour it or put it on a chartreuse ground.
- **Menu bar icon** (`kudzu-menubar.svg`): an 18pt template glyph, the cursor plus one solid leaf. `kudzu-menubar-paused.svg` outlines the leaf for when expansion is paused. Both are black on transparent; macOS tints them for light and dark menu bars, so never colour them.
- In product UI text, the name stays plain type: "Kudzu" in `title-lg` or `stat`, no lockup.

## Building it in SwiftUI

- Colours: one Color Set per colour token in the asset catalogue, with Any and Dark appearances set to the Light and Dark values here; reference them by token name.
- Glass: an `NSVisualEffectView` (or `.background(.ultraThinMaterial)`) with `.behindWindow` blending for the sidebar, overlaid with the `glass` colour, a top 1px `glass-stroke` line and a 1px `glass-edge` border. Menus and the toolbar use the same recipe with `glass-strong`.
- Window: hide the standard toolbar and title, keep the traffic lights, and draw the pane headers yourself at `size-titlebar`.
- Settings: route ⌘, to switch the main window into settings mode instead of opening a `Settings` scene.
- Fonts: add the Geist and Geist Mono files to the bundle and list them under `ATSApplicationFontsPath`; map each type style to `Font.custom(_:size:)` with its weight and line spacing.
