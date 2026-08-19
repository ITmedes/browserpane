#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OBSERVER_DIR="$ROOT_DIR/deploy/examples/egress-observer"
OBSERVER_COMPOSE_FILE="$OBSERVER_DIR/compose.yml"
TLS_COMPOSE_FILE="$OBSERVER_DIR/compose.tls.yml"
OBSERVER_PROJECT="${BPANE_EGRESS_OBSERVER_PROJECT:-bpane-ci-egress}"
TLS_PROJECT="${BPANE_EGRESS_TLS_OBSERVER_PROJECT:-bpane-ci-egress-tls}"
OBSERVER_NETWORK="${BPANE_EGRESS_OBSERVER_NETWORK:-deploy_bpane-internal}"
CA_CERTIFICATE="$ROOT_DIR/dev/egress-ca.pem"
CA_PRIVATE_KEY="$ROOT_DIR/dev/egress-ca.key"
MITMPROXY_CA_DIR="$ROOT_DIR/dev/egress-mitmproxy"
PLAIN_PORT="${BPANE_EGRESS_OBSERVER_PORT:-3128}"
AUTH_PORT="${BPANE_EGRESS_AUTH_OBSERVER_PORT:-3130}"
TLS_PORT="${BPANE_EGRESS_TLS_OBSERVER_PORT:-3129}"
MAX_ATTEMPTS="${BPANE_EGRESS_FIXTURE_MAX_ATTEMPTS:-30}"
SLEEP_SECONDS="${BPANE_EGRESS_FIXTURE_SLEEP_SECONDS:-2}"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command not found: $1" >&2
    exit 1
  fi
}

observer_compose() {
  docker compose --project-name "$OBSERVER_PROJECT" -f "$OBSERVER_COMPOSE_FILE" "$@"
}

tls_compose() {
  docker compose --project-name "$TLS_PROJECT" -f "$TLS_COMPOSE_FILE" "$@"
}

print_container_state() {
  local container_name

  echo "Egress fixture container state:" >&2
  for container_name in \
    bpane-egress-observer \
    bpane-egress-auth-observer \
    bpane-egress-tls-observer; do
    docker inspect \
      --format '{{.Name}} status={{.State.Status}} exit_code={{.State.ExitCode}} error={{.State.Error}}' \
      "$container_name" >&2 2>/dev/null || echo "/$container_name status=missing" >&2
  done
}

report_failure() {
  local exit_code=$?

  if (( exit_code != 0 )); then
    print_container_state
  fi
  exit "$exit_code"
}

wait_for_proxy() {
  local name="$1"
  local proxy_url="$2"
  local proxy_credentials="${3:-}"
  local attempt=1
  local curl_args=(
    --fail
    --silent
    --show-error
    --max-time 5
    --output /dev/null
    --proxy "$proxy_url"
  )

  if [[ -n "$proxy_credentials" ]]; then
    curl_args+=(--proxy-user "$proxy_credentials")
  fi

  until curl "${curl_args[@]}" http://example.com/; do
    if (( attempt >= MAX_ATTEMPTS )); then
      echo "timed out waiting for $name through $proxy_url" >&2
      return 1
    fi
    sleep "$SLEEP_SECONDS"
    attempt=$((attempt + 1))
  done

  echo "$name is reachable through $proxy_url"
}

prepare_test_ca() {
  if [[ ! -e "$CA_CERTIFICATE" && ! -e "$CA_PRIVATE_KEY" ]]; then
    mkdir -p "$(dirname "$CA_CERTIFICATE")"
    umask 077
    openssl req \
      -x509 \
      -newkey rsa:2048 \
      -sha256 \
      -nodes \
      -days 1 \
      -subj '/CN=BrowserPane CI Egress Test CA' \
      -keyout "$CA_PRIVATE_KEY" \
      -out "$CA_CERTIFICATE" \
      >/dev/null 2>&1
    echo "Generated an ephemeral egress test CA."
  elif [[ ! -r "$CA_CERTIFICATE" || ! -r "$CA_PRIVATE_KEY" ]]; then
    echo "Egress CA certificate and private key must either both be absent or both be readable." >&2
    return 1
  fi

  "$OBSERVER_DIR/prepare-mitmproxy-ca.sh" \
    "$CA_CERTIFICATE" \
    "$CA_PRIVATE_KEY" \
    "$MITMPROXY_CA_DIR"
}

trap report_failure EXIT

require_command docker
require_command curl
require_command openssl
docker compose version >/dev/null
docker network inspect "$OBSERVER_NETWORK" >/dev/null

prepare_test_ca
export BPANE_EGRESS_OBSERVER_NETWORK="$OBSERVER_NETWORK"

observer_compose up --detach --build
tls_compose up --detach

wait_for_proxy "plain egress observer" "http://127.0.0.1:$PLAIN_PORT"
wait_for_proxy \
  "authenticated egress observer" \
  "http://127.0.0.1:$AUTH_PORT" \
  "proxy-user:proxy-pass"
wait_for_proxy "TLS-intercept egress observer" "http://127.0.0.1:$TLS_PORT"

trap - EXIT
echo "All compose egress fixtures are ready on $OBSERVER_NETWORK."
