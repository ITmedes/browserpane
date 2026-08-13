#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_COMPOSE="${ROOT_DIR}/deploy/compose.yml"
BROKER_COMPOSE="${ROOT_DIR}/deploy/compose.runtime-broker.yml"

cd "${ROOT_DIR}"
docker compose -f "${BASE_COMPOSE}" build host runtime-broker gateway

browser_image_id="$(docker compose -f "${BASE_COMPOSE}" images -q host)"
if [[ ! "${browser_image_id}" =~ ^sha256:[0-9a-fA-F]{64}$ ]]; then
  echo "failed to resolve an immutable browser image id" >&2
  exit 1
fi

export BPANE_RUNTIME_BROKER_BROWSER_IMAGE="${browser_image_id}"
export BPANE_GATEWAY_RUNTIME_BACKEND=broker_pool

exec docker compose -f "${BASE_COMPOSE}" -f "${BROKER_COMPOSE}" up -d "$@"
