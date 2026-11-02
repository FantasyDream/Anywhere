#!/bin/bash
# Use a dedicated, persistent CI identity in an ephemeral runner keychain.
set -euo pipefail
[[ "${GITHUB_ACTIONS:-}" == true && "$(uname -s)" == Darwin ]] || {
  echo 'This helper is restricted to macOS GitHub Actions runners.' >&2
  exit 1
}
: "${RUNNER_TEMP:?Missing runner temporary directory}"
signing_directory="$RUNNER_TEMP/anywhere-signing"
keychain_path="$signing_directory/build.keychain-db"

case "${1:-}" in
  install)
    : "${ANYWHERE_CI_SIGNING_P12:?Configure the CI signing certificate secret}"
    : "${ANYWHERE_CI_SIGNING_PASSWORD:?Configure the CI signing password secret}"
    umask 077
    mkdir -p "$signing_directory"
    python3 - "$signing_directory/certificate.p12" <<'PY'
import base64
import os
from pathlib import Path
import sys

Path(sys.argv[1]).write_bytes(base64.b64decode(os.environ['ANYWHERE_CI_SIGNING_P12'], validate=True))
PY
    keychain_password=$(openssl rand -hex 32)
    echo "::add-mask::$keychain_password"
    security create-keychain -p "$keychain_password" "$keychain_path"
    security set-keychain-settings -lut 21600 "$keychain_path"
    security unlock-keychain -p "$keychain_password" "$keychain_path"
    security import "$signing_directory/certificate.p12" -f pkcs12 -k "$keychain_path" \
      -P "$ANYWHERE_CI_SIGNING_PASSWORD" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
      -k "$keychain_password" "$keychain_path" >/dev/null
    openssl pkcs12 -in "$signing_directory/certificate.p12" -clcerts -nokeys \
      -passin env:ANYWHERE_CI_SIGNING_PASSWORD -out "$signing_directory/certificate.pem"
    # The disposable hosted VM has no interactive user to approve trust changes.
    sudo -n security add-trusted-cert -d -r trustRoot -p codeSign -k "$keychain_path" \
      "$signing_directory/certificate.pem"
    security list-keychains -d user -s "$keychain_path"
    security find-identity -p codesigning "$keychain_path" | grep -Fq '"Anywhere CI Development"'
    rm -f "$signing_directory/certificate.p12"
    ;;
  cleanup)
    if [[ -e "$keychain_path" ]]; then security delete-keychain "$keychain_path"; fi
    rm -rf "$signing_directory"
    ;;
  *)
    echo 'Usage: scripts/ci-macos-signing.sh install|cleanup' >&2
    exit 2
    ;;
esac
