#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATEWAY_CONTAINER="${BPANE_GATEWAY_CONTAINER:-deploy-gateway-1}"
KEYCLOAK_URL="${BPANE_KEYCLOAK_URL:-http://localhost:8091}"
BROKER_URL="${BPANE_RUNTIME_BROKER_INTERNAL_URL:-http://runtime-broker:8940}"
HOST_IMAGE="${BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE:-$(docker image inspect deploy-host --format '{{.Id}}')}"
WORK_DIR="$(mktemp -d)"
SOURCE_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
CLONE_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
EMPTY_CONTEXT_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
SESSION_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
EMPTY_CONTEXT_SESSION_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
SOURCE_VOLUME="deploy_bpane-session-data-browser-context-${SOURCE_ID//-/}"
CLONE_VOLUME="deploy_bpane-session-data-browser-context-${CLONE_ID//-/}"
EMPTY_CONTEXT_VOLUME="deploy_bpane-session-data-browser-context-${EMPTY_CONTEXT_ID//-/}"
SESSION_VOLUME="deploy_bpane-session-data-${SESSION_ID//-/}"
EMPTY_CONTEXT_SESSION_VOLUME="deploy_bpane-session-data-${EMPTY_CONTEXT_SESSION_ID//-/}"

cleanup() {
  docker exec "$GATEWAY_CONTAINER" sh -c \
    'rm /tmp/bpane-storage-smoke-input.tgz /tmp/bpane-storage-smoke-export.tgz /tmp/bpane-storage-smoke-headers 2>/dev/null || true' \
    >/dev/null 2>&1 || true
  for volume in "$SOURCE_VOLUME" "$CLONE_VOLUME" "$EMPTY_CONTEXT_VOLUME" \
    "$SESSION_VOLUME" "$EMPTY_CONTEXT_SESSION_VOLUME"; do
    docker volume rm "$volume" >/dev/null 2>&1 || true
  done
  rm "$WORK_DIR/input.tgz" "$WORK_DIR/export.tgz" 2>/dev/null || true
  rm "$WORK_DIR/profile/Default/Preferences" "$WORK_DIR/profile/Default/StoragePayload" \
    2>/dev/null || true
  rmdir "$WORK_DIR/profile/Default" "$WORK_DIR/profile" "$WORK_DIR" 2>/dev/null || true
}
trap cleanup EXIT

mkdir -p "$WORK_DIR/profile/Default"
printf 'storage-smoke-profile\n' >"$WORK_DIR/profile/Default/Preferences"
dd if=/dev/urandom of="$WORK_DIR/profile/Default/StoragePayload" \
  bs=1048576 count=4 status=none
tar -czf "$WORK_DIR/input.tgz" -C "$WORK_DIR/profile" .
ARCHIVE_BYTES="$(wc -c <"$WORK_DIR/input.tgz" | tr -d ' ')"
docker cp "$WORK_DIR/input.tgz" "$GATEWAY_CONTAINER:/tmp/bpane-storage-smoke-input.tgz"

CLIENT_SECRET="$(tr -d '\r\n' <"$ROOT_DIR/deploy/runtime-broker/gateway-client-secret")"
TOKEN="$(
  curl -fsS -X POST \
    "$KEYCLOAK_URL/realms/browserpane-dev/protocol/openid-connect/token" \
    -H 'content-type: application/x-www-form-urlencoded' \
    --data-urlencode 'grant_type=client_credentials' \
    --data-urlencode 'client_id=bpane-runtime-broker-gateway' \
    --data-urlencode "client_secret=$CLIENT_SECRET" |
    jq -er '.access_token'
)"

operation_json() {
  local action="$1" session_id="$2" source_id="$3" target_id="$4" declared="$5" file_target="$6"
  local request_id
  request_id="$(uuidgen | tr '[:upper:]' '[:lower:]')"
  jq -cn \
    --arg request_id "$request_id" \
    --arg action "$action" \
    --arg session_id "$session_id" \
    --arg source_id "$source_id" \
    --arg target_id "$target_id" \
    --arg declared "$declared" \
    --argjson file_target "$file_target" \
    '{
      api_version: "v1",
      request_id: $request_id,
      idempotency_key: ("storage:live:" + $request_id),
      operation: {
        kind: "run_storage_helper",
        parameters: {
          action: $action,
          session_id: (if $session_id == "" then null else $session_id end),
          source_context_id: (if $source_id == "" then null else $source_id end),
          target_context_id: (if $target_id == "" then null else $target_id end),
          file_target: $file_target,
          declared_payload_bytes: (if $declared == "" then null else ($declared | tonumber) end)
        }
      }
    }'
}

run_json_operation() {
  local action="$1" session_id="$2" source_id="$3" target_id="$4" declared="$5" file_target="$6"
  local request response http_code state
  request="$(operation_json "$action" "$session_id" "$source_id" "$target_id" "$declared" "$file_target")"
  if [[ "$action" == "import_browser_context" ]]; then
    response="$(
      docker exec -e TOKEN="$TOKEN" -e REQUEST="$request" "$GATEWAY_CONTAINER" sh -ec \
        'curl -sS -w "\n%{http_code}" -X POST "$0/v1/storage-transfers" -H "Authorization: Bearer $TOKEN" -F "request=$REQUEST;type=application/vnd.browserpane.runtime-broker.v1+json" -F "payload=@/tmp/bpane-storage-smoke-input.tgz;type=application/octet-stream"' \
        "$BROKER_URL"
    )"
  elif [[ "$action" == "materialize_session_files" ]]; then
    response="$(
      docker exec -e TOKEN="$TOKEN" -e REQUEST="$request" "$GATEWAY_CONTAINER" sh -ec \
        'curl -sS -w "\n%{http_code}" -X POST "$0/v1/storage-transfers" -H "Authorization: Bearer $TOKEN" -F "request=$REQUEST;type=application/vnd.browserpane.runtime-broker.v1+json" -F "payload=smoke-data;type=application/octet-stream"' \
        "$BROKER_URL"
    )"
  else
    response="$(
      docker exec -e TOKEN="$TOKEN" -e REQUEST="$request" "$GATEWAY_CONTAINER" sh -ec \
        'curl -sS -w "\n%{http_code}" -X POST "$0/v1/storage-transfers" -H "Authorization: Bearer $TOKEN" -F "request=$REQUEST;type=application/vnd.browserpane.runtime-broker.v1+json"' \
        "$BROKER_URL"
    )"
  fi
  http_code="$(printf '%s' "$response" | tail -n 1)"
  state="$(printf '%s' "$response" | sed '$d' | jq -er '.result.state')"
  printf '%-28s HTTP %s %s\n' "$action" "$http_code" "$state"
  [[ "$http_code" == "202" ]]
}

run_json_operation import_browser_context '' '' "$SOURCE_ID" "$ARCHIVE_BYTES" null
run_json_operation measure_browser_context '' "$SOURCE_ID" '' '' null
run_json_operation clone_browser_context '' "$SOURCE_ID" "$CLONE_ID" '' null
run_json_operation initialize_session_data "$EMPTY_CONTEXT_SESSION_ID" '' "$EMPTY_CONTEXT_ID" '' null
run_json_operation initialize_session_data "$SESSION_ID" '' "$SOURCE_ID" '' null
run_json_operation materialize_session_files "$SESSION_ID" '' '' 10 \
  '{"purpose":"session_binding_manifest"}'

docker run --rm \
  --network none \
  --user bpane:bpane \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --mount "type=volume,source=$SESSION_VOLUME,target=/run/bpane/storage-helper/session" \
  "$HOST_IMAGE" \
  /bin/sh -ec 'test "$(cat /run/bpane/storage-helper/session/session-file-bindings.json)" = smoke-data'

EXPORT_REQUEST="$(operation_json export_browser_context '' "$CLONE_ID" '' '' null)"
docker exec -e TOKEN="$TOKEN" -e REQUEST="$EXPORT_REQUEST" "$GATEWAY_CONTAINER" sh -ec \
  'curl -fsS -D /tmp/bpane-storage-smoke-headers -o /tmp/bpane-storage-smoke-export.tgz -X POST "$0/v1/storage-transfers" -H "Authorization: Bearer $TOKEN" -F "request=$REQUEST;type=application/vnd.browserpane.runtime-broker.v1+json"' \
  "$BROKER_URL"
docker cp "$GATEWAY_CONTAINER:/tmp/bpane-storage-smoke-export.tgz" "$WORK_DIR/export.tgz" >/dev/null

EXPORT_BYTES="$(wc -c <"$WORK_DIR/export.tgz" | tr -d ' ')"
EXPORT_SHA256="$(shasum -a 256 "$WORK_DIR/export.tgz" | cut -d ' ' -f 1)"
HEADERS="$(docker exec "$GATEWAY_CONTAINER" cat /tmp/bpane-storage-smoke-headers | tr -d '\r')"
[[ "$(printf '%s\n' "$HEADERS" | awk -F': ' 'tolower($1)=="x-bpane-payload-bytes"{print $2}')" == "$EXPORT_BYTES" ]]
[[ "$(printf '%s\n' "$HEADERS" | awk -F': ' 'tolower($1)=="x-bpane-payload-sha256"{print $2}')" == "$EXPORT_SHA256" ]]
tar -xOf "$WORK_DIR/export.tgz" ./Default/Preferences | grep -qx 'storage-smoke-profile'
printf '%-28s HTTP 200 bytes=%s digest=verified content=verified\n' export_browser_context "$EXPORT_BYTES"

run_json_operation delete_session_data "$SESSION_ID" '' '' '' null
run_json_operation delete_session_data "$EMPTY_CONTEXT_SESSION_ID" '' '' '' null
run_json_operation delete_browser_context '' "$SOURCE_ID" '' '' null
run_json_operation delete_browser_context '' "$CLONE_ID" '' '' null
run_json_operation delete_browser_context '' "$EMPTY_CONTEXT_ID" '' '' null

for volume in "$SOURCE_VOLUME" "$CLONE_VOLUME" "$EMPTY_CONTEXT_VOLUME" \
  "$SESSION_VOLUME" "$EMPTY_CONTEXT_SESSION_VOLUME"; do
  if docker volume inspect "$volume" >/dev/null 2>&1; then
    printf 'volume was not removed: %s\n' "$volume" >&2
    exit 1
  fi
done
if [[ -n "$(docker ps -a --filter name=bpane-storage-helper --format '{{.Names}}')" ]]; then
  printf 'storage helper containers were not removed\n' >&2
  exit 1
fi
if docker volume ls --format '{{.Name}}' | grep -q '^bpane-storage-helper-input-'; then
  printf 'storage helper input volumes were not removed\n' >&2
  exit 1
fi

echo 'runtime broker storage smoke passed'
