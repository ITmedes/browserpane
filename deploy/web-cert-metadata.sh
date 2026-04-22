#!/bin/sh
set -eu

cert="/usr/share/nginx/html/certs/cert.pem"
hash_output="/usr/share/nginx/html/cert-hash.txt"
fingerprint_output="/usr/share/nginx/html/cert-fingerprint.txt"

if [ ! -f "$cert" ]; then
  exit 0
fi

openssl x509 -in "$cert" -outform der 2>/dev/null \
  | openssl dgst -sha256 -binary \
  | openssl enc -base64 -A > "$hash_output"

openssl x509 -in "$cert" -pubkey -noout 2>/dev/null \
  | openssl pkey -pubin -outform der 2>/dev/null \
  | openssl dgst -sha256 -binary \
  | openssl enc -base64 -A > "$fingerprint_output"
