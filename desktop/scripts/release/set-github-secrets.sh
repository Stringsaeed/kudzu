#!/usr/bin/env bash
# Uploads the release secrets to GitHub Actions (Stringsaeed/kudzu) with `gh secret set`.
# Nothing is written to disk or echoed. Run once, and again whenever a credential rotates.
#
#   scripts/release/set-github-secrets.sh \
#     --p12 ~/Desktop/DeveloperID.p12 \
#     --p8 ~/Downloads/AuthKey_ABC123XYZ.p8 --key-id ABC123XYZ --issuer 00000000-0000-0000-0000-000000000000 \
#     --profile ~/Downloads/Kudzu_Developer_ID.provisionprofile      # optional: enables iCloud sync
#
# Prompts for the .p12 password and the Homebrew tap token. The updater key is read from
# ~/.tauri/kudzu-updater.key and its password from the login keychain.
set -euo pipefail
REPO=${REPO:-Stringsaeed/kudzu}
P12= P8= KEY_ID= ISSUER= PROFILE=
while [[ $# -gt 0 ]]; do
  case $1 in
    --p12) P12=$2; shift 2 ;;
    --p8) P8=$2; shift 2 ;;
    --key-id) KEY_ID=$2; shift 2 ;;
    --issuer) ISSUER=$2; shift 2 ;;
    --profile) PROFILE=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done
[[ -f $P12 && -f $P8 && -n $KEY_ID && -n $ISSUER ]] || { sed -n '2,12p' "$0"; exit 1; }

set_secret() { printf '%s' "$2" | gh secret set "$1" --repo "$REPO" >/dev/null && echo "  ✓ $1"; }

read -rsp "Password for $(basename "$P12"): " P12_PASS; echo
IDENTITY=$(openssl pkcs12 -in "$P12" -passin "pass:$P12_PASS" -nokeys -legacy 2>/dev/null | openssl x509 -noout -subject -nameopt multiline 2>/dev/null | awk -F' = ' '/commonName/{print $2; exit}')
[[ $IDENTITY == "Developer ID Application:"* ]] || { echo "error: $P12 isn't a Developer ID Application certificate (got: ${IDENTITY:-nothing})" >&2; exit 1; }

UPDATER_KEY=$(cat ~/.tauri/kudzu-updater.key)
UPDATER_PASS=$(security find-generic-password -a kudzu-updater -s "Kudzu updater signing key password" -w)

echo "Setting secrets on $REPO:"
set_secret APPLE_CERTIFICATE "$(base64 < "$P12")"
set_secret APPLE_CERTIFICATE_PASSWORD "$P12_PASS"
set_secret APPLE_SIGNING_IDENTITY "$IDENTITY"
set_secret APPLE_API_KEY "$KEY_ID"
set_secret APPLE_API_ISSUER "$ISSUER"
set_secret APPLE_API_KEY_P8 "$(cat "$P8")"
[[ -n $PROFILE ]] && set_secret APPLE_PROVISIONING_PROFILE "$(base64 < "$PROFILE")"
set_secret TAURI_SIGNING_PRIVATE_KEY "$UPDATER_KEY"
set_secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD "$UPDATER_PASS"

read -rsp "Homebrew tap token (fine-grained PAT, Contents: write on Stringsaeed/homebrew-tap; Enter to skip): " TAP; echo
[[ -n $TAP ]] && set_secret HOMEBREW_TAP_TOKEN "$TAP"
read -rsp "Release PR token (optional PAT so CI runs on release PRs; Enter to skip): " RP; echo
[[ -n $RP ]] && set_secret RELEASE_PLEASE_TOKEN "$RP"
echo "Done."
