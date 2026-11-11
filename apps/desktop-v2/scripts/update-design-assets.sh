#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
GENERATED=$(mktemp -d)
trap 'rm -rf "$GENERATED"' EXIT
mkdir -p "$GENERATED/scripts" "$GENERATED/assets/tray" "$GENERATED/assets/icons"
cp -R design "$GENERATED/design"
cp scripts/{brand-assets.py,design-assets.swift} "$GENERATED/scripts/"
cd "$GENERATED"
python3 scripts/brand-assets.py
cmp design/app-icon/layers/02-echo-k-printed.svg design/app-icon/Kudzu.icon/Assets/02-echo-k-printed.svg || { echo 'Import the regenerated icon layer in Icon Composer first.' >&2; exit 1; }
ICON_TOOL=${ICON_COMPOSER_TOOL:-/Applications/Xcode.app/Contents/Applications/Icon Composer.app/Contents/Executables/ictool}
"$ICON_TOOL" "$PWD/design/app-icon/Kudzu.icon" --export-image --output-file "$PWD/design/app-icon/Kudzu-1024.png" --platform macOS --rendition Default --width 1024 --height 1024 --scale 1
swift -module-cache-path "${TMPDIR:-/tmp}/kudzu-design-module-cache" scripts/design-assets.swift
mkdir icon.iconset
for points in 16 32 128 256 512; do
  sips -z "$points" "$points" design/icon.png --out "icon.iconset/icon_${points}x${points}.png" >/dev/null
  pixels=$((points * 2))
  sips -z "$pixels" "$pixels" design/icon.png --out "icon.iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil -c icns icon.iconset -o assets/icons/icon.icns
mkdir -p "$ROOT/assets/icons" "$ROOT/assets/tray"
cp assets/icons/icon.icns "$ROOT/assets/icons/icon.icns"
cp assets/tray/*.png "$ROOT/assets/tray/"
cd "$ROOT"
python3 scripts/check-design-assets.py --write --renderer-version "$("$ICON_TOOL" --version)"
