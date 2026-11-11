# Component parity matrix (S06)

Every shadcn primitive in the Tauri app's `components/ui/`, the rune-kit component (or Kudzu composition) that draws it in the new app, how it measures against the reference, and a verdict. Plan: §1.4 (the mapping and the "only door to local component code"), §3 gap 17, §6 D6 (exact metrics for buttons, inputs and switches; rune-kit's own elsewhere).

## How the numbers were produced

- **Rune side:** `cargo xtask parity --window gallery` renders every `gallery` scene headless in both themes with the system font (SF Pro, as S02 captured), reads the `--tree`, and measures each sample by its key (`xtask/src/parity.rs`, `crates/kudzu-ui/src/gallery/components/`).
  - `parity:<window>:<scene>:<role>:<occurrence>:<name>` rebuilds a reference landmark (same label, icon, variant, size) and is gated against it: size within 2 pt, background equal (compared premultiplied, because `metrics.json` reads colours back through an 8-bit premultiplied canvas).
  - `spec:<what>:<w>x<h>` is gated against the size the Tauri classes give.
  - `info:<what>` is reported, not gated (rune-kit's own metrics, D6).
- **Reference side:** `docs/migration/reference/metrics.json` (S02) and the classes in `components/ui/*.tsx`. The side-by-side sheet (`target/parity/sheet.html`) crops each `parity:` sample and its reference landmark from the PNGs at `$KUDZU_REFERENCE_DIR` (D12); without them it is skipped with a message.
- **Run on 2026-11-11 (this commit):** 140 rows, 102 gated, **102 pass, 0 fail**. Re-run after any change to `theme/kit.rs` or `widgets/kit_ext/`.

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

Verdicts: **match** (rune-kit with the Kit, within 2 pt), **themed** (rune-kit with the Kit; its own metrics accepted by D6), **decorated** (rune-kit's component built, then given Kudzu's metrics as overrides), **wrapper** (built from Rune core, tagged `RUNE-GAP`), **composition** (no component needed, or a Kudzu widget).

### Exact-metric primitives (D6)

| shadcn | Backed by | Tauri (classes / reference) | Rune measured | Verdict |
|---|---|---|---|---|
| button | `rune_kit::Button` via `kit_ext::button` | h 24/28/32/36 (`xs`/`sm`/default/`lg`), `px-2`/`px-2.5` + 1 pt border, radius 8 (`xs`, `sm`) or 10, labels 12 / 12.8 / 14 medium, icons 12 / 14 / 16; variants default (primary, `sheen-strong`, `raised`), outline (`surface-raised`, `sheen`, `raised`), secondary, ghost, destructive (`destructive/10`). Reference: “Skip for now” 103.86×32, “Continue →” 103.56×32, “Choose file… for Raycast” 101.42×28, “Sync now” 98.78×28, “Clear” 54.16×28, “More actions” 28×28, “Hide sidebar” 32×32 (expanded, `bg-muted`) | 104×32 (Δ 0.14), 104×32 (0.44), 101.5×28 (0.08), 99×28 (0.22), 54.5×28 (0.34), 28×28 (0), 32×32 (0); fills equal in both themes. Every size and state `spec` row passes. | **decorated** |
| button (icon sizes) | same | `icon-xs` 24, `icon-sm` 28, `icon` 32 squares, glyph 12 / 16 / 16 | 24, 28, 32 | **decorated** |
| toggle | `kit_ext::button` ghost `icon-sm`; on: `surface-raised` + `soft` depth | h 28 square (`sm`), `data-[state=on]:bg-surface-raised shadow-soft` | 28×28, on-fill white (light) | **decorated** (S07's `widgets::toggle_button` composes it) |
| switch | `kit_ext::switch` | 32×18.4, knob 16 (default); 24×14, knob 12 (`sm`); 1 pt inset, capsule; track `iris` / `input` (`input/80` dark), `well` / inset-shadow; knob `background` (`white` checked, `foreground/90` unchecked in dark). Reference: “Open at login” 32×18.39, “Pause all snippets” 32×18.39, “Enabled” 24×14 | 32×18.4 (Δ 0.01), 32×18.4 (0.01), 24×14 (0); fills equal in both themes | **wrapper** |
| input | `rune_kit::TextInput` via `kit_ext::input` | h 32, 14/20, 11 pt either side, radius 10, `foreground/2.5%` (`black/15` dark) + `well`; focus: ring border + 3 pt `ring/50`. Reference: “Trigger prefix” 80×32 | 80×32 (Δ 0), fill equal in both themes; `spec` rows for empty, filled, focus, invalid, disabled pass | **decorated** |
| input-group | same, with `leading_icon` | h 32, addon `pl-2` + border, icon 16 | 180×32 | **decorated** |
| textarea | `kit_ext::textarea` | `min-h-16` (64), `px-2.5 py-2` + border, 14/20, radius 10, the input's surface. Reference: “Mac Linux” (chip choice form, `min-h-20 font-mono`) 296×80 | 296×80 (Δ 0), fill equal in both themes; empty 296×64 | **wrapper** |

### rune-kit's own metrics (D6)

| shadcn | Backed by | Tauri (classes) | Rune measured / from rune-kit source | Verdict |
|---|---|---|---|---|
| checkbox | `rune_kit::Checkbox` | 16×16, radius 4; checked `bg-primary`, `border-primary`, `text-primary-foreground`, `raised`, `sheen-strong` | 16×16, radius `kit.radius.sm` (4); checked fill is `surface_weak` (muted) with a `foreground` mark: **colour differs** | **themed** |
| label | `rune_kit::Field` label / `text` | `text-sm leading-none font-medium`, `gap-2` | Field label in `text_xsmall_medium` (12/16 medium) | **themed** |
| select | `rune_kit::Select`, `SelectAppearance::Field` | trigger h 32 (reference “Keep history for” 128×32), `pl-2.5 pr-2`, radius 10, `surface-raised` + `raised` + `sheen`; items `py-1` rounded-md | trigger 128×28; list rows 28 | **themed** (4 pt shorter: see “Owner decisions”) |
| popover | `rune_kit::Popover` | `w-72` (288), `p-2.5`, `gap-2.5`, radius 10, `popover` + ring + `shadow-md` | 310×112 for the sample content, padding 12, fill `popover`, `elevation.floating` | **themed** |
| tooltip | `rune_kit::Tooltip`, `TooltipAppearance::Standard` | `px-3 py-1.5`, radius `md` (8), `text-xs`, `bg-foreground text-background`, `shadow-float` | 156.5×24, padding 8/2, radius `kit.radius.sm` (4), fill `foreground`, ink `background`, 1 pt `border` | **themed** |
| dialog | `rune_kit::Dialog` on its modal layer | radius `xl` (14; Kudzu's own dialogs set 18), `p-4`, `gap-4`, `shadow-panel`, footer on `muted/50` | radius 6, header padding 16, fill `popover`, `elevation.overlay` (`panel`), scrim `black/10` | **themed** |
| alert-dialog | `rune_kit::Dialog` (`click_outside(false)`) + destructive `kit_ext::button` | as dialog, centred title | as dialog | **themed** |
| dropdown-menu | `rune_kit::Menu` | `p-1`, radius 10, items `px-1.5 py-1` text-sm (28 tall) rounded-md, ring + `shadow-md` | items 28 tall, list padding 2, radius 6, fill `popover`, `elevation.floating` | **themed** |
| context-menu | `rune_kit::Menu::at` (`context_menu`) | as dropdown-menu | as dropdown-menu | **themed** |
| command | `rune_kit::PopupList` under `kit_ext::input` (input group) | input `h-8`, items `px-2 py-1.5` text-sm, `data-selected:bg-muted` | rows 28 tall, active row `muted` | **themed** (the same list drives the editor's slash menu) |
| sonner | `rune_kit::Toast` | `shadow-float`, radius `md` (8) buttons, `cn-toast` | 320 wide, padding 12, radius 8, `elevation.floating` | **themed** |
| badge | `rune_kit::Badge` | h 20 (`h-5`), `px-2`, radius `4xl` (pill), `text-xs` medium | h 16, radius from rune-kit, `small_medium` | **themed** |
| separator | `rune_kit::Divider` | 1 pt, `bg-border` | 240×1, 1×24, fill `border` | **match** |
| kbd | `rune_kit::Keybind`, wrapped by S07's `widgets::shortcut` | h 20 (`h-5`), `min-w-5`, `px-1`, radius 5, 11 medium tabular, `foreground/6%`, `shadow-key` | h 14, padding 4, radius 2, `small_medium` | **themed** (S07 renders shortcuts; its widget can reach `h-5` there) |
| item | plain `row()` | `rounded-lg border px-2.5 py-2 gap-2`, title over description | 360×62 sample | **composition** |
| breadcrumb | S07's `widgets::breadcrumbs` | | | **composition** (S07) |

### In `components/ui` but never rendered

No component under `components/` or `app/` imports these, so they have no scene: **collapsible, radio-group, scroll-area, slider, tabs, toggle-group**. The app's own segmented control (`components/app/segmented.tsx`, `radio` landmarks) and slider are Kudzu compositions owned by later steps (S07 `widgets::slider`, `widgets::toggle_button`). The `.scroll()` + `Kit::scroll_bar` mapping in §1.4 stands for the screens that scroll.

## Decorations and wrappers

| Module | What rune-kit could not take from `Kit` | How it is reached | Unreachable, recorded | Upstream (gap 17, proposed in S06, not filed) |
|---|---|---|---|---|
| `kit_ext::button` (decorated) | Height, padding and radius per size (rune-kit sizes push buttons as AppKit's `NSButton`: 16/20/24/28/36, its own padding); label type per size; icon size per size; fills for `secondary` and `destructive`; the `sheen` on outline and secondary | overrides on the built `El`: `h`, `px`, `radius`, `gap`; `label_with` draws the icon and label in Kudzu's type; `fill`, `gradient`, `press_scale(0.97)`, `focus_ring(3, 0, ring/50)` | `raised-hover` and `pressed` shadows (rune-kit swaps fills, not shadows, under the pointer and while held); dark outline hover is `surface-raised` + `muted/50` rather than `color-mix(surface-raised, white 4%)` | “rune-kit `Button`: take height, padding and radius per size from `Kit`” |
| `kit_ext::input` (decorated) | Padding (rune-kit: 2 before the text, 8 after), radius (`radius.md`), the hover tint, the focus look (1 pt inside ring) | `height(32)`, `text_style(14/20)` through the builder; `pl`, `pr`, `radius`, `hover(transparent)`, `fill`, `outline(3, 0, ring/50)` + an inset ring on the built `El` | none for the matched metrics | “rune-kit `TextInput`: take padding, radius and the focus ring from `Kit`” |
| `kit_ext::switch` (wrapper, `RUNE-GAP(#573)`) | Track and knob size: rune-kit draws an `NSSwitch` (36×16 and up, a 21 pt pill knob) or the OpenCode web switch (24×16, 4 pt corners); both are inner elements | built from Rune core: track `el()` with the knob as an absolute child on the soft spring | the knob's live stretch while pressed (`group-active:scale-x-[1.2]` from one edge): Rune has no press-driven child geometry; drawn for a forced `State::Pressed` only. The `after:-inset-x-3 -inset-y-2` enlarged hit area is not built. | “rune-kit `Switch`: take the track and knob size from `Kit`” |
| `kit_ext::textarea` (wrapper, `RUNE-GAP(#573)`) | The text style (rune-kit: `ty.ui` on a 1.35 line) and padding (8); its editor is an inner element | built from Rune core's `text_input` in a padded column with the input's surface | none for the matched metrics | “rune-kit `TextArea`: take the text style from `Kit`” |

## Known differences outside the gate

- Checkbox checked colours (above): rune-kit reads `surface_weak`, `std_border`, `std_icon`; mapping those to primary would recolour other kit surfaces. Proposed: “rune-kit `Checkbox`: take the checked fill and mark colours from `Kit`”. It is used once (the clipboard “Also clear pinned items” confirmation).
- Overlay radii are rune-kit constants (menu 6, dialog 6, toast 8, tooltip `radius.sm`), smaller than Kudzu's 10 / 14–18 / 8. D6 accepts them; S73 shows them side by side.
- Focus rings: shadcn draws a ring-coloured border plus a 3 pt `ring/50` halo; the decorated button and the wrappers draw the halo as a focus ring and, for a forced focus state, the border too. rune-kit's other components keep their 2 pt `border_focus` ring.
- Press: shadcn scales a pressed control to 0.97 and swaps to `shadow-pressed`; the decorated button scales, the shadow stays.
