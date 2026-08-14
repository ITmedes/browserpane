#!/bin/sh
set -eu

template="/usr/share/nginx/html/auth-config.template.json"
output="/run/browserpane/auth-config.json"

mkdir -p "$(dirname "$output")"
envsubst < "$template" > "$output"
