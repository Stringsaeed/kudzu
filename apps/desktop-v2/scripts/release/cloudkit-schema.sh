#!/usr/bin/env bash
# Adds Kudzu's record types to the iCloud.sh.saeed.kudzu Development schema, keeping every type
# already there (the Swift app's SwiftData types share the container). Run once; afterwards
# deploy to Production in the CloudKit Console (Schema → Deploy Schema Changes…).
#
# Needs a CloudKit management token once:  xcrun cktool save-token --type management
set -euo pipefail
TEAM=${TEAM:-V3HN8HXZYK}
CONTAINER=${CONTAINER:-iCloud.sh.saeed.kudzu}
OURS="$(cd "$(dirname "$0")/../.." && pwd)/cloudkit/kudzu-types.ckdb"
tmp=$(mktemp -d "$(dirname "$OURS")/schema.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

xcrun cktool export-schema --team-id "$TEAM" --container-id "$CONTAINER" --environment development --output-file "$tmp/current.ckdb"

# Append each of our RECORD TYPE blocks that the container doesn't have yet.
python3 -I - "$tmp/current.ckdb" "$OURS" "$tmp/merged.ckdb" <<'PY'
import re, sys
current, ours, out = (open(sys.argv[1]).read(), open(sys.argv[2]).read(), sys.argv[3])
have = set(re.findall(r"RECORD TYPE\s+(\w+)", current))
blocks = re.findall(r"(    RECORD TYPE\s+(\w+)\s*\(.*?\n    \);)", ours, re.S)
missing = [b for b, name in blocks if name not in have]
merged = current.rstrip() + ("\n\n" + "\n\n".join(missing) if missing else "") + "\n"
open(out, "w").write(merged)
print("adding:", ", ".join(n for b, n in blocks if n not in have) or "nothing (already present)")
PY

xcrun cktool validate-schema --team-id "$TEAM" --container-id "$CONTAINER" --environment development --file "$tmp/merged.ckdb"
xcrun cktool import-schema --team-id "$TEAM" --container-id "$CONTAINER" --environment development --file "$tmp/merged.ckdb"
echo "Done. Now deploy to Production: https://icloud.developer.apple.com → $CONTAINER → Schema → Deploy Schema Changes…"
