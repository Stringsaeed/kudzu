#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
harness="$root/build/settings-modal-tests"
mkdir -p "$harness/Sources/SettingsModalSupport" "$harness/Tests/SettingsModalTests"
cat > "$harness/Package.swift" <<'SWIFT'
// swift-tools-version: 6.0
import PackageDescription
let package = Package(
    name: "SettingsModalTests",
    platforms: [.macOS(.v15)],
    targets: [
        .target(name: "SettingsModalSupport"),
        .testTarget(name: "SettingsModalTests", dependencies: ["SettingsModalSupport"])
    ]
)
SWIFT
cp "$root/Kudzu/Settings/SettingsModalFocusSession.swift" "$harness/Sources/SettingsModalSupport/"
cp "$root/Tests/SettingsModal/"*.swift "$harness/Tests/SettingsModalTests/"
swift test --package-path "$harness" "$@"
