#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${BPANE_COMPOSE_FILE:-$ROOT_DIR/deploy/compose.yml}"
RECORDING_IMAGE="${BPANE_RECORDING_WORKER_IMAGE:-deploy-recording-worker}"

remove_containers() {
  local filter_kind="$1"
  local filter_value="$2"
  local container_id

  while IFS= read -r container_id; do
    if [[ -n "$container_id" ]]; then
      docker rm --force "$container_id" >/dev/null 2>&1 || true
    fi
  done < <(docker ps --all --quiet --filter "$filter_kind=$filter_value" 2>/dev/null || true)
}

remove_containers name bpane-runtime-
remove_containers name bpane-workflow-
remove_containers ancestor "$RECORDING_IMAGE"

docker compose -f "$COMPOSE_FILE" down --volumes --remove-orphans || true
