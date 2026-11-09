#!/usr/bin/env bash
# Re-exports every derived design asset, then records the result in design/assets-manifest.json.
# Edit Kudzu/Kudzu.icon in Icon Composer and the sources in design/ first; run this on the design
# Mac (needs Xcode for Icon Composer's ictool). Nothing here needs Node, Tauri or Homebrew:
# SVGs are rasterised by AppKit (scripts/design/design-assets.swift).
set -euo pipefail
cd "$(dirname "$0")/../.."

ICON_TOOL=${ICON_COMPOSER_TOOL:-/Applications/Xcode.app/Contents/Applications/Icon Composer.app/Contents/Executables/ictool}
[[ -x $ICON_TOOL ]] || { echo "error: Icon Composer's ictool not found at $ICON_TOOL (set ICON_COMPOSER_TOOL)" >&2; exit 1; }

# 1. Derived vector art: the printed k for the icon and the menu-bar glyph SVGs.
python3 scripts/design/brand-assets.py
# 2. The k inside Kudzu.icon must be the one just generated (Image > Replace in Icon Composer).
python3 scripts/design/check-design-assets.py --native-only
# 3. The icon as macOS renders it.
"$ICON_TOOL" "$PWD/Kudzu/Kudzu.icon" --export-image --output-file "$PWD/design/app-icon/Kudzu-1024.png" \
  --platform macOS --rendition Default --width 1024 --height 1024 --scale 1
# 4. Rasters: wordmark, padded icon, DMG background, menu-bar glyphs, the website's icon.
swift -module-cache-path "${TMPDIR:-/tmp}/kudzu-design-module-cache" scripts/design/design-assets.swift
# 5. The website serves the full wordmark as-is.
cp design/wordmark/kudzu-wordmark-echo.svg ../website/public/assets/kudzu-wordmark.svg
# 6. Record sources and outputs.
python3 scripts/design/check-design-assets.py --write --renderer-version "$("$ICON_TOOL" --version)"
