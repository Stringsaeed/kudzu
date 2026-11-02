#!/bin/bash
set -euo pipefail

# Build a separately identified debug app with an in-memory library and no background services.
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"
xcodegen generate
xcodebuild -project Kudzu.xcodeproj -scheme Kudzu -configuration Debug \
  -derivedDataPath build/DesignAuditDerivedData build

audit_app="$repo_root/build/UIAudit/Kudzu UI Audit.app"
mkdir -p "$(dirname "$audit_app")"
ditto "$repo_root/build/DesignAuditDerivedData/Build/Products/Debug/Kudzu.app" "$audit_app"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier sh.saeed.kudzu.ui-audit' "$audit_app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName Kudzu UI Audit' "$audit_app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleName Kudzu UI Audit' "$audit_app/Contents/Info.plist"
codesign --force --sign - "$audit_app"
printf '%s\n' "$audit_app"
