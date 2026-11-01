# Kudzu — agent guide

Native macOS text expander. Read `docs/PLAN.md` (decisions), `docs/ARCHITECTURE.md` (layout and flow) and `docs/TEMPLATE_SYNTAX.md` (snippet template format) before changing behaviour.

## Layout

- `Packages/KudzuCore`: pure Swift package with no AppKit/UIKit imports, so it is reusable on iOS. All logic that can be unit-tested lives here.
- `Kudzu/`: thin macOS app (menu bar, windows, panels, event tap, permissions).
- `project.yml`: XcodeGen spec. `Kudzu.xcodeproj` is generated and git-ignored, so **never edit it**.
  - `Kudzu` target: Developer ID build, unsandboxed.
  - `KudzuAppStore` target: sandboxed, `APPSTORE` compilation condition.

## Commands

```sh
# Core tests (fast; run these constantly)
cd Packages/KudzuCore && swift test

# App builds
xcodegen generate
xcodebuild -project Kudzu.xcodeproj -scheme Kudzu -derivedDataPath build/DerivedData build
xcodebuild -project Kudzu.xcodeproj -scheme KudzuAppStore -derivedDataPath build/DerivedData build
```

Signing defaults to ad-hoc (`Config/Signing.xcconfig`). Ad-hoc signatures change every build, so macOS forgets privacy grants. To keep Accessibility and Input Monitoring grants across rebuilds, create `Config/Local.xcconfig` from the `.example` file with your own team.

The toolchain is Xcode 27 / Swift 6.4, with Swift 6 language mode and strict concurrency. The deployment target is macOS 15.

## Conventions

- **Tests:** Swift Testing (`import Testing`, `@Test`, `#expect`), not XCTest. Write tests first for core logic.
- **Shared value types** live in `KudzuCore/Shared/Interchange.swift`: `SnippetRecord`, `GroupRecord`, `UsageRecord`, `AppRule`, `KudzuLibrary` and the enums. Treat them as a contract.
  - Additive changes only.
  - New fields must be optional or defaulted so old JSON still decodes.
- **SwiftData models** must stay CloudKit-compatible:
  - every attribute optional or defaulted
  - relationships optional
  - no `@Attribute(.unique)` / `#Unique`
  - no `.deny` delete rule
- Public API gets a one-line doc comment. Explain the *why* in other comments, sparingly.
- No third-party dependencies beyond those already in `Package.swift` without asking.
- Keep each change scoped to your task's directories. Don't reformat unrelated files.
