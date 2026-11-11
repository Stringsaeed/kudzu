# Component parity matrix (S06, S07)

Every shadcn primitive in the Tauri app's `components/ui/`, the rune-kit component (or Kudzu composition) that draws it in the new app, how it measures against the reference, and a verdict. S07 adds Kudzu's shared widgets (`crates/kudzu-ui/src/widgets/`) and the D6 additions. Plan: §1.4 (the mapping and the "only door to local component code"), §3 gap 17, §6 D6. **D6 as widened by the owner after S06:** exact metrics for buttons, inputs and switches, and also the select trigger (32 pt), the menu, dialog and toast radii (menus 10, dialogs 14–18, toasts 14), the hover and pressed shadows on raised controls with the switch knob's 1.2× stretch while held, and the checkbox's checked colours (primary). rune-kit's own metrics are accepted everywhere else.

Wrappers and decorations link their upstream issue: **Rune #573** ("rune-kit `<Component>`: take `<metric>` from `Kit`", filed 2026-11-11 from this matrix) for metrics, **Rune #360** (commented) for shadows and child geometry that follow hover and press.

## How the numbers were produced

- **Rune side:** `cargo xtask parity` (made fail-closed in 60d99d80) renders every `gallery` scene headless in both themes with the system font (SF Pro, as S02 captured), reads the `--tree`, and measures each sample by its key (`xtask/src/parity/`, `crates/kudzu-ui/src/gallery/components/`, `crates/kudzu-ui/src/gallery/widgets/`). A gallery scene with no keyed sample fails.
  - `parity:<window>:<scene>:<role>:<occurrence>:<name>` rebuilds a reference landmark (same label, icon, variant, size) and is gated against it: size within 2 pt, background and text colour per 8-bit channel (premultiplied, because `metrics.json` reads colours back through an 8-bit premultiplied canvas), padding as the content's inset, and the reference landmarks inside it by offset and size.
  - `spec:<what>:<w>x<h>` is gated against the size the Tauri classes give (`_` for a side the content decides).
  - `info:<what>` is reported, not gated (rune-kit's own metrics, D6, or a look only a side-by-side can judge).
  - Font size and weight, radius, border, shadow, focus ring and the switch knob are not in Rune's `--tree` (nor borders in `metrics.json`): every gated row lists them as **needs side-by-side review**, judged in `sheet.html`.
- **Reference side:** `docs/migration/reference/metrics.json` (S02) and the classes in `components/ui/*.tsx` and `components/app/*.tsx`. The side-by-side sheet (`target/parity/sheet.html`) crops each `parity:` sample and its reference landmark from the PNGs at `$KUDZU_REFERENCE_DIR` (D12); without them it is skipped with a message.
- **Run on 2026-11-11 (S07 review fixes and D21):** 412 rows: 284 gallery rows in both themes, **246 pass, 38 info, 0 fail** (458 checks pass, 540 need side-by-side review, 0 fail); the 128 screen rows are reference states marked not yet built. Re-run after any change to `theme/kit.rs`, `widgets/kit_ext/` or `widgets/`.

Measured values are in points. `Δ` is the largest difference in width or height against the reference landmark.

## The Kit mapping (`crates/kudzu-ui/src/theme/kit.rs`)

| rune-kit role | Kudzu token | Why |
|---|---|---|
| `page`, `bg_base`, `bg_deep` | `background` | the window |
| `layer_01` | `popover` | menus, popovers, dialogs, toasts paint `bg-popover` |
| `layer_02` / `layer_03` / `layer_04` | `muted` / `foreground/6%` / `input` | badge and keycap fills, switch track |
| `bg_contrast`, `bg_icon_button_contrast`, `text_contrast` | `primary`, `primary-foreground` | shadcn's `default` button |
| `bg_button_neutral` | `surface-raised` | shadcn's `outline` button |
| `text_muted`, `text_faint` | `muted-foreground` | placeholders, menu shortcuts, select hints |
| `hover` | `muted` (light), `muted/50` (dark) | `ghost` and menu-item hover |
| `pressed` | `muted` | `aria-expanded:bg-muted` (rune-kit paints an open trigger as pressed); shadcn sinks a press instead of tinting it |
| `border_focus`, `border_strong` | `ring` | focus rings |
| `surface_float`, `text_invert_strong` | `foreground`, `background` | tooltips |
| `scrim` | black 10% | dialog overlay |
| `danger` / `success` / `warning` / `info` | `destructive` / `success` / `warning` / `iris`, at 10% (20% dark) for fills | |
| `elevation.button_neutral`, `button_contrast` | `raised` | |
| `elevation.input()` | `well` | fields |
| `elevation.overlay` / `raised` / `knob` | `panel` / `card` / `knob` | |
| `elevation.floating`, `md` | ring `foreground/10%` + Tailwind `shadow-md` | popover, menu, select |
| `ty.ui` / `ui_medium` / `body` / `body_medium` / `text_xsmall_medium` | body 13/20, 13 medium, `text-sm` 14/20, 14 medium, `text-xs` 12/16 medium | |
| `radius.xs/sm/md/lg/xl` | 2 / 4 / 8 / 10 / 14 | checkbox 4, `rounded-md` 8, `rounded-lg` 10, `rounded-xl` 14 |
| `motion.control` / `overlay` / `disclosure` | soft spring 160 ms / pop spring 160 ms / soft spring 240 ms | |
| `glyph.*` | Kudzu's Lucide `check`, `chevron-*`, `x`, `copy`, `search`, `lock`, `eye-off`; rune-kit's for the rest | |
| `blend_over` | none | every Kudzu window blends in sRGB, as the CSS was written |

## Primitives

Verdicts: **match** (rune-kit with the Kit, within 2 pt), **themed** (rune-kit with the Kit; its own metrics accepted by D6, or recoloured through a copy of the Kit), **decorated** (rune-kit's component built, then given Kudzu's metrics as overrides), **wrapper** (built from Rune core, tagged `RUNE-GAP`), **composition** (no component needed, or a Kudzu widget).

### Exact-metric primitives (D6)

| shadcn | Backed by | Tauri (classes / reference) | Rune measured | Verdict |
|---|---|---|---|---|
| button | `rune_kit::Button` via `kit_ext::button` | h 24/28/32/36 (`xs`/`sm`/default/`lg`), `px-2`/`px-2.5` + 1 pt border, radius 8 (`xs`, `sm`) or 10, labels 12 / 12.8 / 14 medium, icons 12 / 14 / 16; variants default (primary, `sheen-strong`, `raised`), outline (`surface-raised`, `sheen`, `raised`), secondary, ghost, destructive (`destructive/10`). Reference: “Skip for now” 103.86×32, “Continue →” 103.56×32, “Choose file… for Raycast” 101.42×28, “Sync now” 98.78×28, “Clear” 54.16×28, “More actions” 28×28, “Hide sidebar” 32×32 (expanded, `bg-muted`) | 104×32 (Δ 0.14), 104×32 (0.44), 101.5×28 (0.08), 99×28 (0.22), 54.5×28 (0.34), 28×28 (0), 32×32 (0); fills equal in both themes. Every size and state `spec` row passes. | **decorated** |
| button (icon sizes) | same | `icon-xs` 24, `icon-sm` 28, `icon` 32 squares, glyph 12 / 16 / 16 | 24, 28, 32 | **decorated** |
| button: hover and pressed depth (S07, D21) | `kit_ext::button` + `kit_ext::press` | `shadow-raised` → `hover:shadow-raised-hover` → `active:shadow-pressed active:bg-none` on default, outline, secondary; `active:shadow-pressed` on ghost; Space shows `:active` until it comes up | The face (fill, depth, sheen) is drawn by layers inside the button: rest and hover crossfade by hover, the pressed layer follows a live press value the button's drag handler sets (no rebuild). Space on the focused button (D21) holds the pressed look from its key-down, across the rebuild its press causes, to its key-up (`on_key_up`); tested. Forced states: `spec:button/{default,secondary,ghost}/sm/{hover,pressed}` 28 tall, pass; shadows are review items | **decorated** (`RUNE-GAP(#360)`) |
| toggle | `kit_ext::button` ghost `icon-sm`; on: `surface-raised` + `soft` depth | h 28 square (`sm`), `data-[state=on]:bg-surface-raised shadow-soft` | 28×28, on-fill white (light) | **decorated** (drawn by S07's `widgets::toggle_button`) |
| switch | `kit_ext::switch` | 32×18.4, knob 16 (default); 24×14, knob 12 (`sm`); 1 pt inset, capsule; track `iris` / `input` (`input/80` dark), `well` / inset-shadow; knob `background` (`white` checked, `foreground/90` unchecked in dark). Reference: “Open at login” 32×18.39, “Pause all snippets” 32×18.39, “Enabled” 24×14 | 32×18.4 (Δ 0.01), 32×18.4 (0.01), 24×14 (0); fills equal in both themes | **wrapper** |
| switch: knob stretch while held (S07, D21) | `kit_ext::switch` + `kit_ext::press` | `group-active/switch:scale-x-[1.2]` from the edge facing the travel | The knob's face has a live frame (`live_rect`) the switch's drag handler stretches to 1.2× on Down and restores on Up, on the soft spring at `FAST`; Space (D21) toggles on key-down and keeps the knob stretched, from the new edge, until its key-up (tested). A forced `State::Pressed` draws it statically (`spec:switch/*-pressed`, pass). Knob shape is a review item | **wrapper** (`RUNE-GAP(#360)`) |
| input | `rune_kit::TextInput` via `kit_ext::input` | h 32, 14/20, 11 pt either side, radius 10, `foreground/2.5%` (`black/15` dark) + `well`; focus: ring border + 3 pt `ring/50`. Reference: “Trigger prefix” 80×32 | 80×32 (Δ 0), fill equal in both themes; `spec` rows for empty, filled, focus, invalid, disabled pass. S07: the text starts 11 in (S06 left rune-kit's 6 pt gap after its 2 pt spacer in, so it started 17 in; padding now passes) | **decorated** |
| input-group (S07: own scene) | `kit_ext::input` with `leading_icon`, `icon_at`, a trailing `widgets::shortcut` hint or button addon | The filters (`snippet-list.tsx`, `clipboard-view.tsx`): h 32, a 14 pt faint icon at `left-2.5`, `pl-8 pr-14`, 13 pt, `bg-foreground/[0.03]` in both themes. Reference: “Filter snippets” 293.59×32, “Search clipboard history” 329×32 | 293.59×32 and 329×32, padding left 30 (within 2 of 32), right inset ≥ 56, fills equal in both themes; icon-only 180×32, button addon 240×32 | **decorated** |
| textarea | `kit_ext::textarea` | `min-h-16` (64), `px-2.5 py-2` + border, 14/20, radius 10, the input's surface. Reference: “Mac Linux” (chip choice form, `min-h-20 font-mono`) 296×80 | 296×80 (Δ 0), fill equal in both themes; empty 296×64 | **wrapper** |
| select (S07, D21) | `rune_kit::Select` (`Field`) via `kit_ext::select` | trigger `h-8`, `pl-2.5 pr-2` + border, `rounded-lg`, `bg-surface-raised shadow-raised sheen`, `hover:shadow-raised-hover`, `data-[state=open]:shadow-pressed`, `focus-visible:border-ring ring-3 ring-ring/50`; content `rounded-lg`. Reference: “Keep history for” 128×32 | 128×32, fill equal in both themes, text inset 11, 14 pt text (the kit copy's `ui_compact`); hover, focus and open 128×32. Keyboard focus draws the 3 pt `ring/50` halo around rune-kit's 1 pt ring-coloured border (D21); a forced focus draws both. Space opens it, and open shows the pressed depth. Built with a kit whose base background is `surface-raised` and whose field depth is empty; the depth is `kit_ext::press`'s layers; the list's radius is set on what `Select::build` pushed | **decorated** (`RUNE-GAP(#573)`, `#360` for the depth) |
| dropdown-menu, context-menu (S07: radius) | `rune_kit::Menu` via `kit_ext::menu` | `rounded-lg` (10), `p-1`, items `px-1.5 py-1` text-sm (28) rounded-md, ring + `shadow-md` | radius 10 (set on the surfaces `Menu::build` pushes); items 28, list padding 2, fill `popover`, `elevation.floating` stay rune-kit's | **decorated** (`RUNE-GAP(#573)`) |
| dialog, alert-dialog (S07) | `kit_ext::dialog` on rune-kit's modal layer | shadcn: `rounded-xl` (14), `p-4 gap-4`, `shadow-panel`, `ring-foreground/10`, `sm:max-w-sm` (384); Kudzu's own dialogs: `rounded-2xl` (18), `shadow-float`, no ring, their own widths (group settings 520) | `spec:dialog/kudzu:520x_` and `spec:alert-dialog:384x_` pass; radius 18 / 14, padding 16, gap 16, depth `float` / `panel` + ring are review items. rune-kit's panel is a child of the layer it pushes, with fixed 6 pt corners, so this is built from Rune core with rune-kit's structure (scrim, centred panel named by its title, header and close button, modal focus) | **wrapper** (`RUNE-GAP(#573)`) |
| sonner (S07, D21) | `rune_kit::Toast` via `kit_ext::toast` | `--border-radius: calc(var(--radius) * 1.4)` (14), `--width` 356, `shadow-float`; the action inline at the end, a dark Sonner button | radius 14 and width 356 on the built toast (`spec:sonner/toast:356x_`, pass); padding 12 and rune-kit's layout, with actions under the text, stay rune-kit's | **decorated** (`RUNE-GAP(#573)`) |
| checkbox (S07, D21) | `kit_ext::checkbox` | 16×16, radius 4, `border-input shadow-well` off; checked `bg-primary border-primary text-primary-foreground shadow-raised sheen-strong`; focus `border-ring ring-3 ring-ring/50`; the row `gap-2.5 text-[13px]` | built from Rune core (rune-kit's box is an inner element of its row, so its depth and sheen could not be decorated on): the box, primary with the `raised` depth, the strong sheen and a 14 pt check when on, the well and the input border when off, the halo at focus; the row is the control, named by its label. `spec:checkbox/{off,on,focus-off,focus-on,disabled}` 16×16 and the labelled row 20 tall, pass | **wrapper** (`RUNE-GAP(#573)`) |

### rune-kit's own metrics (D6)

| shadcn | Backed by | Tauri (classes) | Rune measured / from rune-kit source | Verdict |
|---|---|---|---|---|
| label | `rune_kit::Field` label / `text` | `text-sm leading-none font-medium`, `gap-2` | Field label in `text_xsmall_medium` (12/16 medium) | **themed** |
| popover | `rune_kit::Popover` | `w-72` (288), `p-2.5`, `gap-2.5`, radius 10, `popover` + ring + `shadow-md` | 310×112 for the sample content, padding 12, fill `popover`, `elevation.floating` | **themed** |
| tooltip | `rune_kit::Tooltip`, `TooltipAppearance::Standard` | `px-3 py-1.5`, radius `md` (8), `text-xs`, `bg-foreground text-background`, `shadow-float` | 156.5×24, padding 8/2, radius `kit.radius.sm` (4), fill `foreground`, ink `background`, 1 pt `border` | **themed** |
| command | `rune_kit::PopupList` under `kit_ext::input` (input group) | input `h-8`, items `px-2 py-1.5` text-sm, `data-selected:bg-muted` | rows 28 tall, active row `muted`; the field sample `spec:command/field:320x32` passes (S07 keyed it) | **themed** (the same list drives the editor's slash menu) |
| badge | `rune_kit::Badge` | h 20 (`h-5`), `px-2`, radius `4xl` (pill), `text-xs` medium | h 16, radius from rune-kit, `small_medium` | **themed** |
| separator | `rune_kit::Divider` | 1 pt, `bg-border` | 240×1, 1×24, fill `border` | **match** |
| kbd | S07's `widgets::shortcut` (`RUNE-GAP(#573)`) | h 20 (`h-5`), `min-w-5`, `px-1`, radius 5, 11 medium tabular, `foreground/6%`, `shadow-key`; the app's `sm` caps 18 and 10.5 | caps built from Rune core at 18 / 20 with Kudzu's padding, radius, type and `key` depth (rune-kit's `Keybind` is 14 with 2 pt corners); `spec:shortcut/*` pass | **composition** |
| item | plain `row()` | `rounded-lg border px-2.5 py-2 gap-2`, title over description | 360×62 sample | **composition** |
| breadcrumb | S07's `widgets::breadcrumbs` | 13 pt muted crumbs, `gap-2` around a 14 pt faint chevron, medium foreground page; past four: first, a “Show parent groups” menu, last two. Reference: toolbar trail 891×20 with “Snippets” 53.17, “Work” 31.34, “Email” 32.36; “Settings” 50.08; clipboard item trail 126.69×20 with “History” 43.45 | crumbs within 2 pt, colours equal (`parity:` rows); trails 891×20, 126.69×20 (`spec:` rows); a tree test pins the 8 + 14 + 8 spacing | **composition** |

### In `components/ui` but never rendered

No component under `components/` or `app/` imports these, so they have no scene: **collapsible, radio-group, scroll-area, slider, tabs, toggle-group**. The app's own segmented control (`components/app/segmented.tsx`) and the Insights range input are S07's `widgets::selection_pill::Segmented` and `widgets::slider`, with scenes below. The `.scroll()` + `Kit::scroll_bar` mapping in §1.4 stands for the screens that scroll.

## Kudzu compositions (S07)

Each has a gallery scene (`gallery/widgets/`). Where the reference has a landmark the sample is a `parity:` row; otherwise `spec:` gives the Tauri classes' size.

| Widget | Tauri source | Samples | Result |
|---|---|---|---|
| `shortcut` | `shortcut.tsx`, `kbd.tsx` | `spec:shortcut/{sm,md,word,ghost,accent,on-accent,spoken}` | pass; spoken in words in the tree, for combos and for recorded accelerators and ready-made labels (`⌃ ⌥ Space` is “Control Option Space”; tests) |
| `trigger_chip` | `trigger-chip.tsx` | `spec:trigger-chip/{active,muted,space,truncated}` 20 tall | pass; `␣` for spaces (test) |
| `selection_pill` (`Segmented`) | `selection-pill.tsx`, `segmented.tsx` | `parity:` radios Dark, Light, System (settings-general) and Automatic, Type, Paste (settings-expansion); `spec:segmented/{sm,none}` | pass in both themes (each capture shows its own theme chosen); slide checked by the motion gate |
| `slider` (`RUNE-GAP(#149)`) | `insights.tsx` range, `slider.tsx` look | `spec:slider/{rest,active,min,max}` 192×12 | pass; role, value and keyboard steps (test) |
| `stepper` | `tag-tools.tsx` `Stepper` | `parity:` “Times” (chip-key), “Milliseconds” (chip-delay), “Copies back” (chip-clipboard), 296×32 | pass |
| `toggle_button` | `toggle.tsx` | the `toggle` scene, `spec:toggle/sm/*` 28×28 | pass |
| `key_hints` | `key-hints.ts`, `key-hint` utility | `spec:key-hint/{enter-cap,copy,held}`, `spec:key-hints/footer:640x40` | pass; a hint holds from its key's press to its real key-up (`release_on_key_up` on the window root, D21; tested) |
| `formatted_text` | `formatted-text.tsx` | `info:formatted-text/preview` | review; the cursor is a 2 pt iris bar (D21): a zero-width space whose span background is padded 1 pt each side, about 1.34 em tall (Rune sizes a span background on the cap height) where the web's is 1.1 em |
| `secret_text` | `clipboard/secret.tsx` | `info:` rows, `spec:secret-text/{cover,revealed}:296x112`, `spec:secret-text/{long-cover,long-revealed}` | pass; the cover is one focusable element that reveals on focus or hover and conceals on blur (`Reveal`, fed by the window's poll); it keeps the hidden text's measured height (D21), so revealing moves nothing; no content in the tree until revealed (tests) |
| `empty_state` | `EditorEmpty` | `spec:empty-state/glyph:48x48`, `info:empty-state/pane` | pass / review |
| `settings_card` | `settings.tsx` `Group`, `Row` | `parity:main:settings-general:region:1:Quick search` (616 wide, heading at 4,0, the recorder button at 456,43), `info:settings-card/switches` | pass in both themes |
| `wordmark` | `wordmark.tsx`, `design/wordmark/` | `spec:wordmark/{full,small}` | pass |

## Decorations and wrappers

| Module | What rune-kit could not take from `Kit` | How it is reached | Unreachable, recorded | Upstream |
|---|---|---|---|---|
| `kit_ext::button` (decorated) | Height, padding and radius per size (rune-kit sizes push buttons as AppKit's `NSButton`: 16/20/24/28/36, its own padding); label type per size; icon size per size; fills for `secondary` and `destructive`; the sheen; the depth under hover and press | overrides on the built `El`: `h`, `px`, `radius`, `gap`; `label_with` draws the icon and label in Kudzu's type; `fill`, `press_scale(0.97)`, `focus_ring(3, 0, ring/50)`; built with a kit without button shadows, its face drawn by `kit_ext::press` layers | dark outline hover is `color-mix(surface-raised, white 4%)` approximated as white 4% over it | Rune #573 (metrics), #360 (depth) |
| `kit_ext::input` (decorated) | Padding (rune-kit: a 2 pt spacer and a 6 pt gap before the text, 8 after), radius (`radius.md`), the hover tint, the focus look (1 pt inside ring) | `height(32)`, `text_style(14/20)` through the builder; `pl(3)`, `pr`, `radius`, `hover(transparent)`, `fill`, `outline(3, 0, ring/50)` + an inset ring on the built `El`; `icon_at` for the filters' 10 pt icon | none for the matched metrics | Rune #573 |
| `kit_ext::switch` (wrapper, `RUNE-GAP(#573)`) | Track and knob size: rune-kit draws an `NSSwitch` (36×16 and up, a 21 pt pill knob) or the OpenCode web switch (24×16, 4 pt corners); both are inner elements | built from Rune core: track `el()` with the knob as an absolute child on the soft spring; the knob's face stretches live while held | the `after:-inset-x-3 -inset-y-2` enlarged hit area is not built | Rune #573 (sizes), #360 (stretch) |
| `kit_ext::textarea` (wrapper, `RUNE-GAP(#573)`) | The text style (rune-kit: `ty.ui` on a 1.35 line) and padding (8); its editor is an inner element | built from Rune core's `text_input` in a padded column with the input's surface | none for the matched metrics | Rune #573 |
| `kit_ext::select` (decorated, S07) | Trigger height (28), padding, radius (6), fill (base), depth (`input`), hover tint, focus look; the list's 6 pt radius | a kit copy with `bg_base = surface-raised` and no field depth; `h`, `pl(3)`, `pr(7)`, `radius(10)`, `fill`, `hover(transparent)`, `focus_ring(3, 0, ring/50)` on the trigger; `kit_ext::press` layers; radius 10 on the pushed list | none for the matched metrics | Rune #573, #360 |
| `kit_ext::menu` (decorated, S07) | The surface's 6 pt radius | radius 10 on every element `Menu::build` pushes (the scrim and the anchor point are transparent) | none | Rune #573 |
| `kit_ext::dialog` (wrapper, S07) | The panel's 6 pt radius, its padding and depth (the panel is a child of the pushed layer) | built from Rune core with rune-kit's structure; radius 14 (shadcn) or 18 (`kudzu()`), `p-4 gap-4`, `panel` + ring or `float` | the `bg-muted/50` footer band (no dialog in the app needs it before S55) | Rune #573 |
| `kit_ext::toast` (decorated, S07) | The 8 pt radius, the 320 width | radius 14, width 356 on the built toast | the inline end action (Sonner) | Rune #573 |
| `kit_ext::checkbox` (wrapper, D21) | The checked fill, border, mark, depth and sheen (an inner element) | built from Rune core | none | Rune #573 |
| `kit_ext::press` (S07, D21) | Shadows and child geometry per hover and press | hover: two face layers crossfaded by `group_opacity`; pointer press: a `Live` value set by the control's drag handler (`Tx::live_only`), bound to the pressed layer's opacity or the knob's `live_rect`; Space: the control's `on_key` marks it held by the keyboard, a control rebuilt while held starts pressed, its `on_key_up` lets go | a pointer press that sees a rebuild while held restarts at rest (the look lets go early); controls are told apart by kind and accessible name, so two same-named buttons on one screen would both show a held Space | Rune #360 |
| `widgets::shortcut` (S07) | `Keybind`'s cap height, padding, radius, type | caps from Rune core | none | Rune #573 |

## Known differences outside the gate

- Breadcrumb trails are gated by size, their crumbs by `parity:` rows: `metrics.json` records the `nav` element's inherited colour (foreground), while the runner compares the ink of the first text drawn inside it, which is a muted crumb in both apps. Reported to the orchestrator as a runner issue (S07).
- Overlay radii now match for menus (10), dialogs (14 / 18), toasts (14) and the select list (10). The tooltip keeps rune-kit's `radius.sm` (shadcn `rounded-md`, 8); popovers keep 6 (shadcn 10): D6 does not list them.
- Focus rings: shadcn draws a ring-coloured border plus a 3 pt `ring/50` halo; the decorated button, input, select, checkbox and the wrappers draw the halo as a focus ring and, for a forced focus state, the border too. rune-kit's other components keep their own ring.
- Press: shadcn scales a pressed control to 0.97 and swaps to `shadow-pressed`; the decorated button does both, for the pointer and for Space until its key-up (D21). Return passes a focused button by in Rune, as in AppKit, so it shows no press.

- The toast's action sits under the text (rune-kit's layout) rather than inline at the end as a dark Sonner button; the dialog wrapper has no `bg-muted/50` footer band yet (S55 builds the group dialog).

## Screens (S57 fill-in form)

`form` / `demo` (`/form?demo`, 440 × content). `cargo xtask parity --window form --scene demo`: 10 rows, 0 fail (52 properties need side-by-side review). Form fields use Tauri's `h-9` (36); S07 gallery inputs stay 32 (`h-8`).

Intentional differences, not closed:

- **First-load preview shimmer.** Tauri animates a `background-position` sweep. Rune `Beat` has no background-position, so the first-load well is a static 14×180 bar (`RUNE-GAP` Beat). The demo scene warms the 50 ms preview so the captured state shows the Name field and the meeting follow-up text.

## Quick Search (S56)

`cargo xtask parity --window palette` passes both themes of `search` and `no-match` (28 landmark rows). These stay:

- The return glyph in the footer cap is 16 pt, not the reference `size-3` (12). The landmark unions icon bounds, and a 12 pt glyph centred in the 18 pt cap starts 3 pt inside the 16 pt padding, which fails. 16 pt sits on that padding.
- The empty search field's placeholder is not a text node in `--tree`, so the `search` combobox ink stays a review row. It is painted the capture's computed foreground. The stylesheet says `placeholder:text-faint`.
- The card is the captured 12 pt inset with an opaque popover fill. The running window's full-bleed vibrancy (`vibrant:p-0`, `bg-popover/55`) is not drawn: the tree has no material, and that frame is an S71 check.
