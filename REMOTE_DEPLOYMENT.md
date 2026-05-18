# Remote Deployment Notes

BrowserPane's checked-in compose stack is a local development and regression
environment. It is useful for self-hosted experiments, but it is not a
production deployment recipe.

## Required Public URLs

Remote browser use requires a secure browser context. In practice that means
serving the web UI over HTTPS and exposing the gateway WebTransport endpoint
with a certificate the browser trusts.

Keep these values aligned when moving away from localhost:

- Web origin: the HTTPS origin that serves the admin console, `bpane-client`, and any test fixtures such as `/test-embed.html`.
- Gateway public URL: the externally reachable `https://...` WebTransport URL
  configured through `--public-gateway-url`.
- OIDC issuer: the issuer embedded in browser access tokens.
- OIDC redirect URI: the web origin path the identity provider redirects back
  to after Authorization Code + PKCE login.
- OIDC audience/JWKS: the audience and key set used by `bpane-gateway` to
  validate browser and service tokens.
- Certificate metadata: `/cert-fingerprint`, `/cert-hash`, and any local
  browser SPKI overrides must describe the same certificate used by the
  gateway WebTransport listener.

For a remote host, do not reuse localhost values such as
`http://localhost:8091`, `http://localhost:8080`, or
`https://localhost:4433` in browser-facing configuration. They only work when
the browser, Keycloak, web UI, and gateway all run on the same machine.

## Compose Defaults And Runtime Mode

`deploy/compose.yml` defaults to `docker_pool` for browser-session testing.
That mode assumes:

- the gateway can access the Docker socket;
- the runtime image is available as `deploy-host` unless overridden;
- the Docker network, socket volume, and session-data volume prefix match the
  compose project name;
- each browser session gets a separate Chromium profile, upload directory, and
  download directory;
- Postgres is available for persisted session-control and runtime-assignment
  metadata.

If you run compose with a non-default project name or with `sudo`, preserve the
same environment overrides explicitly:

```bash
sudo env \
  BPANE_GATEWAY_RUNTIME_BACKEND=docker_pool \
  BPANE_GATEWAY_MAX_ACTIVE_RUNTIMES=2 \
  BPANE_GATEWAY_DOCKER_RUNTIME_IMAGE=deploy-host \
  BPANE_GATEWAY_DOCKER_RUNTIME_NETWORK=deploy_bpane-internal \
  BPANE_GATEWAY_DOCKER_RUNTIME_SOCKET_VOLUME=deploy_agent-socket \
  BPANE_GATEWAY_DOCKER_RUNTIME_SESSION_DATA_VOLUME_PREFIX=deploy_bpane-session-data \
  docker compose -f deploy/compose.yml up --build
```

Adjust the `deploy_...` names if your compose project is not named `deploy`.
Without those overrides, the gateway can start with runtime settings that point
at non-existing images, networks, or volumes.

## Do Not Expose Dev Services Publicly

The local compose ports are intended for a trusted development machine:

- `:5433` Postgres contains session-control metadata.
- `:8200` Vault runs in dev mode for workflow credential binding tests.
- `:8091` Keycloak is a local demo realm with demo credentials.
- `:8932` gateway HTTP API is a control-plane API, not a public unauthenticated
  edge.
- `:8931` MCP bridge can drive delegated browser sessions.

For remote testing, put public traffic behind an HTTPS reverse proxy or tunnel
that exposes only the web UI and the gateway endpoints required by the browser.
Keep Postgres, Vault, Keycloak admin surfaces, gateway internals, and MCP bridge
control endpoints private unless a deployment-specific access-control design is
in place.

## Local Compose Is Not Production Guidance

The local stack prioritizes fast rebuilds, deterministic smoke tests, and
developer visibility. A production-oriented deployment still needs decisions
outside this repo's compose defaults, including:

- durable Postgres and Vault operation;
- real identity-provider client registration and secret handling;
- TLS certificate lifecycle management;
- gateway/API ingress policy;
- Docker runtime scheduling and capacity limits;
- logging, metrics, backup, and incident-response policy.
