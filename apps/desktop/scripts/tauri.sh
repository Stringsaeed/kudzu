#!/usr/bin/env bash
set -euo pipefail

# The standard development command must not use the production app's identity or data.
if [[ "${1:-}" == dev ]]; then
  shift
  exec pnpm exec tauri dev --config src-tauri/tauri.dev.conf.json "$@"
fi

exec pnpm exec tauri "$@"
