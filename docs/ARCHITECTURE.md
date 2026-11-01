# Kudzu — Architecture

This document is planned design: no code exists yet. Decisions and the reasons behind them live in [`PLAN.md`](PLAN.md).

## Layout

```
kudzu/
├── project.yml              # XcodeGen spec (two schemes: Direct, AppStore)
├── Packages/
│   └── KudzuCore/           # Swift package — no AppKit/UIKit, reusable on iOS
│       ├── Sources/KudzuCore/
│       │   ├── Models/      # SwiftData @Model types (CloudKit-safe)
│       │   ├── Matching/    # TriggerIndex, KeyBuffer, CasePropagation, WordBoundary
│       │   ├── Templates/   # Parser → AST → Renderer (date, clipboard, cursor, nested, form)
│       │   ├── Store/       # ModelContainer setup, dedupe, change observation
│       │   ├── Stats/       # Per-device usage records, aggregation
│       │   └── Interchange/ # Kudzu JSON + Espanso YAML import/export, backups
│       └── Tests/KudzuCoreTests/
├── Kudzu/                   # macOS app target (thin)
│   ├── App/                 # @main, menu-bar scene, main window scene
│   ├── Engine/              # EventTap, Inserter (type/paste), SecureInputMonitor, FrontmostApp
│   ├── Panels/              # SearchPalette, FormPanel (NSPanel + SwiftUI), caret locator (AX)
│   ├── Editor/              # Sidebar, list, chip editor, rules UI
│   ├── Onboarding/
│   └── Settings/
└── docs/
```

## Runtime flow

```
CGEventTap (keyDown)
   │  ignore if paused, Secure Input on, or app excluded
   ▼
KeyBuffer ── rolling last-N chars, reset on click/app switch/navigation keys
   │
   ▼
TriggerIndex.match(buffer, frontmostBundleID)    ← in-memory, rebuilt on store changes
   │  respects word mode + effective app rules (snippet ▸ group ▸ global)
   ▼
TemplateRenderer.render(snippet)                 ← may need a form →
   │                                               FormPanel at caret; Esc aborts
   ▼
Inserter
   ├─ delete trigger (backspaces)
   ├─ short plain → synthesized Unicode keystrokes
   └─ long/rich  → pasteboard save → transient write → ⌘V → restore
   │  then move caret to the {{cursor}} position if present
   ▼
UndoTracker (backspace right after → restore trigger)
StatsRecorder (+1 on this device's record; no content recorded)
```

## Data model (sketch)

All attributes are optional or have defaults, and relationships are optional. There are no unique constraints because CloudKit doesn't support them.

- `Snippet`: id, name, content (template source), triggers `[String]`, wordMode, caseMode, insertionOverride, isFavorite, isEnabled, createdAt, updatedAt, group?, rules?
- `SnippetGroup`: id, name, sortIndex, isEnabled, parent?, children, snippets, rules?
- `AppRule`: mode (allow/deny), bundleIDs `[String]`, insertionOverride
- `UsageRecord`: snippetID, deviceID, count, charsSaved, lastUsedAt. **One per (snippet, device)**, summed for display.
- `SyncedSettings`: e.g. typing-speed WPM, default prefix
- Local-only configuration: UserDefaults for now (palette shortcut, form memory, the Settings → Apps exclusions and per-app insertion). App settings move to a synced store in M4.

## Sync

- `ModelContainer` with two `ModelConfiguration`s:
  - **synced:** `cloudKitDatabase: .private("iCloud.sh.saeed.kudzu")`, or `.none` when sync is off.
  - **local:** always `.none`.
- **Dedupe pass** runs after remote changes (via persistent history) and after imports. Snippets with identical normalized triggers and content are merged, keeping the oldest id and summing its usage records.
- **Status UI** comes from `NSPersistentCloudKitContainer.eventChangedNotification` on the underlying stack.
- **iOS later:** the store lives in an App Group container. The keyboard extension opens it read-only with `cloudKitDatabase: .none`, and only the main app syncs.

## Build configurations

| | Direct | AppStore |
|---|---|---|
| Sandbox | off | on |
| Sparkle | linked | excluded (`#if !APPSTORE`) |
| Espanso import | direct path read | `NSOpenPanel` security-scoped bookmark |
| Entitlements | iCloud + CloudKit | iCloud + CloudKit + sandbox (user-selected files read-write) |

## Permissions

| Permission | Why |
|---|---|
| Input Monitoring | `CGEventTap` listen |
| Accessibility | Post events, read the focused element / caret bounds for panel placement |

Onboarding polls `CGPreflightListenEventAccess` / `AXIsProcessTrusted` and advances when each permission is granted.

## Testing

- **`KudzuCore`:** test-first. Golden tests cover Espanso import fixtures. Table tests cover matching, word boundaries, case propagation and template rendering.
- **App layer:** the M0 spike doubles as a manual compatibility matrix (TextEdit, Safari, Chrome, VS Code, Slack, Terminal, iTerm, Notes), recorded in `docs/compat.md`.
