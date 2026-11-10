# CRATES.md — buy vs build for the pure-Rust (Rune) Kudzu

Research proposal, not a decision record. Every "recommendation" below needs the owner's
sign-off before a dependency is added. Versions, release dates and download counts were
verified against the crates.io API on 2026-11-11; anything unverified is marked. "Latest"
means newest non-yanked release. Download counts are lifetime totals, rounded.

**Current dependency baseline** (what the Tauri app compiles today):

- `apps/desktop/src-tauri/Cargo.toml`: tauri 2 (tray-icon, macos-private-api, image-png);
  tauri-plugin-global-shortcut 2, -dialog 2, -opener 2, -autostart 2, -single-instance 2,
  -updater 2, -log 2; serde 1, serde_json 1, uuid 1 (v4, serde), chrono 0.4, log 0.4,
  thiserror 2 (workspace), base64 0.22, parking_lot 0.12, tokio 1 (features: time, sync);
  objc2 0.6, objc2-foundation 0.3, objc2-cloud-kit 0.3 (CKSyncEngine set), block2 0.6.
- `kudzu-core`: serde 1, serde_json 1, uuid 1, chrono 0.4, thiserror 2,
  unicode-segmentation 1.12, unicode-normalization 0.1, **yaml-rust2 0.11**.
- `kudzu-engine`: serde/serde_json/uuid/chrono/log/unicode-segmentation 1; macOS:
  core-foundation 0.10, core-foundation-sys 0.8, plist 1, objc2 0.6, objc2-foundation 0.3,
  objc2-app-kit 0.3 (NSPasteboard, NSImage, NSBitmapImageRep, NSWorkspace, NSRunningApplication…).

Rune context (verified in `scratchpad/rune-src/rune/crates/`): the framework already ships
`rune-highlights` (concern 19), `rune-i18n` — which depends on **icu_decimal / icu_list /
icu_locale / icu_plurals / icu_provider 2.x** — and `rune-mac`/`rune-notify` built on objc2.
Windows, tray and hotkey provision are being audited by another agent; this file only notes
where that answer changes ours.

---

## 1. Secret detection in clipboard text and MCP redaction

**Today.** `kudzu-core/src/secrets.rs` (742 lines, heavily tested): PEM private-key blocks,
JWTs, 37 vendor prefixes (`ghp_`, `sk-ant-`, `AKIA…`), AWS/Google fixed shapes, Sendgrid,
labelled values (`password=`, `"api_key":`, `Bearer …`), URL credentials and webhook paths,
plus Shannon-entropy/character-class shape detection for lone passwords and random runs
(20–512 chars, UUID/commit-id exclusions). Produces kind-tagged byte ranges; `redact()`
emits `[redacted: API key]`-style markers and a `whole` flag; `detect()` scans the first
256 KiB. MCP tools in `kudzu-core/src/agent/` run every result through this before an agent
sees it, and search matches only redacted text. Zero dependencies.

**Candidates.**
- `ripsecrets` 0.1.11 (2025-05-27, ~17k dl, MIT, sirwart/ripsecrets) — gitleaks-style
  regex+entropy scanner, CLI-focused; pattern list is the valuable part.
- `redact-secret` 0.1.0-beta.15 (2026-10-10, ~1.4k dl, MIT) — "deterministic secret
  detection and redaction for AI context"; exactly our use case, but beta and ~zero
  adoption. Unverified quality.
- `secrets_scanner` 0.2.3 (2026-08-13, ~17k dl, MIT) — young Aho-Corasick + regex + entropy.
- gitleaks and TruffleHog themselves are Go; no maintained Rust port of either rule set
  exists (verified by search, 2026-11-11).

**Same result?** No. Those scanners are git-oriented, line/regex based. They would not
reproduce the lone-password heuristic (`hunter2` vs `P@ssw0rd!` in the middle of prose),
the labelled-value parser with its placeholder exclusions (`process.env.PASSWORD` is not a
secret), the `whole`-text flag the UI relies on, or our exact redaction markers — all pinned
by `secrets.rs` tests. Entropy checks we already have.

**Recommendation: keep ours.** Optionally mine gitleaks/ripsecrets pattern lists to extend
`PREFIXES` — data, not a dependency.

## 2. MCP server over stdio / unix socket

**Today.** `kudzu-core/src/agent/` — transport-agnostic synchronous JSON-RPC handler
(protocol revisions 2025-11-25 … 2024-11-05), tools list/read/expand/create/edit snippets,
search clipboard history, redaction woven in. `src-tauri/src/agent.rs` serves it on a 0600
unix socket; `Kudzu --mcp` bridges agent stdio to that socket. Plain threads, no async
runtime.

**Candidates.** `rmcp` 3.9.0 (2026-11-05, ~47M dl, Apache-2.0, modelcontextprotocol/
rust-sdk with Cloudflare) — the official Rust SDK; futures-based, tokio used by the
stdio/child-process transports.

**Same result?** Protocol framing and revision negotiation yes; behaviour no. Our server is
deliberately narrow (7 tools, one socket, no notifications/sampling/streaming), synchronous,
and redaction-integrated. rmcp brings an async stack (tokio) into an otherwise thread-based
app, and the tool bodies — where all our logic and tests live — would be rewritten as
handlers either way.

**Recommendation: keep ours** for now; revisit rmcp if MCP spec churn becomes a maintenance
cost we're paying twice. Note: dropping tokio elsewhere (concern 20) makes rmcp's runtime
the odd one out.

## 3. Fuzzy search / ranking (quick search and filters)

**Today.** `lib/search.ts` (TypeScript, moves to Rust in the rewrite): fzf-model greedy
subsequence match with boundary/camelHump bonuses and a backwards tightening pass; per-term
best-of over fields weighted name 1.0 > trigger 0.85 > content 0.6 (content is
contiguous-substring-only); exact trigger hit wins (10 000), trigger prefix 5 000; usage
boost `log2(1+count)*4` + recency tiers; disabled snippets −15; folded-field index cache.

**Candidates.**
- `frizbee` 0.13.0 (2026-08-13, ~3.0M dl, MIT, saghen — Zed's maintainer) — Zed's fuzzy
  matcher extracted; SIMD Smith-Waterman, "similar algorithm to FZF".
- `nucleo-matcher` 0.3.1 (2024-02-20, ~5.9M dl, **MPL-2.0**, helix-editor/nucleo) — the
  fzf-model matcher Helix uses; no release since Feb 2024, repo alive via Helix.
- `fuzzy-matcher` 0.3.7 (2020-10-04, ~38M dl, MIT) — skim v1 algorithm; effectively
  unmaintained. Avoid.

**Same result?** No crate reproduces the ranking — usage boost, field weights, trigger
exact/prefix wins and content-gating stay ours regardless. What a crate replaces is the
inner per-term `fuzzy()` call. frizbee's Smith-Waterman is typo-tolerant, so slightly more
permissive than our strict subsequence (matches "kuzu"→"kudzu" the same way, but also
near-misses); scores differ, so ordering near ties changes. The TS behaviour has no Rust
test to re-pin yet; the port is the moment to calibrate.

**Recommendation: adopt frizbee** for the inner matcher (active, MIT, SIMD-fast, built for
exactly this UI) and keep `search.ts`'s ranking shell around it. nucleo-matcher is the
fallback if we want the literal fzf scoring model (mind the MPL-2.0).

## 4. Global hotkeys

**Today.** `src-tauri/src/hotkey.rs` via tauri-plugin-global-shortcut 2: two user-configurable
slots (quick search, clipboard history), re-register with rollback on conflict, "another app
may own it" error surface.

**Candidates.** `global-hotkey` 0.10.0 (2026-11-09, ~7.6M dl, Apache-2.0/MIT, tauri-apps —
standalone, **no Tauri dependency**; this is the engine tauri-plugin-global-shortcut wraps;
Carbon `RegisterEventHotKey` on macOS). Also: our own `kudzu-engine/src/macos/tap.rs`
CGEventTap already observes every keyDown — hotkeys could be matched there with zero new
dependencies, since the app already requires Accessibility + Input Monitoring for expansion.

**Same result?** global-hotkey reproduces the current register/unregister/conflict behaviour
1:1. Matching in the tap instead changes failure modes (tap disabled by timeout, hotkeys die
when expansion is toggled off) but adds no permission the app doesn't already need.

**Recommendation: adopt global-hotkey@0.10** — unless the Rune audit (in flight) provides
hotkeys, in which case use those; the tap route is the zero-dependency fallback.

## 5. Login item

**Today.** tauri-plugin-autostart 2, `MacosLauncher::LaunchAgent`, `--hidden` argument.

**Candidates.**
- `objc2-service-management` 0.3.2 (2025-10-04, ~950k dl, Zlib OR Apache-2.0 OR MIT, madsmtm)
  — generated `SMAppService` bindings; same objc2 family/versions we already pin.
- `smappservice-rs` 0.1.3 (2025-05-27, ~560k dl, MIT) — friendly wrapper over SMAppService;
  one small release.
- `auto-launch` 0.6.0 (2026-01-10, ~7.9M dl, MIT) — the LaunchAgent-plist route, i.e. what
  the Tauri plugin effectively does today.

**Same result?** `auto-launch` = today's behaviour exactly. `SMAppService` (macOS 13+) is
the modern API: the toggle shows in System Settings ▸ Login Items & Extensions (better for
users), but registering the main .app does not pass launch arguments, so `--hidden` needs a
small in-app check ("was I launched at login → start hidden") instead. Both are fine.

**Recommendation: adopt objc2-service-management@0.3** — SMAppService, no wrapper layer,
fits the existing objc2 stack. Fall back to `auto-launch` only if exact LaunchAgent parity
matters.

## 6. Single instance

**Today.** tauri-plugin-single-instance 2 — on macOS a **Unix socket** (verified in the
plugin source): the second instance hands its argv/cwd to the first and exits; the first
focuses the main window.

**Candidates.** `single-instance` 0.3.3 (2021-12-16, ~1.0M dl, MIT) — file-lock only, no
argv forwarding, stale for 5 years. Or our own: a tiny `UnixListener` in the data dir plus
`NSRunningApplication` (already available via objc2-app-kit) to focus ourselves.

**Same result?** Our own socket handoff reproduces it exactly, and `src-tauri/src/agent.rs`
already contains all the socket plumbing this needs (`Kudzu --mcp` dials a unix socket
today). The stale crate does less.

**Recommendation: keep ours** (port the ~50-line socket handoff). No dependency.

## 7. Auto-update

**Today.** `src-tauri/src/updater.rs` + tauri-plugin-updater 2: `latest.json` / `beta.json`
GitHub-release feeds, **minisign** signatures, download → verify → hold in memory → install
on quit or on explicit "Restart to Update". Never restarts on its own (a text expander
restarting mid-sentence would eat a trigger).

**Candidates.**
- `minisign-verify` 0.3.0 (2026-09-25, ~21M dl, MIT, jedisct1) — this is the crate
  tauri-plugin-updater uses internally for signature verification.
- Sparkle 2 via objc2 bindings — e.g. `sparkle-updater` 0.1.0 (2026-09-19, ~1.9k dl, MIT;
  unproven) or hand-bound with objc2; appcast feed, EdDSA, delta patches, but it owns the
  install/relaunch cycle and needs the framework bundled.
- `self_update` 1.3.3 (2026-10-27, ~13M dl, MIT) — replaces a binary in place; designed for
  CLI executables, poor fit for a bundled, notarized .app.
- `velopack` 1.3.39 (2026-10-17, ~200k dl, MIT) — full installer+updater stack with ~26
  dependencies and its own packaging pipeline; replaces rather than verifies our feed.
- `cargo-packager` 0.11.8 (2025-11-27, ~256k dl, Apache-2.0/MIT) — packaging only; no
  standalone reusable updater (the Tauri updater is 3.0.0-alpha and still Tauri-bound).
- HTTP client for the download: `ureq` 3.5.1 (2026-11-02, ~262M dl, MIT/Apache, blocking,
  rustls) or `reqwest` 0.13.5 (2026-09-08, ~897M dl, MIT/Apache).

**Same result?** Only the own-updater route preserves the UX contract (deferred install,
never-mid-sentence restart), the feed format, and the signature scheme — and it is small:
fetch JSON, GET the .dmg/.app, `minisign-verify` the bytes, swap on quit. Sparkle is the
macOS gold standard but changes the interaction model and infra; velopack changes the
packaging entirely.

**Recommendation: replace with our own small updater** built on
`minisign-verify@0.3` + `ureq@3` (keep `latest.json`/`beta.json` unchanged). Sparkle 2 is
the alternative if we later want delta updates badly enough to adopt appcasts.

## 8. Logging

**Today.** `log` 0.4 facade throughout kudzu-core / kudzu-engine / the shell; sinks and
rotation provided by tauri-plugin-log 2 (file + macOS console + webview console).

**Candidates.** `tracing` 0.1.44 (2025-12-18, MIT, tokio-rs), `tracing-subscriber` 0.3.23
(2026-03-13), `tracing-appender` 0.2.5 (2026-04-17, rolling files), `tracing-oslog` 0.3.0
(2025-05-29, ~3.6M dl, Zlib, Absolucy) for unified logging.

**Same result?** Better. tracing's `log` feature converts existing `log::` records into
tracing events, so kudzu-core/engine keep compiling untouched; a subscriber stack of
(rolling file + oslog + stderr) replaces the plugin. There is no webview console to feed
any more — Rune's own tooling covers UI-side logs.

**Recommendation: adopt tracing@0.1 + tracing-subscriber@0.3 + tracing-appender@0.2 +
tracing-oslog@0.3** replacing tauri-plugin-log. Risk: tracing-oslog is a single-maintainer
Zlib crate; acceptable, and droppable independently.

## 9. Date/time formatting (ICU patterns, offsets, locale)

**Today.** `kudzu-core/src/template/icu.rs` — 614-line hand-written ICU/DateFormatter
pattern engine on top of `chrono::DateTime<FixedOffset>`: all common letter fields, quoted
literals, localized names for en/fr/de/es/it/pt/nl/ar (others fall back to English), always
Latin digits, GMT-offset zone names. `interchange/strftime.rs` converts Espanso strftime
patterns to ours on import (pure mapping, tests pin it).

**Candidates.**
- `icu_datetime` 2.4.0 / `icu` umbrella 2.4.0 (2026-10-22, Unicode-3.0, unicode-org) —
  real CLDR pattern formatting, hundreds of locales, fixed-offset zones supported.
  **Rune already depends on icu_* 2.x via rune-i18n**, so the provider/data plumbing is
  shared.
- `jiff` 0.2.42 (2026-11-10, Unlicense/MIT, BurntSushi) — superb datetime/tz library, but
  formats strftime-style, not ICU patterns; it would replace *chrono*, not the formatter.
- `chrono` 0.4.45 (2026-06-04, MIT/Apache) — current base.

**Same result?** icu_datetime with a `CustomTimeZone` built from our fixed offset
reproduces the pattern semantics for all locales — strictly better than 8 hand-tables.
Differences to expect: localized digits for `ar` (today always Latin), CLDR-correct
abbreviations (our tables have known simplifications), some width edge cases → tests that
pin exact strings will need re-baselining, mostly for non-English locales. strftime.rs
stays as is.

**Recommendation: replace `template/icu.rs` with icu_datetime@2.4** (via the `icu`
umbrella or just icu_datetime + icu_calendar + icu_locale + icu_timezone). Keep chrono for
now; migrating chrono→jiff is optional and orthogonal.

## 10. YAML for Espanso import/export

**Today.** `kudzu-core/src/interchange/espanso/yaml.rs` builds a permissive document tree
from **yaml-rust2 0.11** parser events with line numbers (for import warnings); the exporter
uses yaml-rust2's `YamlEmitter` with `multiline_strings`.

**Candidates.**
- `yaml-rust2` 0.13.0 (2026-09-11, MIT/Apache, Ethiraric) — same crate, actively released.
- `saphyr` / `saphyr-parser` 0.3.3 (2026-10-27, MIT/Apache, saphyr-rs — the same authors'
  successor project) — event parser with marks (import side), but **no emitter**, so the
  export side would need a rewrite.
- `serde_yaml` 0.9.34+deprecated (dtolnay, archived 2024-03) — do not adopt.
- `serde_yml` 0.0.13 — now published as a deprecated shim; avoid.
- `serde_norway` 0.9.42 (2024-12-21, ~20M dl, MIT/Apache) — maintained serde_yaml fork;
  serde-based, so no line numbers and no all-scalars-as-strings leniency; wrong shape for
  Espanso's permissive configs.

**Same result?** Bumping yaml-rust2 0.11 → 0.13 keeps both halves (parser + emitter) of the
same API lineage; 0.12/0.13 are fixes/perf on this path. saphyr would change the importer's
API and orphan the exporter.

**Recommendation: keep ours, bump yaml-rust2 to 0.13.**

## 11. RTF and HTML reading; HTML generation for formatted pastes

**Today.** `kudzu-core/src/interchange/rich/`: hand-written, bounded readers — `rtf.rs`
(Cocoa RTF from TextExpander: bold/italic/underline, HYPERLINK fields, list markers,
code-page chars, `\pict` PNG/JPEG; 4 MiB cap), `html.rs`, `markdown.rs` (a CommonMark
subset) — all feeding `RichText` (source-app template language + byte-range styles +
`Dropped` warnings). Generation: `formatting.rs::html` emits only our own tags, escapes
everything (`escape_html`) and gates URLs through `safe_link` (no `javascript:`/`data:`),
with tests pinning hostile-input handling.

**Candidates.** `rtf-parser` 0.5.0 (2026-11-09, ~255k dl, MIT, d0rianB — fast RTF
lexer/parser with styling); `html5ever` 0.40.1 (2026-09-14, servo) + `scraper` 0.29.0
(2026-10-21, ISC) for parsing; `ammonia` 4.2.4 (2026-10-17, MIT/Apache) for sanitising.

**Same result?** No. Our readers' job is not "parse the format" but "convert to RichText
spans with named-dropped warnings under hard size/depth caps so hostile imports degrade to
plain text". rtf-parser doesn't cover `\pict` pictures or HYPERLINK fields as span+range
(verify before use); html5ever is spec-compliant but heavy (markup5ever/tendril) and still
leaves the whole span-mapping walker to us. On the output side ammonia is unnecessary: we
generate, not sanitize, and already refuse untrusted markup and unsafe URLs.

**Recommendation: keep ours.** Revisit `rtf-parser` only if it grows picture/field support;
our RTF reader is the single most test-dense import path.

## 12. Image decode / resize / encode to compact PNG

**Today.** kudzu-engine (`macos/clipboard.rs::thumbnail`, `macos/apps.rs::draw_png`):
NSImage decode → NSGraphicsContext resize (max 480 px, no upscaling) → NSBitmapImageRep
PNG encode, via objc2-app-kit (already a dependency). `kudzu-core/src/interchange/rich/
image.rs` hand-rolls strict base64 and PNG/JPEG dimension sniffing for imports.

**Candidates.** `image` 0.25.10 (2026-03-10, ~246M dl, MIT/Apache, image-rs);
`fast_image_resize` 6.1.0 (2026-07-21, MIT/Apache, SIMD); `oxipng` 10.2.1 (2026-09-02, MIT);
`png` 0.18.1 (2026-02-14, MIT/Apache).

**Same result?** AppKit already does decode/resize/encode exactly and is tested on real
pasteboard data. `image` would make the engine pure-Rust/cross-platform — but the app is
macOS-only and objc2-app-kit stays regardless (concern 13). oxipng is lossless-compression
tooling: wrong trade for a thumbnail written on every copy. fast_image_resize targets large
images; ours are ≤480 px.

**Recommendation: keep ours** (AppKit). `image@0.25` is the drop-in if we ever want the
engine testable off macOS.

## 13. System clipboard with rich types

**Today.** `kudzu-engine/src/macos/pasteboard.rs` on NSPasteboard via objc2-app-kit:
snapshot/restore of *every item and type* as raw `(UTI, bytes)` pairs, change-count
polling (300 ms), own-change ring buffer so restores aren't recorded, transient writes
(`write_transient`), promised-data detection, restore-after-paste with a 400 ms delay;
`kudzu-core/src/clipboard.rs` adds the nspasteboard.org privacy markers
(`org.nspasteboard.ConcealedType`, …), classification, and UTI preference order
(PNG/JPEG/GIF/WebP/HEIC/TIFF, RTF/HTML, file URLs, PDF).

**Candidates.** `arboard` 3.7.0 (2026-10-15, ~65M dl, MIT/Apache, 1Password) — text +
images only, single item, no raw types, no change count, no concealment markers.
`clipboard-rs` 0.3.5 (2026-06-30, ~1.2M dl, MIT) — text/html/files/images + change
monitoring; closer, but no multi-item raw snapshot/restore, no own-change tracking, and it
wraps NSPasteboard anyway.

**Same result?** Neither crate can: multi-item fidelity (exact restoration is a core
feature), app-specific types, concealed/transient markers, or change-count semantics. Both
would be a functional downgrade.

**Recommendation: keep ours** (objc2-app-kit NSPasteboard directly).

## 14. Persistence (library, settings, clipboard history)

**Today.** `src-tauri/src/store.rs` — library JSON (the sync contract, additive-only),
settings, form memory, all written atomically (temp + fsync + rename). `clipboard/store.rs`
— `history.json` index (metadata + excerpts, debounced writes) + one blob per copy
(`items/<id>.kzc`) + `thumbs/<id>.png` + trash for undo; ≤5 000 entries (MAX_ENTRIES).

**Candidates.** `rusqlite` 0.41.1 (2026-10-28, ~151M dl, MIT, bundles SQLite C);
`redb` 4.5.0 (2026-11-05, ~16M dl, MIT/Apache, cberner, pure-Rust ACID KV);
`fjall` 3.1.13 (2026-10-25, ~2.1M dl, MIT/Apache, LSM KV).

**Same result?** Speed is not the constraint at this size: a 5 000-row JSON index
deserializes in milliseconds and is written debounced. JSON keeps the library human-
inspectable and the sync payload shape unchanged, and blob-per-file maps 1:1 to pasteboard
restore. SQLite adds C compilation and migrations for no user-visible win; redb/fjall add
binary formats and (fjall) compaction concerns. There are no users yet, so we *could*
switch — there's just nothing to gain today.

**Recommendation: keep ours.** First move if clipboard-history search ever needs indexes:
rusqlite. Not now.

## 15. File watching

**Today.** Nothing watches the filesystem: Espanso import is a one-shot dialog pick,
settings live in our own files, iCloud sync is CKSyncEngine (not FS-based). Verified by
grep — no `notify`/FSEvents/kqueue usage in the tree.

**Candidates.** `notify` 9.0.0-rc.8 (2026-11-09, CC0-1.0, notify-rs; 8.x is the stable
line).

**Recommendation: nothing to do** — don't adopt. If we later want live watching of an
Espanso config folder (Espanso itself does this; we don't), `notify` 8/9 (FSEvents backend)
is the right crate.

## 16. CloudKit sync

**Today.** `objc2-cloud-kit` 0.3 (CKSyncEngine event set, block2) in `src-tauri/src/cloud/`
+ `src-tauri/crates/kudzu-core/src/sync.rs` (pure merge model, SYNC_VERSION, add-only
Production schema per RELEASING.md).

**Candidates.** None exist. objc2-cloud-kit 0.3.2 (2025-10-04, Zlib OR Apache-2.0 OR MIT,
madsmtm) is the only maintained Rust access to CloudKit; it tracks the objc2 0.6 family we
already pin (verified compatible in our own Cargo.toml).

**Recommendation: keep ours** — objc2 is still the right (and only) path.

## 17. macOS system access (AX, CGEventTap, icons, Secure Input)

**Today.** `kudzu-engine/src/macos/`: `ffi.rs` hand-declares CGEventTap/CGEventPost/AXUIElement/
TISInputSource/UCKeyTranslate externs; `ax.rs` does permissions, Secure Input detection
(undocumented session dict PID) and caret/selection queries over core-foundation with a
0.2 s messaging timeout; `tap.rs` runs the event tap; `apps.rs` does NSWorkspace
frontmost-app + icon rasterization; `input_source.rs` keyboard-layout translation.

**Candidates.** `core-graphics` 0.25.0 (2025-05-27, MIT/Apache, servo) — maintained CGEvent/
CGEventTap/posting bindings; `accessibility-sys` 0.2.0 (2025-03-22, ~381k dl, MIT/Apache,
eiz) — AXUIElement FFI; both would replace chunks of `ffi.rs`. Nothing covers the Carbon
HIToolbox calls (TIS, UCKeyTranslate) — those stay hand-written under any plan.
`objc2-app-kit` (NSWorkspace/NSRunningApplication) has no alternative and stays.

**Same result?** Yes — core-graphics and accessibility-sys expose the same C functions we
hand-declared; swapping is mechanical. The win is shedding ~200 lines of our own unsafe
declarations; the cost is two small servo/eiz dependencies and a slightly stale
core-graphics release cadence.

**Recommendation: keep ours** for the rewrite (it works and is tested); optionally tidy up
later by moving the CGEvent externs to `core-graphics@0.25` and AX to
`accessibility-sys@0.2`. Not urgent.

## 18. Colour conversion and contrast (colour clips)

**Today.** `kudzu-core/src/clipboard.rs::parse_color` validates/normalizes CSS colour
strings for classification (hand-written, tests pin classification). The card itself is UI
code: `components/app/clipboard/clip-content.tsx` parses any CSS colour through a canvas
`fillStyle` trick, then hand-coded HSL and Björn Ottosson's OKLab/OKLCH matrices and WCAG
contrast ratios. All of this needs a Rust home in the rewrite.

**Candidates.** `csscolorparser` 0.9.1 (2026-11-10, ~36M dl, MIT/Apache, mazznoer) —
parses every CSS Color 4 form (`#hex`, `rgb()`/`hsl()`/`hwb()`/`lab()`/`lch()`/`oklab()`/
`oklch()`, named colours) to RGBA; `palette` 0.7.7 (2026-08-02, ~39M dl, MIT/Apache,
Ogeon) — correct colour-space conversions incl. OkLab/OkLch and WCAG relative
luminance/contrast.

**Same result?** Yes, better: csscolorparser replaces the canvas hack with real CSS
semantics; palette replaces the hand matrices with tested ones. Expect small rounding
differences in the displayed strings (our TS rounds to 3/1 decimals — keep that formatting
ours). `parse_color`'s classification behaviour changes slightly if swapped too (stricter
CSS grammar); classification tests pin the current shapes, so either re-baseline or keep
`parse_color` as the classifier and use the crates for the card.

**Recommendation: adopt csscolorparser@0.9 + palette@0.7** for the colour card; keep
`parse_color` for classification (or re-baseline its tests onto csscolorparser later).

## 19. Syntax highlighting / language detection for code clips; link detection

**Today.** No highlighting exists: `kudzu-core/src/clipboard.rs::looks_like_code` is a
cheap classification heuristic (bundle-id list + keyword scoring, tests pin it), and the UI
renders code clips as plain monospace. Link detection is a small hand-written `is_url`
sniff in the same file.

**Candidates.**
- `rune-highlights` 0.0.1 (in the Rune workspace, `rune/crates/rune-highlights`): Pierre's
  Highlights translated from WebAssembly to Rust **at build time** — 80 languages, same
  tokens OpenCode's desktop app draws with, runtime dependency only `unicode-ident`,
  MIT AND Apache-2.0. Ships with the UI framework.
- `syntect` 5.3.0 (2025-09-27, MIT, trishume) — Sublime syntax engine; pulls regex
  machinery and syntax data.
- `tree-sitter-highlight` 0.27.2 (2026-11-07, MIT) — needs one grammar crate per language
  (C builds) to get anything done.
- `linkify` 0.11.0 (2026-04-12, ~5.6M dl, MIT/Apache) — URL/email finding with careful
  boundaries.

**Same result?** Highlighting is a new capability, not a replacement — so nothing to
reproduce. `looks_like_code` stays as the cheap classifier regardless (crates highlight,
they don't detect "is this code"). If inline link ranges are needed in the Rust UI,
linkify beats growing our sniff.

**Recommendation: adopt rune-highlights** (free with the framework — no new dependency at
all) for code clips; keep `looks_like_code` and the URL sniff; take `linkify` only if the
Rune UI needs link ranges we don't already compute.

## 20. Async/threads, errors, app dirs, plist, hashing, UUIDs

**Today.** tokio 1 (only `time` + `sync` features, pulled in by tauri) is barely used;
everything else is plain `std::thread` + `parking_lot` 0.12 mutexes (clipboard poll loop,
agent socket, updater state). Errors: `thiserror` 2 in core. App dirs come from Tauri's
resolver. `plist` 1 in the engine (Info.plist reads); kudzu-core hand-writes an XML-plist
subset and an RFC-4180 CSV subset for TextExpander import. Dedup fingerprint: hand-rolled
FNV-1a u64, persisted as hex in `history.json`. UUIDs via `uuid` 1.

**Candidates / notes.**
- Async: **drop tokio**. Plain threads + std mpsc cover every loop we have. If a
  select-style channel is ever needed: `crossbeam-channel` 0.5.17 (2026-09-05, MIT/Apache)
  or `flume` 0.12.0 (2025-12-08, Apache/MIT). Keep `parking_lot` (bump 0.12 → 0.13.0,
  2026-10-21, MIT/Apache, Amanieu). Watch: adopting `rmcp` (concern 2) would drag tokio
  back in — one more reason to keep our MCP server.
- Errors: keep `thiserror` 2.0.21 (2026-09-23); `anyhow` 1.0.104 (2026-07-18) only at the
  thin app/CLI layer if convenient.
- App dirs: `dirs` 7.0.0 (2026-09-05, MIT/Apache) — `data_dir().join(identifier)`
  reproduces Tauri's `app_data_dir` for macOS (`directories` 6.0.0 is the heavier sibling;
  either works).
- plist: keep `plist` 1.10.3 (2026-10-21, MIT) in the engine; keep core's hand readers
  (bounded, XML-only by design, binary plists already rejected by callers).
- Hashing: keep FNV-1a — it's 30 lines, persisted in the index, and dedupe folding doesn't
  need cryptographic strength. If it ever does: `blake3` 1.8.7 (2026-08-20, CC0/Apache).
  `xxhash-rust` 0.8.19 is **BSL-1.0** — note the license if it's ever considered.
- UUIDs: keep `uuid` 1.29.2 (2026-11-09).

**Recommendation: keep ours; adopt dirs@7** to replace the Tauri path resolver. Drop tokio.

## 21. Other things a crate does better

- **File dialogs** (tauri-plugin-dialog today): `rfd` 0.18.0 (2026-11-07, ~44M dl, MIT,
  PolyMeilex) — native open/save dialogs, no Tauri. **Adopt.**
- **Opening URLs/files** (tauri-plugin-opener today): `open` 5.4.5 (2026-10-22, ~70M dl,
  MIT, Byron/open-rs). **Adopt.**
- **Base64**: shell uses `base64` 0.22 (latest 0.23.1, 2026-08-04); core hand-rolls strict
  base64 in `rich/image.rs` to stay dependency-light and bounded. Keep the hand one in
  core (tested, hostile-input safe); the shell can keep its crate.
- **CSV** (TextExpander import, `textexpander/csv.rs`): the `csv` crate 1.4.0 (2025-10-17,
  Unlicense/MIT, BurntSushi) is the maintained wheel, but our reader adds delimiter
  sniffing and stays panic-free on hostile input; import path only. Keep ours.
- **Tray / windows / notifications**: `rune-mac` + `rune-notify` exist in the Rune
  workspace; the Rune audit owns this — no third-party crate proposal from here.
- **Updater HTTP client** — covered in concern 7 (`ureq` 3.5.1).

---

## Summary table

| # | Concern | Recommendation | Crate@version | Risk |
|---|---|---|---|---|
| 1 | Secret detection / MCP redaction | keep ours | — (mine gitleaks/ripsecrets patterns for PREFIXES) | none; scanner drift is on us |
| 2 | MCP server | keep ours | — (rmcp@3.9.0 only if spec churn hurts) | protocol revisions must be tracked manually |
| 3 | Fuzzy search | replace matcher, keep ranking | frizbee@0.13.0 (alt: nucleo-matcher@0.3.1, MPL-2.0) | scoring behaviour shifts near ties; calibrate on port |
| 4 | Global hotkeys | adopt (unless Rune provides) | global-hotkey@0.10.0 | none; verify Rune audit first |
| 5 | Login item | adopt SMAppService | objc2-service-management@0.3.2 (alt: auto-launch@0.6.0) | `--hidden` needs in-app handling |
| 6 | Single instance | keep ours | — (unix socket + NSRunningApplication) | none; ~50 lines |
| 7 | Auto-update | replace with own small updater | minisign-verify@0.3.0 + ureq@3.5.1 | we own download/install edge cases |
| 8 | Logging | adopt tracing stack | tracing@0.1.44 + -subscriber@0.3.23 + -appender@0.2.5 + tracing-oslog@0.3.0 | tracing-oslog single maintainer, Zlib |
| 9 | Date/time ICU patterns | replace formatter | icu_datetime@2.4.0 (icu@2.4.0 umbrella) | locale outputs re-baselined (better CLDR); shared with rune-i18n |
| 10 | YAML (Espanso) | keep ours, bump | yaml-rust2@0.13.0 | low; same API lineage |
| 11 | RTF/HTML readers + HTML generation | keep ours | — (rtf-parser@0.5.0 if it grows pictures) | hostile-input caps and Dropped warnings must stay |
| 12 | Image decode/resize/PNG | keep ours (AppKit) | — (image@0.25.10 if cross-platform needed) | none |
| 13 | Clipboard rich types | keep ours | — (objc2-app-kit@0.3.2 NSPasteboard) | none |
| 14 | Persistence | keep ours | — (rusqlite@0.41.1 only if indexed search needed) | none |
| 15 | File watching | nothing to do | — (notify@9-rc if ever) | n/a |
| 16 | CloudKit | keep ours | objc2-cloud-kit@0.3.2 (bump along objc2@0.6.5) | none; only option |
| 17 | AX/CGEventTap/icons/Secure Input | keep ours (optional tidy later) | — (core-graphics@0.25.0 + accessibility-sys@0.2.0) | hand-rolled externs must stay correct |
| 18 | Colour conversion/contrast | adopt for the colour card | csscolorparser@0.9.1 + palette@0.7.7 | minor rounding/formatting diffs |
| 19 | Syntax highlighting | adopt (ships with Rune) | rune-highlights@0.0.1 | none; new capability |
| 20 | Async/errors/dirs/plist/hash/uuid | keep ours; drop tokio | + dirs@7.0.0; parking_lot@0.13.0 | rmcp would reintroduce tokio |
| 21 | Dialogs / opener / base64 / CSV | adopt dialogs + opener; keep rest | rfd@0.18.0, open@5.4.5 | rfd pulls platform dialog backends |
