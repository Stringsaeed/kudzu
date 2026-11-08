#!/usr/bin/env bash
# Make the same Finder installer as tauri.conf.json, without invoking the Tauri bundler.
# Usage: scripts/native/dmg.sh /path/to/Kudzu.app /path/to/output.dmg
set -euo pipefail

[[ $# == 2 ]] || { echo "usage: $0 APP OUTPUT.dmg" >&2; exit 2; }
APP=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
DMG=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
[[ -d $APP && $(basename "$APP") == Kudzu.app ]] || { echo "error: expected Kudzu.app" >&2; exit 1; }
[[ ! -e $DMG ]] || { echo "error: $DMG already exists" >&2; exit 1; }

# Read the shipping layout rather than maintaining a second set of coordinates.
IFS=$'\t' read -r WIDTH HEIGHT APP_X APP_Y LINK_X LINK_Y WIN_X WIN_Y BACKGROUND < <(
  node - "$ROOT/src-tauri/tauri.conf.json" <<'JS'
const path = require('node:path');
const config = process.argv[2];
const d = require(config).bundle.macOS.dmg;
console.log([d.windowSize.width, d.windowSize.height, d.appPosition.x, d.appPosition.y,
  d.applicationFolderPosition.x, d.applicationFolderPosition.y,
  d.windowPosition?.x ?? 10, d.windowPosition?.y ?? 60,
  path.resolve(path.dirname(config), d.background)].join('\t'));
JS
)

TEMP=$(mktemp -d)
MOUNT=$TEMP/mount
ATTACHED=0
cleanup() {
  local status=$?
  if [[ $ATTACHED == 1 ]]; then
    hdiutil detach "$MOUNT" -quiet || hdiutil detach "$MOUNT" -force -quiet || true
  fi
  rm -rf "$TEMP"
  exit "$status"
}
trap cleanup EXIT
mkdir -p "$TEMP/stage/.background" "$MOUNT"
ditto "$APP" "$TEMP/stage/Kudzu.app"
cp "$BACKGROUND" "$TEMP/stage/.background/background.png"
cp "$APP/Contents/Resources/icon.icns" "$TEMP/stage/.VolumeIcon.icns"
ln -s /Applications "$TEMP/stage/Applications"
# Extra room lets Finder write .DS_Store even for a small native app.
SIZE_MB=$(( $(du -sm "$TEMP/stage" | cut -f1) + 32 ))
hdiutil create -quiet -size "${SIZE_MB}m" -fs HFS+ -volname Kudzu \
  -srcfolder "$TEMP/stage" -format UDRW "$TEMP/writable.dmg"
hdiutil attach -quiet -nobrowse -noautoopen -readwrite -mountpoint "$MOUNT" "$TEMP/writable.dmg"
ATTACHED=1
xcrun SetFile -a C "$MOUNT"

# Any Finder/Automation failure is fatal: an unstyled DMG must never ship.
osascript - "$MOUNT" "$WIDTH" "$HEIGHT" "$APP_X" "$APP_Y" "$LINK_X" "$LINK_Y" "$WIN_X" "$WIN_Y" <<'APPLESCRIPT'
on run argv
  set mountPath to item 1 of argv
  set w to (item 2 of argv) as integer
  set h to (item 3 of argv) as integer
  set ax to (item 4 of argv) as integer
  set ay to (item 5 of argv) as integer
  set lx to (item 6 of argv) as integer
  set ly to (item 7 of argv) as integer
  set wx to (item 8 of argv) as integer
  set wy to (item 9 of argv) as integer
  with timeout of 120 seconds
    tell application "Finder"
      set installer to disk of ((POSIX file mountPath) as alias)
      open installer
      tell container window of installer
        set current view to icon view
        set toolbar visible to false
        set statusbar visible to false
        set bounds to {wx, wy, wx + w, wy + h}
      end tell
      set opts to icon view options of container window of installer
      tell opts
        set arrangement to not arranged
        set icon size to 128
        set text size to 16
      end tell
      set background picture of opts to file ".background:background.png" of installer
      set position of item "Kudzu.app" of installer to {ax, ay}
      set position of item "Applications" of installer to {lx, ly}
      close container window of installer
      open installer
      update installer without registering applications
      delay 2
      close container window of installer
    end tell
  end timeout
end run
APPLESCRIPT
[[ -s $MOUNT/.DS_Store ]] || { echo "error: Finder didn't save the DMG layout" >&2; exit 1; }
sync
hdiutil detach -quiet "$MOUNT"
ATTACHED=0
hdiutil convert -quiet "$TEMP/writable.dmg" -format UDZO -imagekey zlib-level=9 -o "$DMG"
hdiutil verify "$DMG"
