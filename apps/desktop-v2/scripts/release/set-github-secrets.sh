#!/usr/bin/env bash
# Owner only. Values come from explicit environment variables, never auto-discovered.
# --dry-run prints names only. No secret values or local credential stores are read.
set -euo pipefail
names=(APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_P8 APPLE_PROVISIONING_PROFILE TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD)
if [[ ${1:-} == --dry-run ]]; then printf '%s\n' "${names[@]}"; exit 0; fi
[[ $# == 1 && $1 == --apply ]] || { echo 'usage: set-github-secrets.sh --dry-run|--apply (explicit env values, GITHUB_REPOSITORY required)' >&2; exit 2; }
: "${GITHUB_REPOSITORY:?required}"
for name in "${names[@]}"; do declare -p "$name" >/dev/null 2>&1 || { echo "$name missing" >&2; exit 1; }; done
for name in "${names[@]}"; do printf '%s' "${!name}" | gh secret set "$name" --repo "$GITHUB_REPOSITORY"; done
