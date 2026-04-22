#!/bin/sh
set -eu

template="/usr/share/nginx/html/auth-config.template.json"
output="/usr/share/nginx/html/auth-config.json"

if [ ! -f "$template" ]; then
  exit 0
fi

envsubst < "$template" > "$output"
