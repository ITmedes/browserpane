#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_COMPOSE="${ROOT_DIR}/deploy/compose.yml"
BROKER_COMPOSE="${ROOT_DIR}/deploy/compose.runtime-broker.yml"

cd "${ROOT_DIR}"
docker compose --profile workflow -f "${BASE_COMPOSE}" build \
  host runtime-broker gateway workflow-worker recording-worker-image

immutable_image_id() {
  local image_ref="$1"
  local image_id
  image_id="$(docker image inspect "${image_ref}" --format '{{.Id}}')"
  if [[ "${image_id}" =~ ^[0-9a-fA-F]{64}$ ]]; then
    image_id="sha256:${image_id}"
  fi
  if [[ ! "${image_id}" =~ ^sha256:[0-9a-fA-F]{64}$ ]]; then
    echo "failed to resolve an immutable image id for ${image_ref}" >&2
    exit 1
  fi
  printf '%s' "${image_id}"
}

export BPANE_RUNTIME_BROKER_BROWSER_IMAGE="$(immutable_image_id "${BPANE_GATEWAY_DOCKER_RUNTIME_IMAGE:-deploy-host}")"
export BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE="$(immutable_image_id "${BPANE_WORKFLOW_WORKER_IMAGE:-deploy-workflow-worker}")"
export BPANE_RUNTIME_BROKER_RECORDING_IMAGE="$(immutable_image_id "${BPANE_RECORDING_WORKER_IMAGE:-deploy-recording-worker}")"
export BPANE_GATEWAY_RUNTIME_BACKEND=broker_pool

exec docker compose -f "${BASE_COMPOSE}" -f "${BROKER_COMPOSE}" up -d "$@"
