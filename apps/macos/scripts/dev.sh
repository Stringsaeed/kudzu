#!/usr/bin/env bash
# Builds the Debug app (Kudzu Dev, sh.saeed.kudzu.dev) into apps/macos/build/ and launches it,
# quitting a running copy first. Extra arguments go to the app (e.g. `scripts/dev.sh --hidden`).
set -euo pipefail

cd "$(dirname "$0")/.."
# xcodebuild needs the full Xcode, not the Command Line Tools.
if [[ -z "${DEVELOPER_DIR:-}" ]] && ! xcode-select -p 2>/dev/null | grep -q "\.app/"; then
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

app="build/Build/Products/Debug/Kudzu Dev.app"

xcodebuild -project Kudzu.xcodeproj -scheme Kudzu -configuration Debug -derivedDataPath build build -quiet

# Quit a running Kudzu Dev (it has no Dock menu while hidden), then wait for it to exit.
if pkill -x "Kudzu Dev" 2>/dev/null; then
  for _ in {1..50}; do pgrep -x "Kudzu Dev" >/dev/null || break; sleep 0.1; done
fi

open -n "$app" --args "$@"
echo "Launched $app"
