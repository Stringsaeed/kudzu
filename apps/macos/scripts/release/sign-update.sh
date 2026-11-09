#!/usr/bin/env bash
# Same key and outer base64 signature envelope as the shipping Tauri updater.
set -euo pipefail
archive=${1:?usage: sign-update.sh archive.app.tar.gz}
[[ -f $archive ]] || { echo 'error: updater archive is missing' >&2; exit 1; }
: "${TAURI_SIGNING_PRIVATE_KEY:?set the existing updater key or its file path}"
command -v minisign >/dev/null || { echo 'error: install minisign on the release runner' >&2; exit 1; }
work=$(mktemp -d /private/tmp/kudzu-sign-update.XXXXXX)
trap 'rm -rf "$work"' EXIT

# Tauri's key is base64 of a minisign key file. Also accept a native minisign file;
# decode into a private temporary copy, never rewriting the registered key.
python3 - "$work/key" <<'PY'
import base64, os, pathlib, sys
value = os.environ['TAURI_SIGNING_PRIVATE_KEY']
try:
    path = pathlib.Path(value)
    raw = path.read_bytes() if path.is_file() else value.encode()
except OSError:
    raw = value.encode()
if not raw.startswith(b'untrusted comment:'):
    try:
        raw = base64.b64decode(b''.join(raw.split()), validate=True)
    except ValueError:
        sys.exit('error: updater key is neither a minisign file nor its Tauri base64 encoding')
if not raw.startswith(b'untrusted comment:'):
    sys.exit('error: decoded updater key is not a minisign key file')
fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(fd, 'wb') as out:
    out.write(raw)
PY
comment=$(printf 'timestamp:%s\tfile:%s' "$(date +%s)" "$(basename "$archive")")
printf '%s\n' "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" | minisign -S -s "$work/key" -m "$archive" -x "$work/signature" -t "$comment"
base64 <"$work/signature" | tr -d '\n' >"$work/envelope"
printf '\n' >>"$work/envelope"
mv "$work/envelope" "$archive.sig"
