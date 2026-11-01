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
   - menus and popovers: `.strong`, `.withinWindow`, `shadow: .float`
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
2. **Gallery page.** `Gallery/Pages/<Name>Page.swift` already exists as a placeholder and is already listed in `Gallery/Catalog.swift`. Replace its `body` with a `GalleryPageLayout` showing every variant and state. Use `.kzPreviewInteraction(hovered:pressed:focused:)` to show the hover, pressed and focus states statically.
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

A brand-new page that isn't in the design system needs two steps:

1. Create `Gallery/Pages/<Name>Page.swift`, conforming to `GalleryPage` (`id`, `title`, `section`, `icon`).
2. Add one line, `<Name>Page.self,`, to `Catalog.pages`.

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

`.kzSelectionList()` turns off the system accent highlight (a small `NSTableView` bridge; if the table isn't found the list keeps the system highlight) so `vine-soft` is the only selection.

**The window.** `KzAppShell(mode: .window)` applies `KzWindowChrome` to its window: hidden title, an *empty unified toolbar* (which makes AppKit build the 52pt titlebar and centre the traffic lights on y = 26, the middle of a `size-titlebar` header), full-size content and a clear background. Use `mode: .embedded` for galleries and snapshots; it leaves the window alone and draws mock lights. Build pane headers with `KzListHeader` / `KzPaneHeader`. Their `kzWindowDragArea()` lets the window be dragged by the header.

**The editor.** The app's `NSTextView` editor adopts the look through `KzEditorStyle` (content attributes, inset, `configure(_:)`) and `KzEditorTag` / `KzEditorTagCell` (the inline tag). `KzEditorFrame` is the chrome around it.

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
