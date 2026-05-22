#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

CERT_PATH="${1:-$REPO_ROOT/dev/egress-ca.pem}"
KEY_PATH="${2:-$REPO_ROOT/dev/egress-ca.key}"
OUT_DIR="${3:-$REPO_ROOT/dev/egress-mitmproxy}"

if [ ! -r "$CERT_PATH" ]; then
  echo "Egress CA certificate is not readable: $CERT_PATH" >&2
  echo "Generate it first, for example: openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj '/CN=BrowserPane Local Egress Test CA' -keyout dev/egress-ca.key -out dev/egress-ca.pem" >&2
  exit 64
fi

if [ ! -r "$KEY_PATH" ]; then
  echo "Egress CA private key is not readable: $KEY_PATH" >&2
  echo "mitmproxy needs the private key to mint per-site certificates for the local TLS observer." >&2
  exit 64
fi

cert_pub="$(openssl x509 -in "$CERT_PATH" -noout -pubkey | openssl sha256)"
key_pub="$(openssl pkey -in "$KEY_PATH" -pubout | openssl sha256)"
if [ "$cert_pub" != "$key_pub" ]; then
  echo "Egress CA certificate and private key do not match." >&2
  exit 65
fi

mkdir -p "$OUT_DIR"
cat "$KEY_PATH" "$CERT_PATH" > "$OUT_DIR/mitmproxy-ca.pem"
cp "$CERT_PATH" "$OUT_DIR/mitmproxy-ca-cert.pem"
chmod 600 "$OUT_DIR/mitmproxy-ca.pem"
chmod 644 "$OUT_DIR/mitmproxy-ca-cert.pem"

echo "Prepared mitmproxy CA material in $OUT_DIR"
echo "BrowserPane runtime trust should continue to use $CERT_PATH"
