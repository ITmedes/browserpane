#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${BPANE_COMPOSE_FILE:-$ROOT_DIR/deploy/compose.yml}"
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

owned_dynamic_containers() {
  local container_id
  local container_ids
  local container_namespace
  if ! container_ids="$(docker ps --all --quiet --filter "label=browserpane.ci_namespace")"; then
    return 1
  fi
  while IFS= read -r container_id; do
    [[ -n "$container_id" ]] || continue
    if ! container_namespace="$(docker inspect --format '{{ index .Config.Labels "browserpane.ci_namespace" }}' "$container_id")"; then
      return 1
    fi
    if [[ "$container_namespace" == "$CI_NAMESPACE" || "$container_namespace" == "$CI_NAMESPACE"-* ]]; then
      printf '%s\n' "$container_id"
    fi
  done <<<"$container_ids"
}

if [[ -n "$CI_NAMESPACE" ]]; then
  owned_matches=""
  if ! owned_matches="$(owned_dynamic_containers)"; then
    CLEANUP_STATUS=1
  else
    while IFS= read -r container_id; do
      [[ -n "$container_id" ]] || continue
      docker rm --force "$container_id" >/dev/null || CLEANUP_STATUS=1
    done <<<"$owned_matches"
  fi
  if ! owned_matches="$(owned_dynamic_containers)"; then
    CLEANUP_STATUS=1
  elif [[ -n "$owned_matches" ]]; then
    echo "cleanup invariant failed: CI-owned BrowserPane containers remain" >&2
    CLEANUP_STATUS=1
  fi
fi

exit "$CLEANUP_STATUS"
