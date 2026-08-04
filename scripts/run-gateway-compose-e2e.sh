#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/deploy/compose.yml"
SUITE="default"
TEARDOWN=0

usage() {
  cat <<'EOF'
Usage: scripts/run-gateway-compose-e2e.sh [--suite default|docker-pool|all|stack] [--teardown]

Runs the bpane-gateway compose-backed API e2e suites with stack preflight:
- verifies docker, docker compose, curl, and suite-specific tools are available
- refreshes local dev certs
- brings up the local compose stack
- waits for Keycloak, gateway, and mcp-bridge readiness
- verifies session-control parity against the compose Postgres database
- runs the selected ignored Rust integration test target(s), unless stack-only

Options:
  --suite      default | docker-pool | all | stack   (default: default)
               stack prepares the runtime and exits without running API tests
  --teardown   bring the compose stack down after the run
  --help       show this message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --suite)
      if [[ $# -lt 2 ]]; then
        echo "--suite requires a value" >&2
        usage >&2
        exit 2
      fi
      SUITE="${2:-}"
      shift 2
      ;;
    --teardown)
      TEARDOWN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$SUITE" in
  default|docker-pool|all|stack) ;;
  *)
    echo "invalid --suite value: $SUITE" >&2
    usage >&2
    exit 2
    ;;
esac

compose() {
  docker compose -f "$COMPOSE_FILE" "$@"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command not found: $1" >&2
    exit 1
  fi
}

wait_for_http() {
  local name="$1"
  local url="$2"
  local max_attempts="${3:-60}"
  local sleep_seconds="${4:-2}"
  local attempt=1

  until curl -fsS "$url" >/dev/null 2>&1; do
    if (( attempt >= max_attempts )); then
      echo "timed out waiting for $name at $url" >&2
      exit 1
    fi
    sleep "$sleep_seconds"
    attempt=$((attempt + 1))
  done
}

wait_for_gateway_api() {
  wait_for_http "gateway dependency readiness" "http://localhost:8932/readyz"
}

cleanup() {
  if (( TEARDOWN == 1 )); then
    compose down -v --remove-orphans
  fi
}

trap cleanup EXIT

require_command docker
require_command curl
if [[ "$SUITE" != "stack" ]]; then
  require_command cargo
fi

mkdir -p "$ROOT_DIR/dev/certs"
"$ROOT_DIR/deploy/gen-dev-cert.sh" "$ROOT_DIR/dev/certs" >/dev/null

compose up -d --build keycloak postgres host gateway mcp-bridge web

wait_for_http \
  "Keycloak realm metadata" \
  "http://localhost:8091/realms/browserpane-dev/.well-known/openid-configuration"
wait_for_gateway_api
wait_for_http "mcp-bridge health" "http://localhost:8931/health"

run_default_suite() {
  cargo test -p bpane-gateway --test compose_api_surface -- --ignored --test-threads=1
}

run_docker_pool_suite() {
  cargo test -p bpane-gateway --test compose_api_surface_docker_pool -- --ignored --test-threads=1
}

run_session_store_contract() {
  local database_url="${BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL:-postgresql://browserpane:browserpane-dev@localhost:5433/browserpane}"
  BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL="$database_url" \
    cargo test -p bpane-gateway session_store_contract_postgres -- --ignored --test-threads=1
}

if [[ "$SUITE" != "stack" ]]; then
  run_session_store_contract
fi

case "$SUITE" in
  stack)
    ;;
  default)
    run_default_suite
    ;;
  docker-pool)
    run_docker_pool_suite
    ;;
  all)
    run_default_suite
    run_docker_pool_suite
    ;;
esac
