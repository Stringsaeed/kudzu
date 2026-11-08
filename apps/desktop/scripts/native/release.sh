#!/usr/bin/env bash
# Native counterpart to scripts/release/build.sh: identical dist/ assets and credentials.
# APPLE_CERTIFICATE (+ _PASSWORD) optionally imports a CI .p12 into a temporary keychain.
# Local packaging only: APPLE_SIGNING_IDENTITY=- SKIP_NOTARIZE=1 (use a throwaway updater key).
# Ad-hoc packages are never suitable for publication and cannot carry iCloud entitlements.
set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT=$PWD
python3 scripts/check-design-assets.py
VERSION=$(node -p "require('./package.json').version")
TAG="v$VERSION"
REPO=${GITHUB_REPOSITORY:-Stringsaeed/kudzu}
DIST=$ROOT/dist
APP=$ROOT/src-tauri/target/native/Kudzu.app
PROFILE=$ROOT/src-tauri/Kudzu.provisionprofile

need() { [[ -n "${!1:-}" ]] || { echo "error: $1 is not set" >&2; exit 1; }; }
need APPLE_SIGNING_IDENTITY
need TAURI_SIGNING_PRIVATE_KEY
if [[ ${SKIP_NOTARIZE:-} != 1 ]]; then
  need APPLE_API_KEY; need APPLE_API_ISSUER; need APPLE_API_KEY_PATH
  [[ $APPLE_SIGNING_IDENTITY != - ]] || { echo "error: notarization requires Developer ID signing" >&2; exit 1; }
elif [[ $APPLE_SIGNING_IDENTITY == - ]]; then
  echo "warning: ad-hoc packaging check only; do not publish dist/" >&2
fi

TEMP=$(mktemp -d)
KEYCHAIN=
cleanup() {
  local status=$?
  if [[ -n $KEYCHAIN ]]; then security delete-keychain "$KEYCHAIN" || true; fi
  rm -rf "$TEMP"
  exit "$status"
}
trap cleanup EXIT

if [[ -n ${APPLE_CERTIFICATE:-} ]]; then
  need APPLE_CERTIFICATE_PASSWORD
  KEYCHAIN=$TEMP/signing.keychain-db
  KEYCHAIN_PASSWORD=$(openssl rand -hex 32)
  printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$TEMP/certificate.p12"
  security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
  security set-keychain-settings -lut 21600 "$KEYCHAIN"
  security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
  security import "$TEMP/certificate.p12" -k "$KEYCHAIN" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null
  rm "$TEMP/certificate.p12"
  export SIGN_KEYCHAIN=$KEYCHAIN
fi

if [[ -f $PROFILE ]]; then
  PLIST=$(security cms -D -i "$PROFILE")
  APP_ID=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:com.apple.application-identifier" /dev/stdin <<<"$PLIST")
  CONTAINERS=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:com.apple.developer.icloud-container-identifiers" /dev/stdin <<<"$PLIST" 2>/dev/null || true)
  [[ $APP_ID == V3HN8HXZYK.sh.saeed.kudzu ]] || { echo "error: profile is for $APP_ID, not V3HN8HXZYK.sh.saeed.kudzu" >&2; exit 1; }
  [[ $CONTAINERS == *iCloud.sh.saeed.kudzu* ]] || { echo "error: profile doesn't include the iCloud.sh.saeed.kudzu container" >&2; exit 1; }
  echo "==> iCloud: on (profile $(/usr/libexec/PlistBuddy -c 'Print :Name' /dev/stdin <<<"$PLIST"))"
elif [[ ${REQUIRE_ICLOUD:-} == 1 ]]; then
  echo "error: REQUIRE_ICLOUD=1 but $PROFILE is missing" >&2; exit 1
else
  echo "warning: no provisioning profile; this build can't use iCloud sync" >&2
fi

echo "==> Native Kudzu $VERSION (universal-apple-darwin)"
SIGN_IDENTITY=$APPLE_SIGNING_IDENTITY scripts/native/bundle.sh --release --universal
codesign --verify --deep --strict --verbose=1 "$APP"
ARCHS=$(lipo -archs "$APP/Contents/MacOS/kudzu-desktop")
[[ " $ARCHS " == *" x86_64 "* && " $ARCHS " == *" arm64 "* ]] || { echo "error: not a universal binary ($ARCHS)" >&2; exit 1; }
if [[ -f $PROFILE ]]; then
  codesign -d --entitlements - --xml "$APP" 2>/dev/null | grep -q "iCloud.sh.saeed.kudzu" || { echo "error: signed app lacks the iCloud entitlement" >&2; exit 1; }
  [[ -f $APP/Contents/embedded.provisionprofile ]] || { echo "error: provisioning profile wasn't embedded" >&2; exit 1; }
  cmp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
  codesign -d --extract-certificates="$TEMP/c" "$APP" 2>/dev/null
  SIGNED_BY=$(openssl x509 -inform der -in "$TEMP/c0" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)
  ALLOWED=$(python3 -I -c 'import plistlib,hashlib,sys; p=plistlib.loads(sys.stdin.buffer.read()); print(" ".join(hashlib.sha1(c).hexdigest().upper() for c in p["DeveloperCertificates"]))' <<<"$PLIST")
  if [[ " $ALLOWED " != *" $SIGNED_BY "* ]]; then
    echo "error: the app is signed with certificate $SIGNED_BY, but the provisioning profile only allows: $ALLOWED" >&2
    echo "       macOS would refuse to open it. Export the matching Developer ID certificate as the .p12 (or regenerate the profile with this one)." >&2
    exit 1
  fi
  echo "==> Signing certificate $SIGNED_BY matches the provisioning profile"
fi

notarize() {
  xcrun notarytool submit "$1" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" --wait --timeout 30m
  xcrun stapler staple "$2"
  xcrun stapler validate "$2"
}
if [[ ${SKIP_NOTARIZE:-} != 1 ]]; then
  echo "==> Notarizing app"
  ditto -c -k --keepParent "$APP" "$TEMP/Kudzu.zip"
  notarize "$TEMP/Kudzu.zip" "$APP"
  spctl --assess --type execute --verbose=2 "$APP"
fi

rm -rf "$DIST"
mkdir -p "$DIST"
DMG=$DIST/Kudzu_${VERSION}_universal.dmg
ARCHIVE=$DIST/Kudzu_${VERSION}_universal.app.tar.gz
scripts/native/dmg.sh "$APP" "$DMG"
SIGN_ARGS=(--force --sign "$APPLE_SIGNING_IDENTITY")
if [[ -n $KEYCHAIN ]]; then SIGN_ARGS+=(--keychain "$KEYCHAIN"); fi
if [[ $APPLE_SIGNING_IDENTITY == - ]]; then SIGN_ARGS+=(--timestamp=none); else SIGN_ARGS+=(--timestamp); fi
codesign "${SIGN_ARGS[@]}" "$DMG"
codesign --verify --strict "$DMG"
if [[ ${SKIP_NOTARIZE:-} != 1 ]]; then
  echo "==> Notarizing DMG"
  notarize "$DMG" "$DMG"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
fi
codesign --verify --deep --strict --verbose=1 "$APP"

# Archive after stapling, and keep Kudzu.app at the root (the native updater requires it).
COPYFILE_DISABLE=1 tar -czf "$ARCHIVE" -C "$(dirname "$APP")" Kudzu.app
# build.sh accepts either key contents or a path; the standalone signer uses separate flags.
if [[ -f $TAURI_SIGNING_PRIVATE_KEY ]]; then
  env -u TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PATH="$TAURI_SIGNING_PRIVATE_KEY" pnpm exec tauri signer sign --app-version "$VERSION" "$ARCHIVE"
else
  pnpm exec tauri signer sign --app-version "$VERSION" "$ARCHIVE"
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
