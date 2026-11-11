#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR="$PWD/target"
export MACOSX_DEPLOYMENT_TARGET=14.0
PROFILE=assets/Kudzu.provisionprofile
case "${1:-}" in
  --dry-run) exec cargo xtask release --dry-run ;;
  --preflight)
    python3 scripts/check-design-assets.py
    if [[ -f $PROFILE ]]; then
      security cms -D -i "$PROFILE" | python3 -I -c '
import plistlib,sys
p=plistlib.load(sys.stdin.buffer); e=p["Entitlements"]
assert e["com.apple.application-identifier"] == "V3HN8HXZYK.sh.saeed.kudzu", "wrong profile app id"
assert "iCloud.sh.saeed.kudzu" in e["com.apple.developer.icloud-container-identifiers"], "wrong profile container"
'
    elif [[ ${REQUIRE_ICLOUD:-} == 1 ]]; then
      echo 'REQUIRE_ICLOUD=1 but profile missing' >&2; exit 1
    fi
    exit 0 ;;
  --verify-app|--verify)
    APP=$2
    codesign --verify --deep --strict "$APP"
    if [[ ${APPLE_SIGNING_IDENTITY:-} != - ]]; then
      codesign --verify --strict -R='anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = "V3HN8HXZYK" and identifier "sh.saeed.kudzu"' "$APP"
    fi
    for arch in arm64 x86_64; do
      lipo "$APP/Contents/MacOS/Kudzu" -verify_arch "$arch"
      xcrun vtool -arch "$arch" -show-build "$APP/Contents/MacOS/Kudzu" | awk '$1=="minos" {found=1; if ($2!="14.0") exit 1} END {if (!found) exit 1}'
    done
    if [[ -f $PROFILE ]]; then
      [[ -f "$APP/Contents/embedded.provisionprofile" ]]
      cmp "$PROFILE" "$APP/Contents/embedded.provisionprofile"
      certdir=$(mktemp -d "$PWD/target/release-cert.XXXXXX")
      trap 'rm -rf "$certdir"' EXIT
      security cms -D -i "$PROFILE" > "$certdir/profile.plist"
      codesign -d --extract-certificates="$certdir/c" "$APP" 2>/dev/null
      codesign -d --entitlements - --xml "$APP" > "$certdir/entitlements.plist" 2>/dev/null
      python3 -I - "$certdir" <<'PY'
import hashlib, pathlib, plistlib, sys
p=pathlib.Path(sys.argv[1])
profile=plistlib.loads((p/'profile.plist').read_bytes())
assert hashlib.sha1((p/'c0').read_bytes()).digest() in [hashlib.sha1(c).digest() for c in profile['DeveloperCertificates']], 'certificate not allowed by profile'
e=plistlib.loads((p/'entitlements.plist').read_bytes())
assert e['com.apple.application-identifier']=='V3HN8HXZYK.sh.saeed.kudzu'
assert e['com.apple.developer.team-identifier']=='V3HN8HXZYK'
assert e['com.apple.developer.icloud-container-identifiers']==['iCloud.sh.saeed.kudzu']
assert e['com.apple.developer.icloud-services']==['CloudKit']
assert e['com.apple.developer.icloud-container-environment']=='Production'
PY
    fi
    if [[ $1 == --verify ]]; then
      DMG=$3
      codesign --verify --strict "$DMG"
      if [[ ${SKIP_NOTARIZE:-} != 1 ]]; then
        xcrun stapler validate "$APP"
        spctl --assess --type execute --verbose=2 "$APP"
        xcrun stapler validate "$DMG"
        spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
      fi
    fi
    exit 0 ;;
  '') exec cargo xtask release ;;
  *) echo 'usage: build.sh [--dry-run]' >&2; exit 2 ;;
esac
