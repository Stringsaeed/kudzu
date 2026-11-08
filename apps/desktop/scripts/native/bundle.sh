#!/usr/bin/env bash
# Builds the native (Rune) Kudzu.app, the bundle `tauri build` made for the webview app.
#
#   scripts/native/bundle.sh                 Kudzu Dev.app (sh.saeed.kudzu.dev), this Mac's arch, ad-hoc signed
#   scripts/native/bundle.sh --release       Kudzu.app (sh.saeed.kudzu)
#   scripts/native/bundle.sh --release --universal
#   SIGN_IDENTITY="Developer ID Application: …" scripts/native/bundle.sh --release
#
# Output: src-tauri/target/native/<Name>.app. The executable keeps the Tauri build's name
# (`kudzu-desktop`), because agents registered with `claude mcp add` point at that path.
# iCloud: with src-tauri/Kudzu.provisionprofile present, a release build embeds it and signs with
# Kudzu.icloud.entitlements (restricted entitlements only ever apply with a profile).
set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT=$PWD
RELEASE=0
UNIVERSAL=0
for arg in "$@"; do
  case $arg in
    --release) RELEASE=1 ;;
    --universal) UNIVERSAL=1 ;;
    *) echo "unknown option $arg" >&2; exit 2 ;;
  esac
done

VERSION=$(node -p "require('./package.json').version")
if [[ $RELEASE == 1 ]]; then
  NAME=Kudzu
  ID=sh.saeed.kudzu
else
  NAME="Kudzu Dev"
  ID=sh.saeed.kudzu.dev
fi
EXE=kudzu-desktop
OUT=$ROOT/src-tauri/target/native
APP="$OUT/$NAME.app"
PROFILE=$ROOT/src-tauri/Kudzu.provisionprofile
TEMP=$(mktemp -d)
trap 'rm -rf "$TEMP"' EXIT

build() {
  local target=$1
  (cd src-tauri && cargo build --release -p kudzu-app ${target:+--target "$target"}) >&2
}

echo "==> $NAME $VERSION"
if [[ $UNIVERSAL == 1 ]]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  build aarch64-apple-darwin
  build x86_64-apple-darwin
  BIN=$TEMP/kudzu
  lipo -create -output "$BIN" \
    src-tauri/target/aarch64-apple-darwin/release/kudzu \
    src-tauri/target/x86_64-apple-darwin/release/kudzu
else
  build ""
  BIN=src-tauri/target/release/kudzu
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/skills/kudzu"
cp "$BIN" "$APP/Contents/MacOS/$EXE"
cp src-tauri/icons/icon.icns "$APP/Contents/Resources/icon.icns"
cp skills/kudzu/SKILL.md "$APP/Contents/Resources/skills/kudzu/SKILL.md"

# Info.plist: what Tauri generated from tauri.conf.json, plus src-tauri/Info.plist's keys.
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>$NAME</string>
  <key>CFBundleExecutable</key>
  <string>$EXE</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundleIdentifier</key>
  <string>$ID</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>© 2026 Muhammed Ibrahim</string>
$(sed -n '/<dict>/,/<\/dict>/p' src-tauri/Info.plist | sed '1d;$d')
</dict>
</plist>
PLIST
plutil -lint "$APP/Contents/Info.plist" >/dev/null

SIGN_IDENTITY=${SIGN_IDENTITY:--}
SIGN_ARGS=(--force --options runtime)
if [[ $SIGN_IDENTITY == - ]]; then
  SIGN_ARGS+=(--timestamp=none)
else
  SIGN_ARGS+=(--timestamp)
fi
if [[ -n "${SIGN_KEYCHAIN:-}" ]]; then
  SIGN_ARGS+=(--keychain "$SIGN_KEYCHAIN")
fi
if [[ $RELEASE == 1 && -f $PROFILE ]]; then
  [[ $SIGN_IDENTITY != - ]] || { echo "error: iCloud requires a Developer ID signature, not ad hoc signing" >&2; exit 1; }
  cp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
  SIGN_ARGS+=(--entitlements src-tauri/Kudzu.icloud.entitlements)
  echo "==> iCloud: on"
fi
# All native dependencies are statically linked; the only nested code is the main executable.
# Sign inside out rather than using --deep to sign (which can mask missing nested signatures).
codesign "${SIGN_ARGS[@]}" --sign "$SIGN_IDENTITY" "$APP/Contents/MacOS/$EXE"
codesign "${SIGN_ARGS[@]}" --sign "$SIGN_IDENTITY" "$APP"
codesign --verify --deep --strict "$APP"
echo "$APP"
