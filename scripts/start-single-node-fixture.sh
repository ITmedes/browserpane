#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GENERATED_DIR="${ROOT_DIR}/deploy/single-node/generated"
SECRETS_DIR="${GENERATED_DIR}/secrets"
ENV_FILE="${GENERATED_DIR}/fixture.env"

mkdir -p "${SECRETS_DIR}"
chmod 700 "${GENERATED_DIR}" "${SECRETS_DIR}"

build_image() {
  local tag="$1"
  local dockerfile="$2"
  docker build -f "${dockerfile}" -t "${tag}" "${ROOT_DIR}"
}

immutable_image_id() {
  local image_ref="$1"
  local image_id
  image_id="$(docker image inspect "${image_ref}" --format '{{.Id}}')"
  if [[ "${image_id}" =~ ^[0-9a-fA-F]{64}$ ]]; then image_id="sha256:${image_id}"; fi
  if [[ ! "${image_id}" =~ ^sha256:[0-9a-fA-F]{64}$ ]]; then
    echo "failed to resolve immutable image id for ${image_ref}" >&2
    exit 1
  fi
  printf '%s' "${image_id}"
}

write_secret() {
  local filename="$1"
  local value="$2"
  printf '%s\n' "${value}" > "${filename}"
  chmod 600 "${filename}"
}

build_image bpane-single-node-web:fixture "${ROOT_DIR}/deploy/Dockerfile.web-production"
build_image bpane-single-node-gateway:fixture "${ROOT_DIR}/deploy/Dockerfile.gateway"
build_image bpane-single-node-broker:fixture "${ROOT_DIR}/deploy/Dockerfile.runtime-broker"
build_image bpane-single-node-browser:fixture "${ROOT_DIR}/deploy/Dockerfile.host"
build_image bpane-single-node-workflow:fixture "${ROOT_DIR}/deploy/Dockerfile.workflow-worker"
build_image bpane-single-node-recording:fixture "${ROOT_DIR}/deploy/Dockerfile.recording-worker"
docker pull ghcr.io/tecnativa/docker-socket-proxy@sha256:1f5038b54f06c3e18422902cf00ba21803d1c97805aae032e5e6673d532d3459

write_secret "${SECRETS_DIR}/database-url" \
  "postgres://browserpane:single-node-fixture@postgres:5432/browserpane"
write_secret "${SECRETS_DIR}/vault-token" "single-node-fixture-vault-token"
write_secret "${SECRETS_DIR}/broker-gateway-client-secret" \
  "bpane-runtime-broker-gateway-secret"
write_secret "${SECRETS_DIR}/worker-oidc-client-secret" "bpane-mcp-bridge-secret"
# The directory remains 0700; this local bind must be readable by the broker's fixed container UID.
chmod 644 "${SECRETS_DIR}/worker-oidc-client-secret"

"${ROOT_DIR}/deploy/gen-dev-cert.sh" "${GENERATED_DIR}/certs"

cat > "${ENV_FILE}" <<EOF
BPANE_DEPLOYMENT_NAME=bpane-single-node-fixture
BPANE_WEB_IMAGE=$(immutable_image_id bpane-single-node-web:fixture)
BPANE_GATEWAY_IMAGE=$(immutable_image_id bpane-single-node-gateway:fixture)
BPANE_RUNTIME_BROKER_IMAGE=$(immutable_image_id bpane-single-node-broker:fixture)
BPANE_DOCKER_PROXY_IMAGE=ghcr.io/tecnativa/docker-socket-proxy@sha256:1f5038b54f06c3e18422902cf00ba21803d1c97805aae032e5e6673d532d3459
BPANE_BROWSER_IMAGE=$(immutable_image_id bpane-single-node-browser:fixture)
BPANE_WORKFLOW_WORKER_IMAGE=$(immutable_image_id bpane-single-node-workflow:fixture)
BPANE_RECORDING_WORKER_IMAGE=$(immutable_image_id bpane-single-node-recording:fixture)
BPANE_STORAGE_HELPER_IMAGE=$(immutable_image_id bpane-single-node-browser:fixture)
BPANE_WEB_BIND_ADDRESS=127.0.0.1
BPANE_WEB_PORT=18080
BPANE_WEBTRANSPORT_PORT=14433
BPANE_PUBLIC_GATEWAY_URL=https://localhost:14433
BPANE_RECORDING_CONNECT_GATEWAY_URL=https://gateway:4433
BPANE_BROWSER_START_URL=https://example.org/
BPANE_OIDC_PROVIDER_HINT=keycloak
BPANE_OIDC_PUBLIC_ISSUER=http://localhost:18091/realms/browserpane-dev
BPANE_OIDC_INTERNAL_JWKS_URL=http://keycloak:8080/realms/browserpane-dev/protocol/openid-connect/certs
BPANE_OIDC_TOKEN_URL=http://keycloak:8080/realms/browserpane-dev/protocol/openid-connect/token
BPANE_OIDC_GATEWAY_AUDIENCE=bpane-gateway
BPANE_OIDC_BROWSER_CLIENT_ID=bpane-web
BPANE_OIDC_BROWSER_SCOPE=openid
BPANE_OIDC_BROKER_AUDIENCE=bpane-runtime-broker
BPANE_OIDC_BROKER_GATEWAY_CLIENT_ID=bpane-runtime-broker-gateway
BPANE_OIDC_WORKER_CLIENT_ID=bpane-mcp-bridge
BPANE_OIDC_WORKER_SCOPES=
BPANE_VAULT_ADDR=http://vault:8200
BPANE_VAULT_MOUNT_PATH=secret
BPANE_VAULT_PREFIX=browserpane/credential-bindings
BPANE_DATABASE_URL_FILE=${SECRETS_DIR}/database-url
BPANE_VAULT_TOKEN_FILE=${SECRETS_DIR}/vault-token
BPANE_BROKER_GATEWAY_CLIENT_SECRET_FILE=${SECRETS_DIR}/broker-gateway-client-secret
BPANE_WORKER_OIDC_CLIENT_SECRET_FILE=${SECRETS_DIR}/worker-oidc-client-secret
BPANE_TLS_CERT_FILE=${GENERATED_DIR}/certs/cert.pem
BPANE_TLS_KEY_FILE=${GENERATED_DIR}/certs/cert.key
BPANE_MAX_ACTIVE_RUNTIMES=4
BPANE_MAX_STARTING_RUNTIMES=1
BPANE_MAX_VIEWERS=10
BPANE_BROKER_MAX_CONCURRENT=16
BPANE_WORKFLOW_MAX_ACTIVE=2
BPANE_BROWSER_FPS=30
EOF
chmod 600 "${ENV_FILE}"

set -a
# shellcheck disable=SC1090
source "${ENV_FILE}"
set +a
node "${ROOT_DIR}/deploy/single-node/render-config.mjs" --output "${GENERATED_DIR}"
node "${ROOT_DIR}/deploy/single-node/fixture/render-fixture.mjs"

docker compose \
  --project-name bpane-single-node-fixture \
  --env-file "${ENV_FILE}" \
  -f "${ROOT_DIR}/deploy/single-node/compose.yml" \
  -f "${ROOT_DIR}/deploy/single-node/fixture/compose.yml" \
  up -d

echo "Single-node fixture started at http://localhost:18080/admin-new/"
echo "Environment: ${ENV_FILE}"
