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

PNG uses Chromium's lossless PNG encoder; there are no JPEG intermediates. `verify.mjs --dedupe` removes obsolete files and stores identical captures once, preserving aliases in metrics. `STATES.md` maps window-scoped scene names to preview paths; `metrics.json` resolves aliases to actual PNGs. Each metric records logical viewport size, visible DOM landmark rectangles, computed colors, font family/size/weight, radius, padding, shadow and semantic token values. Role/slot/label plus occurrence identify landmarks; the numeric key is only a stable occurrence within that capture. S06 should match semantic landmarks rather than assume DOM indices exist in Rune.

The form uses its measured dialog height + 24 pt browser padding, capped at 640. Other sizes come directly from `tauri.conf.json`, `windows.rs` and `permissions/mod.rs`: main 1180×760, palette 760×480, picker 800×520, permission companion 420×660. These are content viewports, without browser chrome.

Fixtures call the original implementations. Search includes the entire input library for a seeded 5,000-snippet corpus, ordered hit IDs, scores and UTF-16 name-hit positions. Clipboard covers queries, aliases, filters, sections, labels and ranges. Formatting uses actual CodeMirror state/history transactions through the existing loader. Private colour math is transpiled directly from the original TSX helper source. `color-inputs.json` records browser Canvas sRGB decoding (including invalid CSS falling back to black); `color.json` derives conversions and contrast from those inputs. This division keeps Node independent of a browser canvas implementation. To refresh decoding, remove only `color-inputs.json`, rerun capture, then regenerate fixtures.

Limits: browser preview captures do not prove native vibrancy, shadows, clipping, inter-app focus, motion, expansion or permission behavior. See S72 for those. `BASELINE.md` records passive installed-process sampling; launch/palette timings and complete physical footprint remain owner measurements. No Dev build was launched.

The rule-8 Rust gates are unavailable at this source commit because S01's Cargo workspace is absent. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` fail because there is no Cargo.toml; `cargo xtask check-deps` fails because there is no xtask command. S02 adds no Rust files and does not fabricate that workspace.
