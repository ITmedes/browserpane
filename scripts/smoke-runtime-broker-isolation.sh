#!/usr/bin/env bash
set -euo pipefail

GATEWAY_CONTAINER="${BPANE_GATEWAY_CONTAINER:-deploy-gateway-1}"
BROKER_CONTAINER="${BPANE_RUNTIME_BROKER_CONTAINER:-deploy-runtime-broker-1}"
PROXY_CONTAINER="${BPANE_DOCKER_PROXY_CONTAINER:-deploy-docker-proxy-1}"

require_running() {
  local container="$1"
  if [[ "$(docker inspect --format '{{.State.Running}}' "$container" 2>/dev/null)" != "true" ]]; then
    printf 'required container is not running: %s\n' "$container" >&2
    exit 1
  fi
}

require_running "$GATEWAY_CONTAINER"
require_running "$BROKER_CONTAINER"
require_running "$PROXY_CONTAINER"

if docker inspect --format '{{range .Config.Env}}{{println .}}{{end}}' "$GATEWAY_CONTAINER" |
  grep -q '^DOCKER_HOST='; then
  echo 'gateway must not receive DOCKER_HOST in broker topology' >&2
  exit 1
fi

if docker inspect --format '{{range .Mounts}}{{println .Source " -> " .Destination}}{{end}}' \
  "$GATEWAY_CONTAINER" | grep -q 'docker.sock'; then
  echo 'gateway must not mount the Docker socket in broker topology' >&2
  exit 1
fi

if docker inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{println $name}}{{end}}' \
  "$GATEWAY_CONTAINER" | grep -q 'docker-control$'; then
  echo 'gateway must not join docker-control in broker topology' >&2
  exit 1
fi

docker exec "$GATEWAY_CONTAINER" sh -ec '
  test ! -S /var/run/docker.sock
  ! getent hosts docker-proxy >/dev/null 2>&1
  ! curl -fsS --connect-timeout 2 --max-time 3 http://docker-proxy:2375/_ping >/dev/null 2>&1
  curl -fsS --connect-timeout 2 --max-time 3 http://runtime-broker:8940/readyz >/dev/null
'

docker exec "$BROKER_CONTAINER" curl -fsS --connect-timeout 2 --max-time 3 \
  http://docker-proxy:2375/_ping | grep -qx 'OK'

echo 'runtime broker gateway isolation smoke passed'
