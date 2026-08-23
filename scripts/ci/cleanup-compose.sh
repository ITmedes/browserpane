#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${BPANE_COMPOSE_FILE:-$ROOT_DIR/deploy/compose.yml}"
RECORDING_IMAGE="${BPANE_RECORDING_WORKER_IMAGE:-deploy-recording-worker}"
OBSERVER_COMPOSE_FILE="$ROOT_DIR/deploy/examples/egress-observer/compose.yml"
TLS_COMPOSE_FILE="$ROOT_DIR/deploy/examples/egress-observer/compose.tls.yml"
RUN_NAMESPACE="${BPANE_CI_RUN_NAMESPACE:-}"
RUN_NAMESPACE_SUFFIX="${RUN_NAMESPACE:0:32}"
OBSERVER_PROJECT="${BPANE_EGRESS_OBSERVER_PROJECT:-bpane-ci-egress${RUN_NAMESPACE_SUFFIX:+-$RUN_NAMESPACE_SUFFIX}}"
TLS_PROJECT="${BPANE_EGRESS_TLS_OBSERVER_PROJECT:-bpane-ci-egress-tls${RUN_NAMESPACE_SUFFIX:+-$RUN_NAMESPACE_SUFFIX}}"
CLEANUP_STATUS=0
CI_NAMESPACE="$RUN_NAMESPACE"

if [[ -n "$CI_NAMESPACE" && ! "$CI_NAMESPACE" =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]]; then
  echo "invalid BPANE_CI_RUN_NAMESPACE" >&2
  exit 1
fi

docker compose \
  --project-name "$TLS_PROJECT" \
  -f "$TLS_COMPOSE_FILE" \
  down --volumes --remove-orphans || CLEANUP_STATUS=1
docker compose \
  --project-name "$OBSERVER_PROJECT" \
  -f "$OBSERVER_COMPOSE_FILE" \
  down --volumes --remove-orphans || CLEANUP_STATUS=1
docker compose -f "$COMPOSE_FILE" down --volumes --remove-orphans || CLEANUP_STATUS=1

assert_no_dynamic_containers() {
  local filter_value="$1"
  local matches
  if ! matches="$(docker ps --all --quiet --filter "name=$filter_value")"; then
    CLEANUP_STATUS=1
  elif [[ -n "$matches" ]]; then
    echo "cleanup invariant failed: dynamic BrowserPane containers remain" >&2
    CLEANUP_STATUS=1
  fi
}

if [[ -n "$CI_NAMESPACE" ]]; then
  assert_no_dynamic_containers bpane-runtime-
  assert_no_dynamic_containers bpane-workflow-
  recording_matches=""
  if ! recording_matches="$(docker ps --all --quiet --filter "ancestor=$RECORDING_IMAGE")"; then
    CLEANUP_STATUS=1
  elif [[ -n "$recording_matches" ]]; then
    echo "cleanup invariant failed: recording worker containers remain" >&2
    CLEANUP_STATUS=1
  fi
fi

exit "$CLEANUP_STATUS"
