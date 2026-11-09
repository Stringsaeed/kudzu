// swift-tools-version:6.0
// KudzuKit: every module of the native Kudzu app. The Xcode project in `apps/macos` only adds the
// app entry point, Info.plist, entitlements and assets. See `apps/macos/docs/MIGRATION.md`.
import PackageDescription

let package = Package(
    name: "KudzuKit",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "KudzuApp", targets: ["KudzuApp"]),
        .library(name: "KudzuCore", targets: ["KudzuCore"]),
    ],
    targets: [
        // Pure logic: models, templates, matching, interchange, secrets, sync, agent, search. Foundation only.
        .target(name: "KudzuCore", resources: [.copy("Resources")]),
        // System integration: event tap, insertion, pasteboard, permissions, apps, caret.
        .target(name: "KudzuEngine", dependencies: ["KudzuCore"]),
        // Stores, backups, clipboard history, hotkeys, login item, logging, updater, CloudKit, MCP socket.
        .target(name: "KudzuServices", dependencies: ["KudzuCore", "KudzuEngine"]),
        // Design tokens, depth, motion, materials, native control styles, custom views, SF Symbol icons.
        .target(name: "KudzuDesign", dependencies: ["KudzuCore"]),
        // NSTextView template editor with chips and formatting.
        .target(name: "KudzuEditor", dependencies: ["KudzuCore", "KudzuDesign"]),
        // AppModel, features and the AppKit shell.
        .target(name: "KudzuApp", dependencies: ["KudzuCore", "KudzuEngine", "KudzuServices", "KudzuDesign", "KudzuEditor"]),

        // Fixtures/ holds the golden JSON written by kudzu-core's `export_fixtures` example.
        .testTarget(name: "KudzuCoreTests", dependencies: ["KudzuCore"], resources: [.copy("Fixtures")]),
        .testTarget(name: "KudzuEngineTests", dependencies: ["KudzuEngine"]),
        .testTarget(name: "KudzuServicesTests", dependencies: ["KudzuServices"]),
        .testTarget(name: "KudzuDesignTests", dependencies: ["KudzuDesign"], resources: [.copy("Resources")]),
        .testTarget(name: "KudzuEditorTests", dependencies: ["KudzuEditor"]),
        .testTarget(name: "KudzuAppTests", dependencies: ["KudzuApp", "KudzuServices"]),
    ],
    swiftLanguageModes: [.v6]
)
