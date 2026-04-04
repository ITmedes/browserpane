#!/bin/bash
# Generate a self-signed TLS certificate for local WebTransport development.
#
# Chrome requires valid TLS for WebTransport. For local dev, either:
# 1. Add the generated cert to Chrome's trust store, or
# 2. Launch Chrome with: --ignore-certificate-errors-spki-list=<fingerprint>
#
# Usage: ./dev/gen-cert.sh [output_dir]

set -euo pipefail

OUTPUT_DIR="${1:-.}"
CERT_FILE="$OUTPUT_DIR/cert.pem"
KEY_FILE="$OUTPUT_DIR/cert.key"

echo "Generating self-signed certificate..."

openssl req -x509 -nodes -days 30 \
    -newkey ec:<(openssl ecparam -name prime256v1) \
    -keyout "$KEY_FILE" \
    -out "$CERT_FILE" \
    -subj "/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

# Print the SPKI fingerprint for Chrome's --ignore-certificate-errors-spki-list flag
FINGERPRINT=$(openssl x509 -in "$CERT_FILE" -pubkey -noout | \
    openssl pkey -pubin -outform der | \
    openssl dgst -sha256 -binary | \
    openssl enc -base64)

echo ""
echo "Certificate generated:"
echo "  Cert: $CERT_FILE"
echo "  Key:  $KEY_FILE"
echo ""
echo "To use with Chrome for local development, launch with:"
echo "  chrome --ignore-certificate-errors-spki-list=$FINGERPRINT"
echo ""
echo "Or add to Chrome's trust store for your OS."
