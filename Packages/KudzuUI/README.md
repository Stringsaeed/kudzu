# KudzuUI

Kudzu's design system in SwiftUI: tokens, fonts, icons, glass and the shared interaction primitives that every component builds on. The source of truth is `design/kudzu/project/` (`README.md`, `tokens.json`, `components/`). macOS 15, Swift 6 language mode, no dependencies.

Kudzu has its own look, but the app must still behave like a Mac app. Components are **custom styles on native controls**, not hand-rolled gesture views.

## Naming

Everything public starts with `Kz`. Types are `KzThing`, view modifiers are `.kzThing(…)`, and token members are the CSS name in lowerCamelCase without its group prefix.

| Kind | API | Example |
| --- | --- | --- |
| Colour | `KzColor.<token>` (dynamic `Color`) | `KzColor.inkMuted`, `KzColor.vineSoft` |
| Colour metadata | `KzColorToken` (`.light`, `.dark`, `.usage`, `.nsColor`, `.opaqueColor`) | `KzColorToken.focus.aliasOf == .vineInk` |
| Type | `.kzTextStyle(_:)`, `KzTextStyle` (`.font`, `.nsFont`, `.attributes(color:)`) | `.kzTextStyle(.bodyStrong)` |
| Spacing | `KzSpace.s1 … s12` | `KzSpace.s4` (16) |
| Radius | `KzRadius.xs sm md lg xl full` | `KzRadius.md` (8) |
| Size | `KzSize.controlSm control field titlebar sidebar list` | `KzSize.titlebar` (52) |
| Blur | `KzBlur.glass` | 28 |
| Shadow | `.kzShadow(_:in:)`, `KzShadow` | `.kzShadow(.control, in: shape)` |
| Fonts | `KzFonts.register()` | call once at launch (idempotent) |
| Icons | `KzIcon(_:size:label:)`, `KzIconName` | `KzIcon(.search, size: 14)` |
| Menu bar | `KzMenuBarIcon.<state>.image()` | `.active`, `.paused`, `.permissionsMissing`, `.secureInput` |
| Glass | `.kzGlass(_:in:blending:shadow:)`, `KzGlass` | `.kzGlass(.strong, in: shape, shadow: .float)` |
| Interaction | `KzInteractive`, `KzControlState` | see below |
| Focus | `.kzFocusRing(_:style:isFocused:)`, `KzFocusRingStyle.ring/.edge` | |
| Hover/press | `.kzHover(_:)`, `.kzStateLayer(_:isHovered:isPressed:)` | |
| Motion | `KzMotion.color/.knob`, `.kzAnimation(_:value:)`, `withKzAnimation(_:reduceMotion:_:)` | |
| Disabled | `KzDisabled.foreground/.fill`, `.kzDisabledForeground()` | |
| Snapshots | `KzSnapshot.write(_:named:to:)`, `.kzPreviewInteraction(…)` | |

Every token member carries its design-system usage note as its doc comment, so Quick Help tells you how it's meant to be used.

Components are named after the design system (`components/<Name>`) with the `Kz` prefix: `KzButton`, `KzTriggerChip`, `KzSettingsRow`. Put each one in `Sources/KudzuUI/Components/<Name>/`.

## Native behaviour rules

Every component must follow these rules.

1. **Wrap a native control.** Use `Button` + `ButtonStyle`, `Toggle` + `ToggleStyle`, `TextField`, `Picker`, `Menu` or `DisclosureGroup` styles. When SwiftUI can't do it, use an AppKit-backed view (`NSViewRepresentable`), for example the editor's `NSTextView`. Never build a control from `onTapGesture`, because it loses keyboard activation, VoiceOver roles and focus.
2. **Build the body on `KzInteractive`.** It gives you hover, pressed, focus and enabled state. It draws the shared `surface-hover` / `surface-pressed` layer and the focus ring, and animates colour with `KzMotion.color`:

   ```swift
   struct KzExampleButtonStyle: ButtonStyle {
       func makeBody(configuration: Configuration) -> some View {
           let shape = RoundedRectangle(cornerRadius: KzRadius.md)
           KzInteractive(shape, isPressed: configuration.isPressed) { state in
               configuration.label
                   .kzTextStyle(.bodyStrong)
                   .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                   .padding(.horizontal, KzSpace.s3)
                   .frame(height: KzSize.control)
                   .background(KzColor.surfaceRaised, in: shape)
                   .overlay(shape.strokeBorder(KzColor.line))
                   .kzShadow(.control, in: shape)
           }
       }
   }
   ```

   If a control changes its own fill on hover instead of using the overlay (the primary button uses `vine-hover`), pass `stateLayer: false` and read `state.isHovered`.
3. **Focus.**
   - Controls use `.ring`: 2px `focus`, 2px offset.
   - Fields use `.edge`: a 2px `vine-ink` edge. Drive it from a `@FocusState`: `KzInteractive(shape, focusRing: .edge, isFocused: focused)`.
   - The ring follows `\.isFocused`. On macOS that is keyboard focus: buttons only take focus by Tab when Keyboard navigation is on, so a click never shows the ring. `KzInteractive` and `kzFocusRing` turn off the system focus effect.
   - Keep the native focus order. Don't add `.focusable()` to views that aren't controls.
4. **Accessibility.**
   - Keep the native role, or add the right one: `.accessibilityAddTraits(.isSelected)` for selected rows, `.isHeader` for section titles.
   - Icon-only controls need a label, e.g. `.accessibilityLabel(KzIconName.more.accessibilityLabel)` or your own text. A `KzIcon` without a `label:` is hidden from VoiceOver.
   - Toggles keep `Toggle` semantics.
5. **Light, Dark and Increase Contrast** are automatic when you use `KzColor`. Never hard-code hex values, and never read `colorScheme` to pick colours, except for shadow tokens, which `.kzShadow` already handles. Under Increase Contrast:
   - `line` becomes `line-strong`
   - `line-strong` becomes `edge`
   - `ink-faint` becomes `ink-muted`
   - glass becomes opaque
6. **Motion.**
   - Colour: `KzMotion.color` (160 ms ease-out).
   - The toggle knob: `KzMotion.knob` (180 ms, slight overshoot).
   - Never animate layout.
   - Use `.kzAnimation` / `withKzAnimation`, which turn animation off under Reduce Motion.
7. **Disabled.**
   - Text and icons use `KzDisabled.foreground` (`ink-faint`).
   - Filled controls use `KzDisabled.fill` (`surface-sunken`).
   - No hover or pressed layer: `KzInteractive` does this when `.disabled(true)` is set.
8. **Glass only at the edges**, never stacked:
   - the sidebar: `.regular`, `.behindWindow`
   - menus and popovers: `.strong`, `shadow: .float`. Drawn in a window (specimens) they use `.withinWindow`; in their own floating panel they use `.behindWindow`, because that window has nothing of its own to blur. `KzMenuSurface`, `KzPopoverSurface` and `KzFloatingPanel` already do this.
   - the toolbar: `.strong`, `shadow: .glass`

   Reduce Transparency switches glass to the opaque tint.
9. **Type.**
   - Apply `.kzTextStyle` to the `Text` itself, not a container. Its padding reproduces the CSS line box, so a single line is exactly `lineHeight` tall.
   - Use `matchLineBox: false` when a native container already sizes the text.
   - For AppKit text use `style.attributes(color:)`.
10. **Icons.**
    - Use `KzIcon` only, never SF Symbols. If a glyph is missing, add it to `KzIconName` (see below).
    - Colour icons through `foregroundStyle`: `inkMuted` at rest, `ink` on hover, `vineInk` when active, `bloom` only for `starFill`.

## Adding a component

1. **Code.** Write it in `Sources/KudzuUI/Components/<Name>/`, from `design/kudzu/project/components/<Name>/README.md` and the exact paddings and states in `components/bundle.css`.
2. **Gallery page.** `Gallery/Pages/<Name>Page.swift`, listed in `Gallery/Catalog.swift` under its section (Controls, Lists & structure or Floating). Give it a `GalleryPageLayout` showing every variant and state. Use `.kzPreviewInteraction(hovered:pressed:focused:)` to show the hover, pressed and focus states statically.
3. **Snapshot test.** Add `Tests/KudzuUITests/Components/<Name>Tests.swift`:

   ```swift
   import KudzuUI
   import SwiftUI
   import Testing

   @MainActor
   @Suite struct ButtonSnapshotTests {
       @Test func states() throws {
           try snapshot("button-states", width: 480) {
               HStack { KzButton("Save") {}; KzButton("Save") {}.kzPreviewInteraction(hovered: true) }
           }
       }
   }
   ```

   This writes `Snapshots/button-states-light@2x.png` and `-dark@2x.png`.
4. **Check.** Run `swift test`, open the PNGs, and look at them.

A brand-new page needs two steps:

1. Create `Gallery/Pages/<Name>Page.swift`, conforming to `GalleryPage` (`id`, `title`, `section`, `icon`).
2. Add one line, `<Name>Page.self,`, to `Catalog.pages`, under its section's comment. Every page appears once.

Gallery pages use the real components, never stand-ins. The shared menus (`GalleryMenus`) and the sample window (`GallerySampleShell`) live in `Gallery/Support/`.

## Lists and the window

**Selectable lists.** Use `List(selection:)`, not a `LazyVStack`: it is backed by `NSTableView`, so it recycles rows (thousands are fine) and gives arrow-key and type-to-select navigation, multi-select, context menus and VoiceOver selection for free. Kudzu rows (`KzSnippetRow`, `KzListRow`) are plain views; the list owns selection and you pass `isSelected`:

```swift
List(selection: $selection) {
    ForEach(rows) { row in
        KzSnippetRow(name: row.name, triggers: row.triggers, isSelected: selection == row.id)
            .kzListRow().tag(row.id)
    }
}
.kzSelectionList()
```

Add `.kzContextMenu(isPresented:)` to a row for its right-click menu and `.kzContextTarget(_:)` to outline the row while the menu is open.

`.kzSelectionList()` turns off the system accent highlight (a small `NSTableView` bridge; if the table isn't found the list keeps the system highlight) so `vine-soft` is the only selection.

**The window.** `KzAppShell(mode: .window)` applies `KzWindowChrome` to its window: hidden title, an *empty unified toolbar* (which makes AppKit build the 52pt titlebar and centre the traffic lights on y = 26, the middle of a `size-titlebar` header), full-size content and a clear background. Use `mode: .embedded` for galleries and snapshots; it leaves the window alone and draws mock lights. Build pane headers with `KzListHeader` / `KzPaneHeader`. Their `kzWindowDragArea()` lets the window be dragged by the header. The snippet's name in the detail pane is mono: `Text(name).kzMonoTitle()`.

**The editor.** The app's `NSTextView` editor adopts the look through `KzEditorStyle` (content attributes, inset, `configure(_:)`) and `KzEditorTag` / `KzEditorTagCell` (the inline tag). `KzEditorFrame` is the chrome around it.


## Floating layers

Menus, popovers and panels live in their own borderless, non-activating `NSPanel`, so they can leave the window and sit over other content. The window is larger than the glass by the shadow token's reach (`KzFloatingChrome.margins`), because `shadow-float` is drawn by SwiftUI like the CSS rather than by the window server.

| API | Use |
| --- | --- |
| `KzMenu`, `KzMenuItem`, `KzMenuSeparator`, `KzMenuHeader`, `KzSubmenu`, `@KzMenuBuilder` | The menu model and DSL (`for`, `if`, `switch` work). |
| `KzMenuButton(title:placement:menu:label:)` | Pop-up and pull-down buttons: Select, Sort, Insert tag, "more". Opens on mouse-down. |
| `.kzMenu(isPresented:title:placement:content:)` | A menu driven by state, attached to any view. |
| `.kzContextMenu(title:isPresented:_:)` | Right-click and Control-click menus. |
| `KzMenu.popUp(at:in:)`, `popUp(from:in:placement:)` | The same from AppKit views. |
| `KzMenuSurface(_:highlightedID:)` | A static specimen for galleries and snapshots. |
| `.kzPopover(isPresented:edge:alignment:behavior:content:)`, `KzPopoverSurface` | Anchored glass popovers with no arrow. |
| `KzFloatingPanel`, `KzFloatingPlacement`, `KzFloatingPanelSurface` | The base panel for the search palette and the form at the caret. |
| `KzToolbarGroup`, `KzToolbarSeparator`, `\.kzInToolbarGroup` | The floating toolbar capsule. |
| `KzSelect(_:selection:options:)`, `KzSelect(_:value:placement:menu:)`, `KzSelectLabel`, `KzSelectStyle` | The pop-up button, on `KzMenuButton(placement: .select)`. |
| `KzIconButton(_:label:caret:menuTitle:placement:menu:)`, `KzButton(_:…menuTitle:placement:menu:)` | Icon and text buttons that open a menu. |
| `.kzContextTarget(_:)` | The outline on the right-clicked row. |

### Attaching menus

```swift
// Select: the checked item opens over the field, like NSPopUpButton.
KzSelect("Group", selection: $group, options: groups.map { ($0.id, $0.name) })

// Select with a custom menu (headers, icons, submenus): the lower-level form.
KzSelect("Mode", value: mode.title) {
    for mode in Mode.allCases { KzMenuItem(mode.title, isChecked: mode == current) { current = mode } }
}
// ...which is KzMenuButton(placement: .select) { … } label: { isOpen in KzSelectLabel(value, isOpen: isOpen) }
//    .buttonStyle(KzSelectStyle())

// Sort: an icon button that opens a titled pull-down. It shows the active look while the menu is open.
KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
    for order in SortOrder.allCases { KzMenuItem(order.title, isChecked: sort == order) { sort = order } }
}

// A text button that opens a menu: the editor's Insert.
KzButton("Insert", variant: .ghost, size: .sm, icon: .plus, iconEnd: .chevronDown, menuTitle: "Insert tag") {
    KzMenuItem("Date", icon: .clock) { insert("{date}") }
}

// Any other control: KzMenuButton with your own label. Headers, icons, shortcuts and submenus all work.
KzMenuButton(title: "Insert tag") {
    KzMenuHeader("Dates")
    KzMenuItem("Date", icon: .clock) { insert("{date}") }
    KzSubmenu("Form field", icon: .type) { KzMenuItem("Text") { insert("{form:text}") } }
} label: { isOpen in MyLabel(isOpen: isOpen) }

// Context menu on a row; `kzContextTarget` draws the right-clicked row's outline while it is open.
KzSnippetRow(name: snippet.name, triggers: snippet.triggers, isSelected: selection == snippet.id)
    .kzContextMenu(isPresented: contextBinding(for: snippet)) {
        KzMenuItem("Duplicate", icon: .copy) { duplicate(snippet) }.keyboardShortcut("d")
        KzMenuItem("Favorite", icon: .star, isChecked: snippet.isFavorite) { toggleFavorite(snippet) }
        KzSubmenu("Move to group", icon: .folder) { for group in groups { KzMenuItem(group.name) { move(snippet, to: group) } } }
        KzMenuSeparator()
        KzMenuItem("Delete", icon: .trash, role: .danger) { delete(snippet) }.keyboardShortcut(.delete)
    }
    .kzContextTarget(contextTarget == snippet.id)
```

Controls that show their own open state read `isOpen` from the label closure; `KzIconButton`, `KzSelect` and the menu form of `KzButton` already do. Inside a `KzToolbarGroup`, `\.kzInToolbarGroup` is true and `KzIconButton` is 28pt tall (30pt wide) instead of 30.

### How menus behave

A menu never takes key focus from the window it opened from (like NSMenu's tracking loop): a local event monitor sends it keys and clicks first, so the window keeps its first responder. When no Kudzu window is key (a right-click in an inactive window), the menu panel becomes key itself without activating Kudzu. Keyboard logic is in `KzMenuNavigator` and pointer hit-testing in `KzMenuLayout`, both pure and unit-tested.

| Native behaviour | Kudzu menu |
| --- | --- |
| ↑/↓ move, skipping separators, headers and disabled items; wraps; ↑ with nothing highlighted goes to the last item | Same |
| Home/End, Page Up/Down, ⌥↑/⌥↓ jump to the first/last item | Same |
| Return or Space chooses; Return with nothing highlighted closes | Same |
| Esc closes | Closes the whole menu, submenus included |
| Type-ahead by first letters, 1s timeout, repeated letter steps through matches, Space extends a word; a prefix that matches nothing starts over from the new letter, and a letter that matches nothing is dropped | Same (case- and diacritic-insensitive) |
| Key equivalents shown in Mac glyphs, and they work while the menu is open, including ⌘/⌃ with arrows, Home, End, Page Up and Page Down | Same |
| Hover highlights; leaving the menu clears it | Same; a keyboard highlight survives the pointer moving outside, and the gap between a menu and its submenu counts as inside |
| Pop-up buttons open on mouse-down; press-drag-release chooses; a click leaves the menu open; drag off and release closes | Same with `KzMenuButton`. `.kzMenu(isPresented:)` opens on the button's mouse-up |
| In an open menu, pressing an item, dragging off the menu and releasing closes it | Same |
| Context menus: right-click or Control-click, press-drag-release | Same |
| Right-clicking a list row outlines it without changing the selection, and the menu acts on that row; anywhere in the row counts | Same with `.kzContextMenu(isPresented:)` on each row plus `.kzContextTarget(_:)`; a click in the row's side gutter goes to the row's menu |
| Pop-up menus put the checked item over the button | Same (`.select`) |
| Submenus: hover opens, → opens and highlights the first item, ← closes, the pointer can cross other items on the way | Same, with a 120 ms open delay and a 250 ms grace period instead of a "safe triangle". A submenu that hasn't opened yet doesn't open once the pointer has left its item |
| The click outside closes the menu and goes nowhere else | Same inside Kudzu; in another app the click still lands there |
| Closes on app switch, window move, resize, minimise, close, Space change, another window taking key | Same |
| Selection blinks once, the menu fades, then the action runs | Same; the action runs after the menu has closed and focus is back. No fade under Reduce Motion. If the menu is dismissed during the blink (a click outside, an app switch), the item doesn't run and focus isn't pulled back |
| Focus stays in (returns to) the view that had it | Same |
| VoiceOver: menu and menu-item roles, opened/closed notifications, checked state, shortcut | Roles, `AXMenuOpened`/`AXMenuClosed`, selection changes and press work. Check state is the item's value ("checked") and the shortcut is its help text, because AppKit's modern accessibility API has no setter for `AXMenuItemMarkChar`/`AXMenuItemCmdChar` |

Not matched:

- **Scrolling.** A menu taller than the screen scrolls with the wheel and follows the keyboard highlight, but has no scroll arrows that scroll on hover.
- **Text views.** Right-clicks on text fields and editors keep the native text menu (Cut, Copy, Paste, spelling, Services); `.kzContextMenu` doesn't replace it.
- **The menu bar extra** stays a system menu; macOS doesn't allow restyling it.
- **While a menu is open** every key goes to it, as in NSMenu, so app shortcuts don't fire until it closes (unless they're items in the menu).
- **Accessibility zoom / Full Keyboard Access** follow the highlight only through VoiceOver's notifications, not through the system menu tracking they know.
- **Right-to-left.** Submenus and `.leading`/`.trailing` placements use physical left and right.

### Popovers

`.kzPopover` is built on `KzFloatingPanel` rather than `NSPopover`: the design has no arrow and `NSPopover` can't take Kudzu glass without private API. It opens on `edge` of the view (flipping when there isn't room), becomes key without activating Kudzu, closes on Esc, and gives focus back on close. `.transient` (the default) also closes on a click outside, when another window takes key, or when Kudzu is deactivated; clicking the view that opened it closes it rather than reopening it. Its content updates with the state it's built from, and it resizes to fit.

### FloatingPanel

`KzFloatingPanel` is what the search palette and the form at the caret should adopt:

```swift
// Search palette: fixed size, centred in the upper part of the screen, closes when it loses key.
let panel = KzFloatingPanel(sizing: .fixed(CGSize(width: 760, height: 460))) { SearchPaletteView(…) }
panel.keyHandler = { event in controller.handle(event) }   // before the search field's editor
panel.onClose = { _ in model.isActive = false }
panel.present(.centered(topFraction: 1.0 / 6))

// Form at the caret: fits its content, below the caret (above when there's no room), stays until Esc or submit.
let form = KzFloatingPanel(behavior: .modeless) { FormPanelView(…) }
form.onClose = { reason in if reason == .escape { finish(nil) } }
form.present(location.source == .mouse ? .centered() : .edge(.bottom, anchor: location.rect, gap: 6))
```

`Behavior` options: `.closesOnEscape` (intercepted before the focused view, except during IME composition), `.closesOnResignKey` (ignores its own child windows, so a menu or popover opened from it doesn't close it), `.closesOnOutsideClick` (the transparent shadow margin), `.restoresFocus`. `.transient` has all four; `.modeless` has Esc and focus.

## Tokens

`Sources/KudzuUI/Tokens/Tokens.generated.swift` is generated from `design/kudzu/project/tokens.json`. Never edit it by hand.

```sh
python3 scripts/design/generate-tokens.py          # regenerate (from the repo root)
python3 scripts/design/generate-tokens.py --check  # exit 1 if stale
```

The generator uses only the standard library and is deterministic. It:

- resolves aliases (`{vine-ink}`)
- reads 8-digit hex alpha
- parses CSS shadow lists into `KzShadowLayer`s
- emits each usage note as a doc comment

The test `Tokens.generated.swift is up to date` fails if `tokens.json` or the generator changed without regenerating. The Increase Contrast substitutions live in the generator, because `tokens.json` doesn't specify them.

## Fonts

The bundle includes Geist and Geist Mono v1.9.0 (OFL, `Resources/Fonts/OFL.txt`) in static OTF at weights 400, 500 and 600, from the official `vercel/geist-font` release.

- **Registration.** `KzFonts.register()` registers the fonts for the process with `CTFontManagerRegisterFontsForURL`. The Kudzu app calls it in `KudzuApp.init`, and type-style APIs call it lazily too.
- **Fallback.** If a face fails to register, `KzFonts.status` is `.failed(missing:)` and styles fall back to the system font at the same size and weight.
- **Tabular figures.** `stat` uses tabular figures (`monospacedDigit` / `kMonospacedNumbersSelector`).

## Icons

`KzIconName` contains two groups:

- The design system's 33 glyphs, ported verbatim from `bundle.js` `ICONS`. A test diffs them against `bundle.js`.
- 17 additions drawn in the same style: 16-unit grid, 1.5 stroke, round caps and joins.

`KzIcon` renders them via an SVG path parser (`KzSVGPath`, which handles every path command including arcs), and the stroke scales with the size.

To add a glyph:

1. Add a case to `KzIconName` under "Additions", with `parts` and `accessibilityLabel`.
2. Run `python3 scripts/design/export-icon-additions.py`. It writes `design/kudzu/additions/icons/<name>.svg` and a paste-ready `ICONS-additions.js.txt` for the design system.

A test checks that the SVGs match the Swift source.

### Menu bar

`KzMenuBarIcon` draws the bundled SVGs into 18pt template `NSImage`s. Use it in the menu bar label as `Image(nsImage: KzMenuBarIcon.paused.image())`.

| State | Source |
| --- | --- |
| `.active` | `kudzu-menubar.svg` (design system) |
| `.paused` | `kudzu-menubar-paused.svg` (design system) |
| `.permissionsMissing` | `design/kudzu/additions/kudzu-menubar-permissions.svg`: outlined leaf and a knocked-out warning triangle |
| `.secureInput` | `design/kudzu/additions/kudzu-menubar-secure-input.svg`: outlined leaf and a small padlock |

A test keeps the bundled copies identical to the design sources.

### SF Symbol mapping

These are the symbols the app uses today, and what replaces each one:

| SF Symbol | `KzIconName` |
| --- | --- |
| `exclamationmark.triangle`, `.fill` | `.warning` |
| `exclamationmark.octagon.fill` | `.error` |
| `exclamationmark.circle.fill` | `.alert` |
| `checkmark.circle`, `.fill` | `.checkCircle` |
| `checkmark.seal.fill` | `.checkCircle` |
| `checkmark` | `.check` |
| `minus.circle.fill` | `.minusCircle` |
| `shield.lefthalf.filled` | `.shield` |
| `lock.fill` | `.lock` |
| `text.cursor`, `character.cursor.ibeam` | `.cursor` |
| `line.3.horizontal` | `.grip` |
| `doc.on.doc` (copy) | `.copy` |
| `text.insert` | `.bolt` (expansions stat) |
| `text.badge.plus` | `.textPlus` (or `.compose` for "new snippet") |
| `text.badge.minus` | `.pause` (the menu bar uses `KzMenuBarIcon.paused`) |
| `list.bullet.rectangle` | `.list` |
| `gearshape` | `.sliders` |
| `folder` | `.folder` |
| `folder.badge.plus` | `.folderPlus` |
| `folder.badge.minus` | `.folderMinus` |
| `folder.badge.gearshape` | `.folder` |
| `star` / `star.fill` | `.star` / `.starFill` |
| `clock` | `.clock` |
| `clock.arrow.circlepath` | `.history` |
| `chart.bar` | `.chart` |
| `keyboard` | `.keyboard` |
| `ladybug` | `.bug` |
| `icloud` | `.cloud` |
| `info.circle` | `.info` |
| `timer` | `.timer` |
| `tray.full` | `.stack` (All snippets) |
| `square.stack` | `.stack` |
| `square.and.pencil` | `.compose` |
| `arrow.up.arrow.down` | `.sort` |
| `arrow.up.arrow.down.square` | `.tray` (Import & export) |
| `arrow.turn.down.right` | `.inherit` |
| `app.badge.checkmark` | `.app` |
| `magnifyingglass`, `.circle`, `text.magnifyingglass` | `.search` |
| `xmark` | `.x` |
| `chevron.right` | `.chevronRight` |
| `ellipsis.circle` | `.more` |
| `plus` | `.plus` |
| (new) `play`, `pause`, `external` | `.play`, `.pause`, `.external` |

## Gallery

`KudzuGallery` is an app target in `project.yml`, with sources in `Gallery/`. It has a sidebar of every page in `Catalog.pages` and a System / Light / Dark switch, which sets `NSApp.appearance` so dynamic colours follow it.

```sh
xcodegen generate
xcodebuild -project Kudzu.xcodeproj -scheme KudzuGallery -derivedDataPath build/DerivedData build
open build/DerivedData/Build/Products/Debug/KudzuGallery.app
```

## Snapshots

This is how to review visual changes without a screen. PNGs go to `Packages/KudzuUI/Snapshots/`, which is git-ignored.

- **Unit snapshots.**
  - The test helper is `snapshot("name", width:) { view }` in `Tests/KudzuUITests/Support/Snapshots.swift`. It renders with `ImageRenderer` in Light and Dark at 2x.
  - `swift test` writes them, including `foundations-*.png` for icons, type, colours, shadows and glass, interaction states and menu bar glyphs.
- **Gallery pages.** These render offscreen and the app exits without opening a window:

  ```sh
  build/DerivedData/Build/Products/Debug/KudzuGallery.app/Contents/MacOS/KudzuGallery \
    --snapshot Packages/KudzuUI/Snapshots/gallery [--page colors --page button]
  ```

  Output is `<page-id>-light@2x.png` and `<page-id>-dark@2x.png`. Run it from the repo root, or pass an absolute directory.
- **Limits.**
  - `NSVisualEffectView` can't render offscreen, so `kzGlass` draws only its tint when `\.kzSnapshotRendering` is set. `KzSnapshot` sets it for you.
  - Hover, press and focus can't happen offscreen. Force them with `.kzPreviewInteraction(…)`.
