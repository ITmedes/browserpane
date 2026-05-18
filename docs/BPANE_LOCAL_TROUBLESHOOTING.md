# BrowserPane Local Troubleshooting Runbook

This runbook targets the checked-in local development stack in
`deploy/compose.yml`. It is not a production deployment guide.

## Docker Socket And Compose

The local gateway starts docker-backed browser and workflow workers through the
host Docker socket. Verify Docker before debugging BrowserPane services:

```bash
docker ps
docker compose -f deploy/compose.yml ps
```

If Docker requires `sudo`, preserve the BrowserPane environment explicitly:

```bash
sudo env \
  BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 \
  docker compose -f deploy/compose.yml up --build
```

For non-default compose project names, also align the runtime image, network,
socket volume, and session-data volume prefix documented in `README.md`.

Docker Compose may print a buildx/Bake delegation warning on some Docker
versions. That warning alone is not a BrowserPane failure; investigate only if
the image build or service start exits non-zero.

## Local Ports

The local web UI is `http://localhost:8080/admin/`. The gateway HTTP API is
`http://localhost:8932`, while browser WebTransport connects to
`https://localhost:4433`. Do not use the HTTP API port as the WebTransport URL.

The local stack also exposes:

- Keycloak on `:8091`
- Postgres on `:5433`
- Vault dev mode on `:8200`
- MCP bridge on `:8931`

## Workflow Source Checkout

Local workflow templates use a git source rooted at `/workspace`, mounted into
the gateway container as read-only source. The gateway image configures:

```bash
git config --system --add safe.directory /workspace
```

If a custom gateway image or shell session reports `dubious ownership`, add that
same `safe.directory` setting. Source errors returned by the API include a
workflow-source `code`, `category`, and `recovery_hint`; the admin app surfaces
those messages when the BrowserPane Tour template or a workflow run cannot be
resolved.

To manually check the mounted checkout from the gateway container:

```bash
docker compose -f deploy/compose.yml exec gateway git -C /workspace rev-parse HEAD
docker compose -f deploy/compose.yml exec gateway git ls-remote /workspace HEAD
```

Avoid running generators as root against the host checkout. Root-owned files in
the repo can cause local edit, build, or git metadata problems.

## Workflow Worker Image

The gateway auto-launches workflow workers with the `deploy-workflow-worker`
image. Rebuild it before workflow smokes after worker or dependency changes:

```bash
docker compose -f deploy/compose.yml build workflow-worker
```

API-only docker-pool sessions are lazy: a session resource can exist before a
runtime container starts. Connect through the admin app or invoke a workflow to
materialize the runtime.

## MCP Bridge

The MCP bridge runs the locally installed `@playwright/mcp` package from
`code/integrations/mcp-bridge/node_modules`. It should not download
`@playwright/mcp@latest` on first connect.

After rebuilding, inspect logs for accidental runtime installs:

```bash
docker compose -f deploy/compose.yml logs mcp-bridge | grep -E "npm warn exec|@playwright/mcp@latest"
```

No output is expected. If the bridge reports that the local executable is
missing, run `npm ci` in `code/integrations/mcp-bridge` or rebuild the image.

## Certificate Metadata

Regenerate local WebTransport certificate metadata with:

```bash
./deploy/gen-dev-cert.sh dev/certs
```

The web service serves:

- `http://localhost:8080/cert-fingerprint`
- `http://localhost:8080/cert-hash`

Both endpoints should be uncached. If WebTransport still fails after certificate
rotation, reload the admin app and confirm Chromium trusts the current SPKI
fingerprint:

```bash
curl -i http://localhost:8080/cert-fingerprint
curl -i http://localhost:8080/cert-hash
```

## Database Diagnostics

Postgres stores owner-scoped sessions, runtime assignments, workflow runs, and
recording metadata. Useful local checks:

```bash
docker compose -f deploy/compose.yml exec postgres \
  psql -U browserpane -d browserpane -c "select id, owner_subject, state, updated_at from control_sessions order by updated_at desc limit 10;"

docker compose -f deploy/compose.yml exec postgres \
  psql -U browserpane -d browserpane -c "select session_id, runtime_id, status, updated_at from control_session_runtimes order by updated_at desc limit 10;"
```

`control_sessions.runtime_binding` describes the intended runtime mode.
`control_session_runtimes` records concrete docker-pool runtime assignments that
can be reconciled after gateway restart.

## Camera Provisioning

Camera ingress is disabled in default compose. To test it, the host must provide
a `v4l2loopback` device and the compose override must map that device into the
runtime container. Unsupported states usually mean the browser lacks H.264
WebCodecs encode support or the expected video device was not mounted.
