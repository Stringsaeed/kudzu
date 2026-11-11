# S04 review fixes

The CSS excerpt in `mod.rs` comes from commit
`bca79f179f5b6e7c869161d80ce6d3ebb1106689`, `apps/desktop/app/globals.css`.
The pinned Rune source was verified at `29aacb688883a9712bc27175bdafd99fc6edb421`.

`gamut.rs` implements [CSS Color 4 binary search with local MINDE](https://www.w3.org/TR/css-color-4/#binsearch):
constant lightness/hue, chroma search, deltaEOK JND 0.02, epsilon 0.0001,
then clipping. Rune's `Color::from_oklab` clips too early to measure that distance.
Tests cover the spec's Display-P3 yellow / reduced-chroma boundary example,
white/black lightness limits, preserved alpha, and the S02 resolved channels.

The owner also requested an experiment with the draft's [ray-trace algorithm](https://www.w3.org/TR/css-color-4/#raytrace).
A standalone Python implementation of §14.2.6 (four iterations, linear-sRGB cube
intersection, original lightness/hue restoration) was tried against the S02 table.
It did not meet ±1: light chip-time became (0,98,138), chart-2 (50,128,255),
chart-1 (145,197,255), destructive (228,0,22). Their reference values are
(0,99,152), (43,127,255), (142,197,255), (231,0,11), respectively.
Binary search is retained. The owner approved ±2 for light chip-time alone;
every other token remains ±1. Chromium's mapping differs slightly; the shipped
Tauri app renders through WebKit, which may differ again. Translucent canvas
reference channels are compared premultiplied to avoid amplified 8-bit
unpremultiplication noise (the reference's white dark-border red is 245).

## Out-of-sRGB tokens

8-bit sRGB before → after; alpha is unchanged. Values that round identically still
have an unbounded linear RGB component outside [0,1]. Neutral white's floating
point boundary noise is excluded.

| Appearance | Tokens | Before | After |
|---|---|---|---|
| Light | destructive | #e7000b | #e7000b |
| Light | chart-1 | #8ec5ff | #8ec5ff |
| Light | chart-2 | #2b7fff | #2b7fff |
| Light | chip-time | #006398 | #006396 |
| Light | chip-clipboard | #006e40 | #006e40 |
| Light | chip-form | #9c5400 | #9c5400 |
| Light | success | #009956 | #009956 |
| Light | warning | #d28500 | #d28500 |
| Dark | destructive | #ff6467 | #ff6467 |
| Dark | chart-1, iris-foreground, chip-cursor | #8ec5ff | #8ec5ff |
| Dark | chart-2, sidebar-primary, iris, iris-soft (alpha .18) | #2b7fff | #2b7fff |
| Dark | chip-error | #ff807b | #ff807b |

## Blending and golden precision

`theme::window::spec` sets sRGB blending and FontSmoothing::Off. Both current
windows use it. The source guard rejects direct Rune window constructors or blend
setters anywhere else in composition/UI source, including future overlays.
Rune's `theme::for_blend` and `Theme::alpha` compensate alpha for linear blending;
they are unnecessary when sRGB compositing is the invariant, and cannot recover
RGB gamut information already clipped by `oklcha`.

Both rendered images and checked-in goldens have every swatch centre sampled at
its keyed layout bounds, with ±1 against the resolved token composited over the
background in sRGB. The full-image comparison remains. Independent shadow pixels
were not added: blur, overlapping layers and inset edges need renderer-specific
coverage calculations; the existing depth-table tests and image comparison remain.

Regeneration changed 15,184 pixels in the light golden, bounding box
x=896–1087, y=838–917 (physical pixels, 2×); maximum channel delta 3 at rasterized
edges. This is the chip-time swatch. The dark golden is pixel-identical.

## Proposed upstream issue (not filed)

Worth proposing as a separate, lower-priority capability; it does not block sRGB
parity. `gpu/metal/surface.rs:89` advertises only SurfaceColorSpaces::SRGB,
so changing texture format alone cannot enable Display P3 output.

Title: **Support Display P3 color spaces on Metal window surfaces**

Body:
Metal surfaces currently advertise only SRGB; please expose Display P3 surface selection with matching CAMetalLayer color-space configuration.
Preserve wide-gamut authored colors until output mapping, while retaining explicit sRGB output/blending for CSS parity and headless goldens.

## Validation and integration

All requested gates passed: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
(508 passed, zero failed), `cargo xtask check-deps`, and
`python3 scripts/check-headless.py` (all four modes under the filesystem sandbox).

Integration edits outside the theme/gallery/golden paths: `src/compose.rs` routes
the main window through the helper; root `Cargo.toml` references the already
approved workspace `serde_json` dependency for tests, with its generated
`Cargo.lock` entry. No new crate was introduced. Implementer: GPT-6.1-sol (high).
