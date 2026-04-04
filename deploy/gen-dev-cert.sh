#!/bin/bash
# Generate a self-signed cert for local BrowserPane development.
# Writes into the given directory:
#   cert.pem, cert.key           - TLS certificate and key
#   cert-fingerprint.txt         - SPKI fingerprint (for Chrome CLI flag)
#   cert-hash.txt                - SHA-256 of DER cert (for WebTransport serverCertificateHashes)
#
# The cert is valid for 10 days (WebTransport requires <=14 days for
# serverCertificateHashes to work).

set -euo pipefail

OUT="${1:-.}"
mkdir -p "$OUT"

CERT="$OUT/cert.pem"
KEY="$OUT/cert.key"
FP="$OUT/cert-fingerprint.txt"
HASH="$OUT/cert-hash.txt"

# Only regenerate if the cert doesn't exist or is expired/about to expire
if [ -f "$CERT" ] && openssl x509 -in "$CERT" -checkend 3600 -noout 2>/dev/null; then
  echo "cert still valid, reusing $CERT"
else
  openssl req -x509 -nodes -days 10 \
    -newkey ec:<(openssl ecparam -name prime256v1) \
    -keyout "$KEY" \
    -out "$CERT" \
    -subj "/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" \
    2>/dev/null
  echo "generated new cert: $CERT"
fi

# SPKI fingerprint (for --ignore-certificate-errors-spki-list)
FINGERPRINT=$(openssl x509 -in "$CERT" -pubkey -noout 2>/dev/null \
  | openssl pkey -pubin -outform der 2>/dev/null \
  | openssl dgst -sha256 -binary \
  | openssl enc -base64 -A)
echo -n "$FINGERPRINT" > "$FP"
echo "SPKI fingerprint: $FINGERPRINT"

# SHA-256 of the full DER certificate (for WebTransport serverCertificateHashes)
CERT_HASH=$(openssl x509 -in "$CERT" -outform der 2>/dev/null \
  | openssl dgst -sha256 -binary \
  | openssl enc -base64 -A)
echo -n "$CERT_HASH" > "$HASH"
echo "Cert hash (serverCertificateHashes): $CERT_HASH"
