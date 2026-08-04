#!/usr/bin/env sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <exact-rust-toolchain>" >&2
  exit 2
fi

toolchain="$1"
if ! printf '%s\n' "$toolchain" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "Rust toolchain must be an exact numeric version: $toolchain" >&2
  exit 2
fi

rustup_version=1.28.2
case "$(dpkg --print-architecture)" in
  amd64)
    rustup_target=x86_64-unknown-linux-gnu
    rustup_sha256=20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c
    ;;
  arm64)
    rustup_target=aarch64-unknown-linux-gnu
    rustup_sha256=e3853c5a252fca15252d07cb23a1bdd9377a8c6f3efa01531109281ae47f841c
    ;;
  *)
    echo "Unsupported Rust builder architecture: $(dpkg --print-architecture)" >&2
    exit 1
    ;;
esac

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  "https://static.rust-lang.org/rustup/archive/${rustup_version}/${rustup_target}/rustup-init" \
  --output /tmp/rustup-init
echo "${rustup_sha256}  /tmp/rustup-init" | sha256sum --check --strict
chmod +x /tmp/rustup-init
/tmp/rustup-init -y --profile minimal --default-toolchain none
rm /tmp/rustup-init
rustup set auto-self-update disable
rustup toolchain install "$toolchain" --profile minimal \
  --component clippy --component llvm-tools-preview --component rustfmt
rustup default "$toolchain"
