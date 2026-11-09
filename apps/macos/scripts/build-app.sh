#!/usr/bin/env bash
# Universal Release Kudzu.app.
#
# UNSIGNED=1 (local): ad-hoc signature, no notarization, no updater signature, no iCloud
# entitlements. The package is not a valid update for a Developer ID install.
#
# Signed (CI): Developer ID + hardened runtime, notarize and staple unless SKIP_NOTARIZE=1,
# minisign the updater archive in the Tauri double-encoded form, and apply
# Kudzu.icloud.entitlements only when Kudzu.provisionprofile matches this app.
#
# Version comes from Kudzu/Version.xcconfig. CHANNEL=beta or ALLOW_VERSION_OVERRIDE=1 may
# set VERSION for that build only; the file is not edited.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

UNSIGNED=${UNSIGNED:-0}
CHANNEL=${CHANNEL:-stable}
DERIVED_DATA=${DERIVED_DATA:-$ROOT/build}
DIST=${DIST:-$ROOT/dist}
REPO=${GITHUB_REPOSITORY:-Stringsaeed/kudzu}
REQUIRE_ICLOUD=${REQUIRE_ICLOUD:-0}
SKIP_NOTARIZE=${SKIP_NOTARIZE:-0}
PROFILE=$ROOT/Kudzu.provisionprofile
BG=$ROOT/design/dmg/background.png
XCCONFIG=$ROOT/Kudzu/Version.xcconfig

DEV_DIR=$(xcode-select -p 2>/dev/null || true)
if [[ $DEV_DIR != *.app/* && $DEV_DIR != *.app ]]; then
  [[ -d /Applications/Xcode.app/Contents/Developer ]] || die "full Xcode is required (xcode-select is $DEV_DIR)"
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

FILE_VERSION=$(sed -n 's/^MARKETING_VERSION *= *\([^ ]*\).*/\1/p' "$XCCONFIG" | head -1)
[[ -n $FILE_VERSION ]] || die "MARKETING_VERSION missing in $XCCONFIG"
[[ $(cat "$ROOT/version.txt") == "$FILE_VERSION" ]] || die "release-please version.txt and Version.xcconfig disagree"
if [[ -z ${VERSION:-} ]]; then
  VERSION=$FILE_VERSION
elif [[ $VERSION != "$FILE_VERSION" && $CHANNEL != beta && ${ALLOW_VERSION_OVERRIDE:-0} != 1 ]]; then
  die "VERSION=$VERSION overrides $XCCONFIG ($FILE_VERSION). Set CHANNEL=beta or ALLOW_VERSION_OVERRIDE=1. The file is not edited."
fi
[[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] || die "version is not semver: $VERSION"
BUILD_NUMBER=${CURRENT_PROJECT_VERSION:-${GITHUB_RUN_NUMBER:-1}}
[[ $BUILD_NUMBER =~ ^[0-9]+$ ]] || die "build number is not an integer: $BUILD_NUMBER"
if [[ $UNSIGNED != 1 ]]; then
  python3 - "$VERSION" "$CHANNEL" <<'PY'
import sys
version, channel = sys.argv[1:]
core = tuple(map(int, version.split('-')[0].split('.')))
if core < (0, 7, 0) or (channel == 'stable' and '-' in version):
    sys.exit('error: native releases must be at least 0.7.0; stable releases cannot be prereleases')
PY
fi

if [[ $UNSIGNED == 1 && -f $PROFILE ]]; then
  die "UNSIGNED=1 cannot embed $PROFILE (restricted entitlements need a Developer ID signature). Move the profile aside, or build signed."
fi
if [[ $UNSIGNED == 1 && -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]]; then
  note "TAURI_SIGNING_PRIVATE_KEY is set. UNSIGNED=1 ignores it and does not sign the updater archive."
fi
if [[ $UNSIGNED != 1 ]]; then
  if [[ -z ${APPLE_SIGNING_IDENTITY:-} ]]; then
    die "APPLE_SIGNING_IDENTITY is not set. Use UNSIGNED=1 for an ad-hoc local package. This script does not pick a login-keychain identity."
  fi
  [[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]] || die "TAURI_SIGNING_PRIVATE_KEY is not set"
  if [[ $SKIP_NOTARIZE != 1 ]]; then
    if [[ -z ${APPLE_API_KEY:-} || -z ${APPLE_API_ISSUER:-} || -z ${APPLE_API_KEY_PATH:-} ]]; then
      if [[ -z ${APPLE_ID:-} || -z ${APPLE_APP_SPECIFIC_PASSWORD:-} ]]; then
        die "notarization needs APPLE_API_KEY, APPLE_API_ISSUER, and APPLE_API_KEY_PATH (or APPLE_ID and APPLE_APP_SPECIFIC_PASSWORD). SKIP_NOTARIZE=1 leaves the app signed only."
      fi
    fi
  fi
fi

note "Design assets"
python3 "$ROOT/scripts/design/check-design-assets.py"
[[ -f $BG ]] || die "DMG background missing: $BG"
bg_meta=$(sips -g pixelWidth -g pixelHeight -g dpiWidth -g dpiHeight "$BG")
echo "$bg_meta" | grep -q "pixelWidth: 1320" || die "DMG background pixelWidth is not 1320"
echo "$bg_meta" | grep -q "pixelHeight: 880" || die "DMG background pixelHeight is not 880"
echo "$bg_meta" | grep -Eq "dpiWidth: 144(\.0*)?$" || die "DMG background dpiWidth is not 144"
echo "$bg_meta" | grep -Eq "dpiHeight: 144(\.0*)?$" || die "DMG background dpiHeight is not 144"

KEYCHAIN=""
P12=""
DMG_DEV=""
KEYCHAIN_PASS=""
cleanup() {
  if [[ -n $DMG_DEV ]]; then
    hdiutil detach "$DMG_DEV" -force >/dev/null 2>&1 || true
    DMG_DEV=""
  fi
  if [[ -n $KEYCHAIN ]]; then
    security delete-keychain "$KEYCHAIN" >/dev/null 2>&1 || true
    KEYCHAIN=""
  fi
  if [[ -n $P12 && -f $P12 ]]; then
    rm -f "$P12"
    P12=""
  fi
}
trap cleanup EXIT

import_certificate() {
  [[ -n ${APPLE_CERTIFICATE:-} ]] || return 0
  [[ -n ${APPLE_CERTIFICATE_PASSWORD:-} ]] || die "APPLE_CERTIFICATE is set without APPLE_CERTIFICATE_PASSWORD"
  KEYCHAIN="/private/tmp/kudzu-release-$$.keychain-db"
  P12=$(mktemp /private/tmp/kudzu-cert.XXXXXX.p12)
  KEYCHAIN_PASS=$(openssl rand -hex 16)
  printf '%s' "$APPLE_CERTIFICATE" | base64 --decode >"$P12"
  security create-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN"
  security set-keychain-settings -lut 21600 "$KEYCHAIN"
  security unlock-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN"
  security import "$P12" -k "$KEYCHAIN" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security >/dev/null
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASS" "$KEYCHAIN" >/dev/null
  security list-keychains -d user -s "$KEYCHAIN" $(security list-keychains -d user | sed -e 's/^ *"//' -e 's/"$//')
  rm -f "$P12"
  P12=""
}

validate_profile() {
  [[ -f $PROFILE ]] || {
    if [[ $REQUIRE_ICLOUD == 1 ]]; then
      die "REQUIRE_ICLOUD=1 but $PROFILE is missing"
    fi
    note "no provisioning profile; this build cannot use iCloud sync"
    return 1
  }
  local plist
  plist=$(security cms -D -i "$PROFILE")
  python3 "$ROOT/scripts/release/validate_profile.py" <<<"$plist" || die "iCloud profile validation failed"
  PROFILE_PLIST=$plist
  return 0
}

sign_inside_out() {
  local identity=$1 entitlements=$2
  local args=(--force --sign "$identity" --options runtime --timestamp)
  local item
  while IFS= read -r item; do
    [[ -n $item ]] || continue
    codesign "${args[@]}" "$item"
  done < <(find "$APP" -depth \( -name '*.framework' -o -name '*.dylib' -o -name '*.appex' -o -name '*.xpc' -o -name '*.bundle' \) ! -path "$APP")
  if [[ -n $entitlements ]]; then
    args+=(--entitlements "$entitlements")
  fi
  codesign "${args[@]}" "$APP"
}

notarize() {
  local target=$1
  if [[ -n ${APPLE_API_KEY_PATH:-} && -n ${APPLE_API_KEY:-} && -n ${APPLE_API_ISSUER:-} ]]; then
    xcrun notarytool submit "$target" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" --wait --timeout 30m
  else
    xcrun notarytool submit "$target" --apple-id "$APPLE_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --team-id "${APPLE_TEAM_ID:-V3HN8HXZYK}" --wait --timeout 30m
  fi
}

sign_updater() {
  bash "$ROOT/scripts/release/sign-update.sh" "$1"
}

probe_mcp() {
  local exe=$1
  python3 - "$exe" <<'PY'
import os, subprocess, sys
exe = sys.argv[1]
env = os.environ.copy()
env.pop("TAURI_SIGNING_PRIVATE_KEY", None)
env.pop("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", None)
env.pop("APPLE_CERTIFICATE", None)
env.pop("APPLE_CERTIFICATE_PASSWORD", None)
env.pop("APPLE_APP_SPECIFIC_PASSWORD", None)
message = b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"kudzu-release-probe","version":"0"}}}\n'
proc = subprocess.Popen([exe, "--mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
try:
    out, err = proc.communicate(message, timeout=15)
except subprocess.TimeoutExpired:
    proc.kill()
    proc.communicate()
    sys.stderr.write(f"error: {exe} --mcp did not answer within 15s\n")
    sys.exit(1)
if b"protocolVersion" not in out:
    sys.stderr.write(f"error: {exe} --mcp did not return protocolVersion\n")
    sys.stderr.write(out.decode("utf-8", "replace")[:500] + "\n")
    sys.stderr.write(err.decode("utf-8", "replace")[:500] + "\n")
    sys.exit(1)
print(f"mcp ok {exe}")
PY
}

mkdir -p "$DERIVED_DATA" "$DIST"
LOG=$DERIVED_DATA/xcodebuild.log
note "xcodebuild Release $VERSION ($BUILD_NUMBER) universal → $DERIVED_DATA"
if ! xcodebuild \
  -project "$ROOT/Kudzu.xcodeproj" \
  -scheme Kudzu \
  -configuration Release \
  -destination "generic/platform=macOS" \
  -derivedDataPath "$DERIVED_DATA" \
  build \
  ARCHS="arm64 x86_64" \
  ONLY_ACTIVE_ARCH=NO \
  MARKETING_VERSION="$VERSION" \
  CURRENT_PROJECT_VERSION="$BUILD_NUMBER" \
  CODE_SIGN_IDENTITY="-" \
  CODE_SIGN_STYLE=Manual \
  CODE_SIGNING_ALLOWED=YES \
  >"$LOG" 2>&1; then
  tail -n 80 "$LOG" >&2
  die "xcodebuild failed (full log $LOG)"
fi

APP=$DERIVED_DATA/Build/Products/Release/Kudzu.app
[[ -d $APP ]] || die "expected $APP"
[[ $(basename "$APP") == "Kudzu.app" ]] || die "product is not Kudzu.app"
EXE_NAME=$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")
[[ $EXE_NAME == "Kudzu" ]] || die "CFBundleExecutable is $EXE_NAME (Installer rejects kudzu-desktop)"
BID=$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$APP/Contents/Info.plist")
[[ $BID == "sh.saeed.kudzu" ]] || die "bundle id is $BID"
SHORT=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")
[[ $SHORT == "$VERSION" ]] || die "short version is $SHORT, expected $VERSION"
ARCHS_OUT=$(lipo -archs "$APP/Contents/MacOS/Kudzu")
[[ $ARCHS_OUT == *arm64* && $ARCHS_OUT == *x86_64* ]] || die "not universal: $ARCHS_OUT"
SKILL_SRC=$ROOT/Packages/KudzuKit/Sources/KudzuCore/Resources/SKILL.md
SKILL_DST=$APP/Contents/Resources/skills/kudzu/SKILL.md
[[ -f $SKILL_DST ]] || die "bundled skill missing at Contents/Resources/skills/kudzu/SKILL.md"
cmp -s "$SKILL_SRC" "$SKILL_DST" || die "bundled skill bytes differ from $SKILL_SRC"
LINK=$APP/Contents/MacOS/kudzu-desktop
[[ -L $LINK ]] || die "MCP compatibility symlink missing: Contents/MacOS/kudzu-desktop"
[[ $(readlink "$LINK") == "Kudzu" ]] || die "kudzu-desktop symlink points at $(readlink "$LINK"), expected Kudzu"
note "bundle Kudzu.app id $BID version $SHORT arches $ARCHS_OUT executable $EXE_NAME"

USE_ICLOUD=0
ENTITLEMENTS=""
if [[ $UNSIGNED != 1 ]]; then
  import_certificate
  if validate_profile; then
    USE_ICLOUD=1
    ENTITLEMENTS=$ROOT/Kudzu/Kudzu.icloud.entitlements
    cp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
  fi
  note "Developer ID signature ($APPLE_SIGNING_IDENTITY)"
  sign_inside_out "$APPLE_SIGNING_IDENTITY" "$ENTITLEMENTS"
  codesign --verify --deep --strict --verbose=2 "$APP"
  codesign --verify --strict -R 'anchor apple generic and identifier "sh.saeed.kudzu" and certificate leaf[subject.OU] = "V3HN8HXZYK" and certificate leaf[field.1.2.840.113635.100.6.1.13] exists' "$APP" \
    || die "release must be signed by Kudzu's Developer ID Application identity (team V3HN8HXZYK)"
  if [[ $USE_ICLOUD == 1 ]]; then
    codesign -d --entitlements - --xml "$APP" 2>/dev/null | grep -q "iCloud.sh.saeed.kudzu.library" || die "signed app lacks the iCloud entitlement"
    [[ -f $APP/Contents/embedded.provisionprofile ]] || die "provisioning profile was not embedded"
    CERTS=$(mktemp -d /private/tmp/kudzu-certs.XXXXXX)
    codesign -d --extract-certificates="$CERTS/c" "$APP" 2>/dev/null
    SIGNED_BY=$(openssl x509 -inform der -in "$CERTS/c0" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)
    rm -rf "$CERTS"
    ALLOWED=$(python3 -I -c 'import plistlib,hashlib,sys; p=plistlib.loads(sys.stdin.buffer.read()); print(" ".join(hashlib.sha1(c).hexdigest().upper() for c in p["DeveloperCertificates"]))' <<<"$PROFILE_PLIST")
    [[ " $ALLOWED " == *" $SIGNED_BY "* ]] || die "signed with $SIGNED_BY but the profile allows: $ALLOWED"
    note "signing certificate matches the provisioning profile"
  fi
else
  codesign --force --sign - "$APP"
  codesign --verify --strict "$APP" || die "ad-hoc signature does not verify"
  note "UNSIGNED=1: ad-hoc signature only. This package is not notarized and is not a valid update for a Developer ID installation (the installer checks the designated requirement)."
fi

note "MCP initialize probe (--mcp only; no tool calls or app-data writes)"
probe_mcp "$APP/Contents/MacOS/Kudzu"
probe_mcp "$APP/Contents/MacOS/kudzu-desktop"

if [[ $SKIP_NOTARIZE != 1 && $UNSIGNED != 1 ]]; then
  ZIP=$(mktemp /private/tmp/kudzu-notarize.XXXXXX.zip)
  ditto -c -k --keepParent "$APP" "$ZIP"
  note "notarizing app"
  notarize "$ZIP"
  rm -f "$ZIP"
  xcrun stapler staple "$APP"
  xcrun stapler validate "$APP"
fi

ARCHIVE=$DIST/Kudzu_${VERSION}_universal.app.tar.gz
rm -f "$ARCHIVE" "$ARCHIVE.sig"
tar -czf "$ARCHIVE" -C "$(dirname "$APP")" Kudzu.app
tar -tzf "$ARCHIVE" | grep -q '^Kudzu.app/Contents/' || die "updater archive is not rooted at Kudzu.app/Contents"
FIRST=$(tar -tzf "$ARCHIVE" | head -1)
[[ $FIRST == Kudzu.app || $FIRST == Kudzu.app/* ]] || die "updater archive starts at $FIRST"
note "updater archive $(basename "$ARCHIVE") (one Kudzu.app wrapper; Tauri skips the first path component, the native installer takes the .app)"

if [[ $UNSIGNED != 1 ]]; then
  sign_updater "$ARCHIVE"
  node "$ROOT/scripts/release/latest-json.mjs" \
    --version "$VERSION" \
    --url "https://github.com/$REPO/releases/download/v$VERSION/$(basename "$ARCHIVE")" \
    --signature "$ARCHIVE.sig" \
    --notes "${RELEASE_NOTES_FILE:-}" \
    --out "$DIST/latest.json"
fi

# DMG. A volume already named Kudzu would make Finder edit the wrong disk.
if diskutil info Kudzu >/dev/null 2>&1 || [[ -d /Volumes/Kudzu ]]; then
  die "a volume named Kudzu is already mounted. Detach it before packaging."
fi
SETFILE=$(command -v SetFile || true)
[[ -n $SETFILE ]] || SETFILE=$(xcrun --find SetFile)
[[ -x $SETFILE ]] || die "SetFile is not available"

STAGE=$DERIVED_DATA/dmg-stage
MOUNT=/Volumes/Kudzu
RW=$DERIVED_DATA/kudzu-rw.dmg
rm -rf "$STAGE" "$RW"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Kudzu.app"
ln -s /Applications "$STAGE/Applications"
SIZE_MB=$(( $(du -sm "$STAGE" | awk '{print $1}') + 40 ))
note "DMG layout (window 660×440 at 10,60; icons 128; text 16; app 170,220; Applications 490,220)"
hdiutil create -size "${SIZE_MB}m" -fs HFS+ -volname Kudzu "$RW" >/dev/null
ATTACH=$(hdiutil attach -readwrite -noverify -noautoopen -mountpoint "$MOUNT" "$RW")
DMG_DEV=$(printf '%s\n' "$ATTACH" | awk '/^\/dev\//{print $1; exit}')
[[ -n $DMG_DEV ]] || die "hdiutil attach did not return a device"

rm -rf "$MOUNT/Kudzu.app" "$MOUNT/Applications"
ditto "$STAGE/Kudzu.app" "$MOUNT/Kudzu.app"
ln -s /Applications "$MOUNT/Applications"
mkdir -p "$MOUNT/.background"
cp "$BG" "$MOUNT/.background/background.png"
cmp -s "$BG" "$MOUNT/.background/background.png" || die "DMG background is not byte-identical to design/dmg/background.png"
[[ -f $APP/Contents/Resources/Kudzu.icns ]] || die "Kudzu.icns missing from the built app"
cp "$APP/Contents/Resources/Kudzu.icns" "$MOUNT/.VolumeIcon.icns"
"$SETFILE" -c icnC "$MOUNT/.VolumeIcon.icns" || die "SetFile -c icnC failed"

LAYOUT=$DERIVED_DATA/dmg-layout.applescript
cat >"$LAYOUT" <<'APPLESCRIPT'
on run argv
	set volumeName to item 1 of argv
	set mountPath to item 2 of argv
	set backgroundFile to POSIX file (mountPath & "/.background/background.png") as alias
	tell application "Finder"
		tell disk volumeName
			open
			set theXOrigin to 10
			set theYOrigin to 60
			set theWidth to 660
			set theHeight to 440
			set theBottomRightX to (theXOrigin + theWidth)
			set theBottomRightY to (theYOrigin + theHeight)
			tell container window
				set current view to icon view
				set toolbar visible to false
				set statusbar visible to false
				set the bounds to {theXOrigin, theYOrigin, theBottomRightX, theBottomRightY}
				set statusbar visible to false
				set position of every item to {theBottomRightX + 100, 100}
			end tell
			set opts to the icon view options of container window
			tell opts
				set icon size to 128
				set text size to 16
				set arrangement to not arranged
				set background picture to backgroundFile
			end tell
			set position of item "Kudzu.app" to {170, 220}
			set the extension hidden of item "Kudzu.app" to true
			set position of item "Applications" to {490, 220}
			close
			open
			delay 1
			tell container window
				set statusbar visible to false
				set the bounds to {theXOrigin, theYOrigin, theBottomRightX - 10, theBottomRightY - 10}
			end tell
		end tell
		delay 1
		tell disk volumeName
			tell container window
				set statusbar visible to false
				set the bounds to {theXOrigin, theYOrigin, theBottomRightX, theBottomRightY}
			end tell
		end tell
		delay 3
		set waitTime to 0
		repeat
			delay 1
			set waitTime to waitTime + 1
			if waitTime > 30 then error "Finder did not write .DS_Store within 30 seconds at " & mountPath
			try
				do shell script "[ -f " & quoted form of (mountPath & "/.DS_Store") & " ]"
				exit repeat
			end try
		end repeat
		log "waited " & waitTime & " seconds for .DS_Store"
	end tell
end run
APPLESCRIPT

VERIFY=$DERIVED_DATA/dmg-verify.applescript
cat >"$VERIFY" <<'APPLESCRIPT'
on run argv
	set volumeName to item 1 of argv
	tell application "Finder"
		tell disk volumeName
			set b to bounds of container window
			set opts to icon view options of container window
			set appPos to position of item "Kudzu.app"
			set appsPos to position of item "Applications"
			set extHidden to extension hidden of item "Kudzu.app"
			set tb to toolbar visible of container window
			set sb to statusbar visible of container window
			return (item 1 of b as integer as text) & "," & (item 2 of b as integer as text) & "," & (item 3 of b as integer as text) & "," & (item 4 of b as integer as text) & "|" & (icon size of opts as integer as text) & "|" & (text size of opts as integer as text) & "|" & (item 1 of appPos as integer as text) & "," & (item 2 of appPos as integer as text) & "|" & (item 1 of appsPos as integer as text) & "," & (item 2 of appsPos as integer as text) & "|" & (extHidden as text) & "|" & (tb as text) & "|" & (sb as text)
		end tell
	end tell
end run
APPLESCRIPT

note "Finder layout (sleep 2s, then osascript; this fails the build if Finder cannot place the icons)"
sleep 2
if ! python3 - "$LAYOUT" "$MOUNT" <<'PY'
import subprocess, sys
script, mount = sys.argv[1], sys.argv[2]
try:
    result = subprocess.run(["/usr/bin/osascript", script, "Kudzu", mount], timeout=180, capture_output=True, text=True)
except subprocess.TimeoutExpired:
    sys.stderr.write("error: osascript timed out after 180s (Finder did not finish the DMG layout)\n")
    sys.exit(1)
sys.stdout.write(result.stdout)
sys.stderr.write(result.stderr)
sys.exit(result.returncode)
PY
then
  die "Finder AppleScript failed. The app and the updater archive are already in $DIST. Layout is not skipped."
fi
sleep 4
LAYOUT_OUT=$(osascript "$VERIFY" "Kudzu") || die "could not read the Finder layout back"
[[ $LAYOUT_OUT == 10,60,670,500\|128\|16\|170,220\|490,220\|true\|false\|false ]] || die "DMG layout mismatch: $LAYOUT_OUT (expected 10,60,670,500|128|16|170,220|490,220|true|false|false)"
cmp -s "$BG" "$MOUNT/.background/background.png" || die "DMG background changed during layout"
[[ -f $MOUNT/.DS_Store ]] || die "Finder did not write .DS_Store"
"$SETFILE" -a C "$MOUNT" || die "SetFile -a C failed"
if ! bless --folder "$MOUNT"; then
  die "bless --folder failed. SetFile and the icon positions succeeded; the volume icon may not show until bless works."
fi
chmod -Rf go-w "$MOUNT" >/dev/null 2>&1 || true
hdiutil detach "$DMG_DEV" >/dev/null
DMG_DEV=""

DMG=$DIST/Kudzu_${VERSION}_universal.dmg
rm -f "$DMG"
hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$DMG" >/dev/null
[[ -f $DMG ]] || die "UDZO convert did not write $DMG"

if [[ $UNSIGNED != 1 ]]; then
  codesign --force --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$DMG"
  codesign --verify --strict "$DMG"
  if [[ $SKIP_NOTARIZE != 1 ]]; then
    note "notarizing DMG"
    notarize "$DMG"
    xcrun stapler staple "$DMG"
    xcrun stapler validate "$DMG"
    spctl --assess --type execute --verbose=2 "$APP"
    spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
  else
    note "SKIP_NOTARIZE=1: signed, not notarized, not stapled"
  fi
fi

(cd "$DIST" && shasum -a 256 Kudzu_* >SHA256SUMS.txt)
if [[ $VERSION == "$FILE_VERSION" && $FILE_VERSION == 0.6.0 ]]; then
  note "local development package $VERSION; signed releases require at least 0.7.0. Put Release-As: 0.7.0 in the migration commit body before release-please runs."
fi
note "done ($DIST)"
ls -lh "$DIST"
