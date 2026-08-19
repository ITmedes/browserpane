#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${BPANE_COMPOSE_FILE:-$ROOT_DIR/deploy/compose.yml}"
RECORDING_IMAGE="${BPANE_RECORDING_WORKER_IMAGE:-deploy-recording-worker}"
OBSERVER_COMPOSE_FILE="$ROOT_DIR/deploy/examples/egress-observer/compose.yml"
TLS_COMPOSE_FILE="$ROOT_DIR/deploy/examples/egress-observer/compose.tls.yml"
OBSERVER_PROJECT="${BPANE_EGRESS_OBSERVER_PROJECT:-bpane-ci-egress}"
TLS_PROJECT="${BPANE_EGRESS_TLS_OBSERVER_PROJECT:-bpane-ci-egress-tls}"

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

docker compose \
  --project-name "$TLS_PROJECT" \
  -f "$TLS_COMPOSE_FILE" \
  down --volumes --remove-orphans || true
docker compose \
  --project-name "$OBSERVER_PROJECT" \
  -f "$OBSERVER_COMPOSE_FILE" \
  down --volumes --remove-orphans || true
docker compose -f "$COMPOSE_FILE" down --volumes --remove-orphans || true
