#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
harness="$root/build/editor-model-tests"
mkdir -p "$harness/Sources/EditorModels" "$harness/Tests/EditorModelsTests"
cat > "$harness/Package.swift" <<'SWIFT'
// swift-tools-version: 6.0
import PackageDescription
let package = Package(
    name: "EditorModelTests",
    platforms: [.macOS(.v15)],
    dependencies: [.package(path: "../../Packages/KudzuCore"), .package(path: "../../Packages/KudzuUI")],
    targets: [
        .target(name: "EditorModels", dependencies: [.product(name: "KudzuCore", package: "KudzuCore"), .product(name: "KudzuUI", package: "KudzuUI")]),
        .testTarget(name: "EditorModelsTests", dependencies: ["EditorModels"])
    ]
)
SWIFT
for source in LibraryModel SnippetEditorModel BrowserState; do
    cp "$root/Kudzu/Editor/$source.swift" "$harness/Sources/EditorModels/"
done
cp "$root/Tests/EditorModels/"*.swift "$harness/Tests/EditorModelsTests/"
swift test --package-path "$harness" "$@"
