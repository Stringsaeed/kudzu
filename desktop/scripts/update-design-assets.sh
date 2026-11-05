#!/usr/bin/env bash
# Edit Kudzu.icon in Icon Composer; this script exports derived assets.
set -euo pipefail
cd "$(dirname "$0")/.."
GENERATED=$(mktemp -d)
trap 'rm -rf "$GENERATED"' EXIT
python3 scripts/brand-assets.py
python3 scripts/check-design-assets.py --native-only
mkdir -p public/brand
cp design/wordmark/*.svg public/brand/
pnpm exec tauri icon design/wordmark/kudzu-wordmark-echo-small.svg --fit contain --png 741 --output "$GENERATED/wordmark"
cp "$GENERATED/wordmark/741x741.png" design/wordmark/kudzu-wordmark-echo-small.png
ICON_TOOL=${ICON_COMPOSER_TOOL:-/Applications/Xcode.app/Contents/Applications/Icon Composer.app/Contents/Executables/ictool}
"$ICON_TOOL" "$PWD/design/app-icon/Kudzu.icon" --export-image --output-file "$PWD/design/app-icon/Kudzu-1024.png" --platform macOS --rendition Default --width 1024 --height 1024 --scale 1
swift -module-cache-path "${TMPDIR:-/tmp}/kudzu-design-module-cache" scripts/design-assets.swift
pnpm exec tauri icon design/icon.png --output "$GENERATED/app"
for target in src-tauri/icons/*; do
  name=${target##*/}
  if [[ $name != tray* && -f "$GENERATED/app/$name" ]]; then cp "$GENERATED/app/$name" "$target"; fi
done
cp "$GENERATED/app/128x128@2x.png" public/icon.png
cp "$GENERATED/app/icon.ico" app/favicon.ico
for state in normal paused permissions secure-input; do
  suffix="-$state"
  [[ $state == normal ]] && suffix=""
  source="design/menu-bar/kudzu-menubar$suffix"
  pnpm exec tauri icon "$source.svg" --png 18 --png 36 --png 54 --output "$GENERATED/$state"
  cp "$GENERATED/$state/18x18.png" "$source.png"
  cp "$GENERATED/$state/36x36.png" "$source@2x.png"
  cp "$GENERATED/$state/54x54.png" "$source@3x.png"
  case "$state" in
    normal) target=tray ;;
    secure-input) target=tray-secure ;;
    *) target="tray-$state" ;;
  esac
  cp "$GENERATED/$state/36x36.png" "src-tauri/icons/$target.png"
done
python3 scripts/check-design-assets.py --write --renderer-version "$("$ICON_TOOL" --version)"
