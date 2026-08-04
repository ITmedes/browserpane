#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <image> <expected-version>" >&2
  exit 2
fi

image="$1"
expected_version="$2"

metadata="$(docker image inspect "$image" --format '{{json .}}')"
architecture="$(jq -r '.Architecture' <<<"$metadata")"
source="$(jq -r '.Config.Labels["org.opencontainers.image.source"]' <<<"$metadata")"
version="$(jq -r '.Config.Labels["org.opencontainers.image.version"]' <<<"$metadata")"

[[ "$architecture" == "amd64" ]] || {
  echo "Expected amd64 builder image, found: $architecture" >&2
  exit 1
}
[[ "$source" == "https://github.com/ITmedes/browserpane" ]] || {
  echo "Unexpected builder source label: $source" >&2
  exit 1
}
[[ "$version" == "$expected_version" ]] || {
  echo "Unexpected builder version label: $version" >&2
  exit 1
}

docker run --rm --platform linux/amd64 "$image" sh -lc '
  set -eu
  test "$(rustc --version | cut -d" " -f2)" = "1.93.1"
  test "$(cargo --version | cut -d" " -f2)" = "1.93.1"
  test -d /build/target/release/deps
  test ! -e /build/code
  test ! -e /build/Cargo.toml
  test ! -e /workspace
  ! find /build /tmp -xdev -type f \( \
    -name "*.pem" -o -name "*.key" \
  \) -print -quit 2>/dev/null | grep -q .
  ! find / -xdev -type f -name ".github_token" -print -quit 2>/dev/null | grep -q .
'

echo "CI Rust builder contract passed for $image"
