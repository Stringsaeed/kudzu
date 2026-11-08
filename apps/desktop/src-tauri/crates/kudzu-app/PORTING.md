# Porting Kudzu's UI to Rune

`kudzu-app` is Kudzu as a native app: [Rune](https://github.com/anomalyco/rune) (a GPU-rendered
Rust UI) over the Tauri shell's Rust. The Tauri + Next.js app stays in place until this one is at
parity; then it goes.

**The bar is parity.** Same layout, spacing, type, colours, gradients, shadows, motion, hover and
press feedback, focus behaviour, keyboard shortcuts, drag and drop, empty states, copy and
error messages. Nothing is dropped or simplified. If Rune lacks something, build it here; if
you truly can't, say so explicitly in your report. Never leave it silently out.

## Read first

- `apps/desktop/README.md`: features. `apps/desktop/docs/TEMPLATE_SYNTAX.md`: templates.
- Rune docs (local clone at the pinned rev):
  `/private/tmp/claude-501/-Users-saeed--t3-worktrees-text-expander-t3-edaab808/3a95c864-ad39-46d6-b704-8f06aed4a76b/scratchpad/rune-src/rune/`.
  Read `crates/rune/README.md` (the whole API: layout gotchas, motion, text fields, rich text and
  pills, scrolling, virtual rows, drag, drop, popup menus, file dialogs, windows) and
  `crates/rune-kit/README.md` (components). Grep `crates/rune/src` and `crates/rune-kit/src`
  for exact signatures; `examples/` and `crates/rune-kit/examples/` show real usage.

## Layout

```
src/
  main.rs            --mcp bridge, logging, load, ui::run
  backend/           the Tauri shell's Rust without Tauri (store, engine host, clipboard, agent,
                     cloud, updater, permissions, windows, hotkey). `backend::commands::*` are
                     the old IPC commands, called directly. Blocking ones say so: run them on a
                     worker (`MainUi::spawn`, or a thread + `AppHandle::update`).
  ui/
    mod.rs           five windows, the Shell, appearance (`ui::theme_for(cx)`), bus wiring
    theme/           tokens (generated from app/globals.css), type scale, radii, motion, helpers
    icons.rs         lucide icons (generated: the exact glyphs the web UI imports)
    kit/             components/ui/* (shadcn) and shared components/app/*.tsx, as builders
    main/            the main window: frame, toolbar, sidebar, list, settings, insights, …
    editor/          the snippet editor
    clipboard/       clipboard history pages and the picker window
    palette.rs form.rs permissions.rs   the other windows
    native/          AppKit bits: sidebar vibrancy, tray, appearance, pointer display
```

The web's pure client logic (`lib/search.ts`, `groups.ts`, `stats.ts`, `keys.ts`,
`clipboard.ts`, `template.ts`, `formatting.ts`, …) is ported to `kudzu_core::client`; use it,
don't re-port it.

## How a screen maps

- A component file becomes a module with a `view(...) -> El` (or a builder for reusable pieces).
- React state in a component becomes fields on that module's state struct (`s.editor`,
  `s.list`, …: the field already exists on `MainUi`; add what you need to *your* struct only).
- zustand (`lib/store.ts`) is `MainUi` in `ui/main/mod.rs`: its methods (`new_snippet`,
  `save_snippet`, `request_delete`, `update_settings`, …) do what the store's did.
- `api.x()` → `crate::backend::commands::x()` (or `backend::clipboard::*`, `backend::agent::*`).
- `on(Events.x)` → `apply(state, &Event, tx)` (bus events already reach every window).
- Tailwind → tokens: `bg-background` is `t.background`, `text-muted-foreground` is
  `t.muted_foreground`, `shadow-card` is `.elevate(&t.elev_card)`, `rounded-lg` is
  `radius::LG`, `text-[13px] leading-5` is `text::BASE`, `sheen` is `.sheen(false)`,
  `press` is `.press_sink(0.97)` (with `[--press-scale:x]` use x). `px-3` is 12 pt (Tailwind's 4 pt
  unit). Read the exact classes in the TSX; don't eyeball.
- Animations: the keyframes in globals.css (`rise`, `pop`, `check-in`, `tick`, `star-burst`,
  `bar-grow`, `shimmer`, `.stagger`) map to Rune presence transitions, springs, `El::sequence`
  and loops; `theme::motion::{fast, moderate, soft, pop}` are the CSS durations and springs.
  Reduced motion is honoured by Rune.
- Every action keeps its shortcut and its tooltip/`help`. Keyboard navigation (arrow keys,
  Tab order, Escape, Return) matches the web's.
- Accessibility: give controls roles and labels as the web gave aria attributes.

## The kit

`ui/kit/` already has every shadcn control and the shared app components, each matched against
the web: `Button`, `Input`, `Textarea`, `InputGroup`, `Label`, `separator`, `Switch`,
`Checkbox`, `radio_group`, `Toggle`, `ToggleGroup`, `Slider`, `Badge`, `Kbd`, `Shortcut`,
`Item`, `breadcrumb`/`Breadcrumbs`, `Segmented`, `SelectionPill`, `trigger_chip`, `app_icon`,
`wordmark`, `FormattedText`. Read each file's builder before use. `KUDZU_GALLERY=controls`
renders them all. Controls track their own pressed look by id: in a loop with repeated labels,
give each `.id(..)`.

The overlays (tooltip, popover, dialog, alert dialog, dropdown and context menus, select,
command, scroll area, collapsible, tabs, app picker, the toast region) are being finished in
`ui/kit/` right now. Read their files for the pattern (the top of `kit/mod.rs` documents it).
If one you need isn't ready, build the rest of your screen, wire the overlay last, and come
back to it; don't write your own.

## Ownership

Edit only the files your brief names. If you need something from another module, say so in
your report rather than editing it. Shared components go in `ui/kit/` (owned by the kit work);
until one exists, ask for it or build a private one in your module and note it. Never edit
generated files (`ui/theme/tokens.rs`, `ui/icons.rs`): change the source and rerun
`node scripts/rune-tokens.mjs` / `node scripts/rune-icons.mjs` from `apps/desktop`.

Several agents work in this crate at once. A compile error in a file you don't own is someone
else's work in progress: wait a minute and retry; never "fix" it.

## Verify like a user

From `apps/desktop/src-tauri`:

```sh
cargo build -p kudzu-app
cargo test -p kudzu-app
cargo clippy -p kudzu-app --all-targets -- -D warnings

# The web UI (reference): served on :3917 by `pnpm exec next dev --port 3917` in apps/desktop
# (start it if `curl -s localhost:3917` fails). Screenshot any state with headless Chrome:
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu \
  --hide-scrollbars --force-device-scale-factor=2 --window-size=1180,760 \
  --virtual-time-budget=4000 --screenshot=web.png "http://localhost:3917/?view=insights"
# Query params: ?view=insights|clipboard|settings:apps|…, ?onboarding, ?light, ?update,
# /palette, /clipboard, /form?demo, /permissions (see apps/desktop/README.md).

# The native UI on the same data:
node --experimental-strip-types --no-warnings ../scripts/rune-fixture.mjs /tmp/kz-fixture   # once
KUDZU_DATA_DIR=/tmp/kz-fixture target/debug/kudzu --window main --snap native.png --size 1180x760
# --scene, --script (clicks, typing, keys), --tree (element tree as JSON): see "Agent tooling"
# in Rune's README. For light/dark, set "theme": "system" in the fixture's settings.json and pass
# --theme light|dark (Kudzu's own Dark/Light settings override the system appearance).
```

Put scratch files under
`/private/tmp/claude-501/-Users-saeed--t3-worktrees-text-expander-t3-edaab808/3a95c864-ad39-46d6-b704-8f06aed4a76b/scratchpad/`
(use your own subfolder), not in the repo. Compare web and native screenshots side by side
for every state you port (default, hover/selected, empty, error, light and dark), and iterate
until they match. Native views (vibrancy) don't appear in headless snapshots; that's expected.

Don't run the GUI app itself (it opens real windows on the user's screen); headless is enough.
Don't commit or stash. Don't add third-party dependencies.
