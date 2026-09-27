#!/bin/bash
set -euo pipefail

# Keep this identity in the user's login keychain. Reusing the same private key
# gives future builds the same designated requirement for macOS privacy grants.
identity='Anywhere Local Code Signing'
keychain="$HOME/Library/Keychains/login.keychain-db"
if security find-identity -p codesigning "$keychain" | grep -Fq "\"$identity\""; then
  printf 'Reusing %s in the login keychain.\n' "$identity"
  exit 0
fi

umask 077
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
password=$(openssl rand -hex 24)
openssl req -x509 -newkey rsa:3072 -sha256 -days 3650 -nodes \
  -keyout "$scratch/key.pem" -out "$scratch/cert.pem" \
  -subj "/CN=$identity" \
  -addext 'keyUsage=critical,digitalSignature' \
  -addext 'extendedKeyUsage=codeSigning' >/dev/null 2>&1
# Security.framework does not import OpenSSL 3's default PKCS#12 encryption.
openssl pkcs12 -export -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
  -inkey "$scratch/key.pem" -in "$scratch/cert.pem" \
  -out "$scratch/signing.p12" -passout "pass:$password" >/dev/null 2>&1
security import "$scratch/signing.p12" -f pkcs12 -k "$keychain" \
  -P "$password" -T /usr/bin/codesign >/dev/null

if ! security find-identity -p codesigning "$keychain" | grep -Fq "\"$identity\""; then
  echo 'The signing identity was not available after import.' >&2
  exit 1
fi
printf 'Created %s in the login keychain. Keep this identity for future builds.\n' "$identity"
