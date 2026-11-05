#!/usr/bin/env bash
# Builds a signed, notarized, universal Kudzu release into apps/desktop/dist/.
#
# Same script in CI (.github/workflows/build-macos.yml) and on a Mac. Needs:
#   APPLE_SIGNING_IDENTITY   "Developer ID Application: … (TEAMID)", or a certificate SHA-1 locally
#   APPLE_API_KEY            App Store Connect API key ID           (notarization)
#   APPLE_API_ISSUER         App Store Connect issuer ID
#   APPLE_API_KEY_PATH       path to the AuthKey_<id>.p8 file
#   TAURI_SIGNING_PRIVATE_KEY (+ _PASSWORD)   updater key (contents, or a path)
# CI also sets APPLE_CERTIFICATE / APPLE_CERTIFICATE_PASSWORD (base64 .p12); Tauri imports it
# into a throwaway keychain. Locally the identity is read from your login keychain.
# Optional: RELEASE_NOTES_FILE (Markdown for latest.json), SKIP_NOTARIZE=1 for a quick signed build.
# iCloud sync: put the Developer ID provisioning profile at src-tauri/Kudzu.provisionprofile (CI
# writes it from the APPLE_PROVISIONING_PROFILE secret). Without it the build works but can't sync;
# REQUIRE_ICLOUD=1 turns that into an error.
set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT=$PWD
python3 scripts/check-design-assets.py
TARGET=universal-apple-darwin
VERSION=$(node -p "require('./package.json').version")
TAG="v$VERSION"
REPO=${GITHUB_REPOSITORY:-Stringsaeed/kudzu}
DIST=$ROOT/dist
BUNDLE=$ROOT/src-tauri/target/$TARGET/release/bundle

need() { [[ -n "${!1:-}" ]] || { echo "error: $1 is not set" >&2; exit 1; }; }
need APPLE_SIGNING_IDENTITY
need TAURI_SIGNING_PRIVATE_KEY
if [[ "${SKIP_NOTARIZE:-}" != 1 ]]; then
  need APPLE_API_KEY; need APPLE_API_ISSUER; need APPLE_API_KEY_PATH
else
  # Tauri notarizes whenever these are present; unset them for a signed-only build.
  unset APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH
fi

PROFILE=$ROOT/src-tauri/Kudzu.provisionprofile
TAURI_ARGS=()
if [[ -f $PROFILE ]]; then
  # The profile must grant iCloud for exactly this app, or macOS kills the app at launch.
  PLIST=$(security cms -D -i "$PROFILE")
  APP_ID=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:com.apple.application-identifier" /dev/stdin <<<"$PLIST")
  CONTAINERS=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:com.apple.developer.icloud-container-identifiers" /dev/stdin <<<"$PLIST" 2>/dev/null || true)
  [[ $APP_ID == "V3HN8HXZYK.sh.saeed.kudzu" ]] || { echo "error: profile is for $APP_ID, not V3HN8HXZYK.sh.saeed.kudzu" >&2; exit 1; }
  [[ $CONTAINERS == *iCloud.sh.saeed.kudzu* ]] || { echo "error: profile doesn't include the iCloud.sh.saeed.kudzu container" >&2; exit 1; }
  TAURI_ARGS+=(--config src-tauri/tauri.icloud.conf.json)
  echo "==> iCloud: on (profile $(/usr/libexec/PlistBuddy -c 'Print :Name' /dev/stdin <<<"$PLIST"))"
elif [[ "${REQUIRE_ICLOUD:-}" == 1 ]]; then
  echo "error: REQUIRE_ICLOUD=1 but $PROFILE is missing" >&2; exit 1
else
  echo "warning: no provisioning profile; this build can't use iCloud sync" >&2
fi

echo "==> Kudzu $VERSION ($TARGET)"
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
rm -rf "$DIST" "$BUNDLE"
mkdir -p "$DIST"

# Builds the frontend, both architectures, lipo's them, signs with the hardened runtime,
# notarizes + staples the .app, then makes the DMG and the signed updater archive.
pnpm tauri build --target "$TARGET" --bundles app,dmg ${TAURI_ARGS[@]+"${TAURI_ARGS[@]}"}

APP="$BUNDLE/macos/Kudzu.app"
DMG_SRC=$(ls "$BUNDLE"/dmg/*.dmg | head -1)
DMG="$DIST/Kudzu_${VERSION}_universal.dmg"
ARCHIVE="$DIST/Kudzu_${VERSION}_universal.app.tar.gz"
cp "$DMG_SRC" "$DMG"
cp "$BUNDLE/macos/Kudzu.app.tar.gz" "$ARCHIVE"
cp "$BUNDLE/macos/Kudzu.app.tar.gz.sig" "$ARCHIVE.sig"

# The DMG is what people download first, so it's notarized and stapled too (Gatekeeper checks
# it before the app inside is ever opened). Tauri already signed it. Don't re-sign it here: in
# CI, Tauri imports the certificate into a temporary keychain that's deleted once bundling ends.
codesign --verify --strict "$DMG" || { echo "error: the DMG isn't signed" >&2; exit 1; }
if [[ "${SKIP_NOTARIZE:-}" != 1 ]]; then
  echo "==> Notarizing DMG"
  xcrun notarytool submit "$DMG" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" --wait --timeout 30m
  xcrun stapler staple "$DMG"
fi

echo "==> Verifying"
codesign --verify --deep --strict --verbose=1 "$APP"
lipo -archs "$APP/Contents/MacOS/kudzu-desktop" | grep -q "x86_64 arm64" || { echo "error: not a universal binary" >&2; exit 1; }
if [[ -f $PROFILE ]]; then
  codesign -d --entitlements - --xml "$APP" 2>/dev/null | grep -q "iCloud.sh.saeed.kudzu" || { echo "error: signed app lacks the iCloud entitlement" >&2; exit 1; }
  [[ -f "$APP/Contents/embedded.provisionprofile" ]] || { echo "error: provisioning profile wasn't embedded" >&2; exit 1; }
  # With restricted entitlements, macOS only launches the app if it was signed by a certificate
  # the profile lists; otherwise Finder just says "can't be opened". Gatekeeper and
  # notarization don't catch this, so check it here.
  CERTS=$(mktemp -d)
  codesign -d --extract-certificates="$CERTS/c" "$APP" 2>/dev/null
  SIGNED_BY=$(openssl x509 -inform der -in "$CERTS/c0" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)
  rm -rf "$CERTS"
  ALLOWED=$(python3 -I -c 'import plistlib,hashlib,sys; p=plistlib.loads(sys.stdin.buffer.read()); print(" ".join(hashlib.sha1(c).hexdigest().upper() for c in p["DeveloperCertificates"]))' <<<"$PLIST")
  if [[ " $ALLOWED " != *" $SIGNED_BY "* ]]; then
    echo "error: the app is signed with certificate $SIGNED_BY, but the provisioning profile only allows: $ALLOWED" >&2
    echo "       macOS would refuse to open it. Export the matching Developer ID certificate as the .p12 (or regenerate the profile with this one)." >&2
    exit 1
  fi
  echo "==> Signing certificate $SIGNED_BY matches the provisioning profile"
fi
if [[ "${SKIP_NOTARIZE:-}" != 1 ]]; then
  spctl --assess --type execute --verbose=2 "$APP"
  xcrun stapler validate "$DMG"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
fi

node scripts/release/latest-json.mjs \
  --version "$VERSION" \
  --url "https://github.com/$REPO/releases/download/$TAG/$(basename "$ARCHIVE")" \
  --signature "$ARCHIVE.sig" \
  --notes "${RELEASE_NOTES_FILE:-}" \
  --out "$DIST/latest.json"

(cd "$DIST" && shasum -a 256 Kudzu_* > SHA256SUMS.txt)
echo "==> Done"
ls -lh "$DIST"
