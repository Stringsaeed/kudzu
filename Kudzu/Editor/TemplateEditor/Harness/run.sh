#!/bin/zsh
# Builds and runs the editor harness: behaviour checks, plus PNG renders when given `--out DIR`.
# Usage (from the repo root, after `xcodebuild … -scheme Kudzu -derivedDataPath build/DerivedData build`):
#   Kudzu/Editor/TemplateEditor/Harness/run.sh [--out build/editor-renders]
set -euo pipefail
root=$(cd "$(dirname "$0")/../../../.." && pwd)
products="$root/build/DerivedData/Build/Products/Debug"
work="$root/build/editor-harness"
mkdir -p "$work"
swiftc -swift-version 6 -parse-as-library -D EDITOR_HARNESS -Onone -o "$work/EditorHarness" \
  -I "$products" -I "$root/build/DerivedData/SourcePackages/checkouts/Yams/Sources/CYaml/include" \
  "$root"/Kudzu/Editor/TemplateEditor/**/*.swift \
  "$products/KudzuCore.o" "$products/KudzuUI.o" "$products/Yams.o" "$products/CYaml.o"
# KudzuUI looks for its resource bundle (fonts) next to the executable.
ln -sfn "$products/KudzuUI_KudzuUI.bundle" "$work/KudzuUI_KudzuUI.bundle"
"$work/EditorHarness" "$@"
