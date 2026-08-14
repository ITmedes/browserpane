#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ROOT_DIR}/deploy/single-node/generated/fixture.env"

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "single-node fixture environment does not exist" >&2
  exit 1
fi

docker ps \
  --filter label=browserpane.runtime.operation \
  --format '{{.Names}}' |
  while IFS= read -r container_name; do
    case "${container_name}" in
      bpane-single-node-fixture-*) docker stop "${container_name}" >/dev/null ;;
    esac
  done

docker compose \
  --project-name bpane-single-node-fixture \
  --env-file "${ENV_FILE}" \
  -f "${ROOT_DIR}/deploy/single-node/compose.yml" \
  -f "${ROOT_DIR}/deploy/single-node/fixture/compose.yml" \
  down
