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
operating_system="$(jq -r '.Os' <<<"$metadata")"
source="$(jq -r '.Config.Labels["org.opencontainers.image.source"]' <<<"$metadata")"
version="$(jq -r '.Config.Labels["org.opencontainers.image.version"]' <<<"$metadata")"
license="$(jq -r '.Config.Labels["org.opencontainers.image.licenses"]' <<<"$metadata")"

[[ "$architecture" == "amd64" ]] || {
  echo "Expected amd64 builder image, found: $architecture" >&2
  exit 1
}
[[ "$operating_system" == "linux" ]] || {
  echo "Expected Linux builder image, found: $operating_system" >&2
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
[[ "$license" == "AGPL-3.0-only" ]] || {
  echo "Unexpected builder license label: $license" >&2
  exit 1
}

history="$(docker history --no-trunc "$image" --format '{{.CreatedBy}}')"
if grep -Eiq '(\.github_token|GITHUB_TOKEN|BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY)' <<<"$history"; then
  echo "Builder image history contains credential material" >&2
  exit 1
fi

docker run --rm --platform linux/amd64 "$image" sh -lc '
  set -eu
  test "$(rustc --version | cut -d" " -f2)" = "1.93.1"
  test "$(cargo --version | cut -d" " -f2)" = "1.93.1"
  command -v cc >/dev/null
  command -v cmake >/dev/null
  command -v pkg-config >/dev/null
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
