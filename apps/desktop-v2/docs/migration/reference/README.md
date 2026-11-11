# S02 reference capture

## Screenshots

The 126 PNGs are **not in git** (owner's decision, 2026-11-11): they would keep 40 MiB in history after cut-over. They live at `$KUDZU_REFERENCE_DIR`, default `~/.local/share/kudzu-migration/reference/`, in the same `<window>/<state>.<theme>.png` layout that `metrics.json` and `STATES.md` name. `capture.mjs` still writes them here (ignored by `.gitignore`); after a capture, sync them with `rsync -a --include='*/' --include='*.png' --exclude='*' apps/desktop-v2/docs/migration/reference/ ~/.local/share/kudzu-migration/reference/`. They can be regenerated from the frozen Tauri app until cut-over. To run `verify.mjs`, copy them back here first. CI never compares pixels.

Source: `2352d319969bc753b2de1ca12ce76b7f8c3035c6` in `t3/capture-reference-fixtures`. This directory is an offline yardstick, not a dependency of the new app. Only these tools read the frozen Tauri source. No package manifests or lockfiles were changed.

Run from the repository root:

```sh
pnpm install
# Separate terminal; port can be changed through REFERENCE_URL for the capture.
cd apps/desktop && pnpm dev --port 3042
```

```sh
# Generate pure fixtures first (the first run needs color-inputs.json, already provided).
TZ=UTC LANG=en_US.UTF-8 UPDATE_FIXTURES=1 node --loader ./apps/desktop/scripts/tests/typescript-loader.mjs --test apps/desktop-v2/docs/migration/reference/fixtures.mjs
pnpm dlx playwright@1.65.0 install chromium
pnpm dlx --package playwright@1.65.0 -c 'node apps/desktop-v2/docs/migration/reference/capture.mjs'
node apps/desktop-v2/docs/migration/reference/verify.mjs --dedupe
# Verify without updating any fixtures.
TZ=UTC LANG=en_US.UTF-8 node --loader ./apps/desktop/scripts/tests/typescript-loader.mjs --test apps/desktop-v2/docs/migration/reference/fixtures.mjs
node apps/desktop-v2/docs/migration/reference/verify.mjs
```

`determinism.mjs` fixes 2026-11-11 12:00 UTC, UTC timezone, en-US locale, and LCG seed 2352. UUIDs use a separate seeded stream. Browser contexts are fresh per state/theme, with reduced motion, native macOS system fonts and device scale 2. The sample library is injected into mock localStorage from `mock-state.json`; 22 seed-library UUID calls are skipped so clipboard IDs follow the same sequence as Node. Chip scenes change only the first sample snippet's content. All mutations affect isolated browser mock data. The installed app and its data are never opened by these tools.

PNG uses Chromium's lossless PNG encoder; there are no JPEG intermediates. `verify.mjs --dedupe` removes obsolete files and stores identical captures once, preserving aliases in metrics. `STATES.md` maps window-scoped scene names to preview paths; `metrics.json` resolves aliases to actual PNGs. Permission captures remove the browser-only `PermissionPreview` aside before measuring or photographing; revoked remains an alias of missing.

## Semantic metrics (schema 2)

`metrics.json` is compact JSON, bounded below 800,000 bytes by the verifier. Scene viewports and rectangles use logical points, rounded to 0.001 pt. Only visible buttons, inputs (including their semantic ARIA roles), headings, named regions/landmarks and template chips are included. CSS-hidden elements are excluded. Visible controls behind modal overlays remain geometry landmarks even when inert or aria-hidden, so screen comparisons retain the underlying layout. There are no DOM-index keys, data-slot-only nodes, CodeMirror containers or generic role containers.

Each landmark is a tuple following `landmarkColumns`:

```text
[role, accessibleName, occurrence, [x, y, width, height], styleId]
```

The first three values form the semantic key. Occurrence is 1-based among visible landmarks sharing the same role/name within the scene, in reading order. Names come from aria-label, aria-labelledby/associated label text, trimmed own text, or the control's single text label span beside its icon. Counts, shortcut groups and arbitrary concatenated descendant text are not used as names. A control without such a name is omitted. Rune compares the resolved semantic key, regardless of how its element tree is structured.

`styleId` indexes `tables.styles`. Each style follows `styleColumns`:

```text
[fontId, fontSize, fontWeight, radius, padding, foregroundId, backgroundId, shadowId]
```

Font sizes and weights are numeric. Radius is top-left/top-right/bottom-right/bottom-left; padding is top/right/bottom/left. Computed px lengths are numeric logical points; a non-px radius retains its CSS unit. Font and shadow strings are interned in `tables.fonts` and `tables.shadows`. Colour IDs index `tables.colors`: `[red, green, blue, alpha]`, sRGB integer channels 0–255 and alpha 0–1, resolved through a one-pixel sRGB Canvas readback (8-bit alpha, rounded to 0.001). Repeated style records and colour values are interned too.

`tokens.light` and `tokens.dark` are shared across scenes. Each CSS custom-property name maps to `{source, rgba}`: `source` indexes `tables.colorSources`, holding the exact authored declaration value from `globals.css` (including `var(...)` references); `rgba` indexes the same resolved sRGB colour table. The light declarations come from `:root`; dark overlays only the standalone `.dark` block, inheriting unchanged root values. Motion/font/shadow declarations are excluded from this colour-token map. Token sources are never Chromium-normalized `lab()` text. Shadows retain their computed CSS string as a separate interned style property. `verify.mjs` checks authored token text against the frozen source.

The form uses the native `resize_form` contract: measured dialog height clamped to 160–680, with no extra 24 points. Its browser-only outer padding is removed to match the native `vibrant:p-0` layout before measuring. `dialogHeight` records the clamp input. Other sizes come directly from `tauri.conf.json`, `windows.rs` and `permissions/mod.rs`: main 1180×760, palette 760×480, picker 800×520, permission companion 420×660. These are content viewports, without browser chrome.

A full capture refreshes every scene and the shared dictionaries. For a targeted repeat after that, `ONLY=demo` captures both form themes; comma-separated substrings select several scenes. It reuses schema-2 dictionaries and replaces the selected scene rows. Run verification/deduplication afterwards, then sync the PNGs using the Screenshots command above.

Fixtures call the original implementations. Search includes the entire input library for a seeded 5,000-snippet corpus, ordered hit IDs, scores and UTF-16 name-hit positions. Clipboard covers queries, aliases, filters, sections, labels and ranges. The S22 review extension adds dated time and both secret-visibility states for all 85 entries, 35 URL descriptions (including two IDN hosts), six UTF-16 range cases (including İ expansion and astral lowercase), eight byte labels (including positive ties), and a previous-year day label. Every expected output is recorded by calling the frozen TypeScript. Rust deliberately keeps Unicode IDN titles rather than implementing full WHATWG/IDNA punycode; the two title deviations are explicit in its tests, while their subtitles still match. The range fixtures preserve TypeScript offsets in the lowercased string, including expansion drift. Formatting uses actual CodeMirror state/history transactions through the existing loader, including underline, link creation/editing, clear formatting, image insertion and removing image formatting while retaining its alternative text. Private colour math is transpiled directly from the original TSX helper source. `color-inputs.json` records browser Canvas sRGB decoding (including invalid CSS falling back to black); `color.json` derives conversions and contrast from those inputs. This division keeps Node independent of a browser canvas implementation. To refresh decoding, remove only `color-inputs.json`, rerun capture, then regenerate fixtures.

Limits: browser preview captures do not prove native vibrancy, shadows, clipping, inter-app focus, motion, expansion or permission behavior. See S72 for those. `BASELINE.md` records passive installed-process sampling; launch/palette timings and complete physical footprint remain owner measurements. No Dev build was launched.

The rule-8 Rust gates are unavailable at this source commit because S01's Cargo workspace is absent. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` fail because there is no Cargo.toml; `cargo xtask check-deps` fails because there is no xtask command. S02 adds no Rust files and does not fabricate that workspace.

## Collation fixture (S21)

`collation.mjs` records orderings from the runtime's own ICU `String.prototype.localeCompare` (`en-US`): 529 strings covering ASCII, digits, punctuation and symbols, emoji, Latin accents and expansions (including `ß`), German, French, Turkish, CJK, other scripts, snippet-name shapes and ignorable characters. It is deterministic, and `--check` fails if `fixtures/collation.json` is stale. The Rust copy lives in `crates/kudzu-core/tests/fixtures/ts-parity/collation.json`.

```sh
node apps/desktop-v2/docs/migration/reference/collation.mjs          # write
node apps/desktop-v2/docs/migration/reference/collation.mjs --check  # verify
```
