# Operator CLI And Local Diagnostics

This document defines the supported `bpane` operator surface and the local
troubleshooting path. The frozen HTTP contract remains
[`openapi/bpane-control-v1.yaml`](../openapi/bpane-control-v1.yaml), and its
audience assignments remain
[`openapi/bpane-control-v1.classifications.json`](../openapi/bpane-control-v1.classifications.json).
The CLI is intentionally narrower than the API: it exposes repeatable owner
operations, not every UI, compatibility, or worker route.

## CLI Contract

- Use `./scripts/bpane` from the repository root. The installable package binary
  is also named `bpane`.
- Command flags override environment variables, which override the selected
  profile and then local defaults.
- Keep bearer tokens in `BPANE_ACCESS_TOKEN` or a process-local `--access-token`.
  `profile init` persists a token only with `--save-token`; profile files use
  mode `0600`.
- Success and failure output is structured JSON. Usage, authentication, API,
  and unexpected failures have stable non-zero exit codes.
- Binary input and output use explicit paths. Downloads never print artifact
  bytes to stdout.
- Credential payloads, signing secrets, proxy credentials, custom CA material,
  and worker-resolved values are not operator output.

## API-Family Support Matrix

The matrix is based on all `131` operations in
`openapi/bpane-control-v1.operations.json` as of 2026-08-10.

| OpenAPI family | CLI status | Canonical surface and boundary |
| --- | --- | --- |
| Admin Events | Admin/API-only | Event-stream access tokens and the event stream belong to Admin-New's refresh-safe realtime client, not a short-lived CLI command. |
| Automation Tasks | Compatibility-only | The legacy task API remains available to compatibility clients. New operator automation uses `bpane workflow`; task state/log mutation remains internal. |
| Browser Contexts | Supported | `browser-context` covers create, list, get, clone, import, export, and delete. |
| Credential Bindings | Supported metadata | `credential-binding` covers create, list, and get. Resolved credentials remain worker-only. |
| Egress Profiles | Supported | `egress-profile` covers create, list, get, update/disable, diagnostics, and active probe. |
| Extensions | Supported | `extension` covers definition create/list/get, version publication, enable, and disable. Definitions are retained because the API has no delete route. |
| File Workspaces | Supported transfers | `file-workspace` covers workspace create/list/get and file list/upload/download/delete. Individual file metadata is available in list results; there is no workspace delete route. |
| Identity | Supported | `identity`, `service-principal`, and `identity-mapping` cover current identity, access review, lifecycle, and disable transitions. |
| Projects | Supported | `project` covers create/list/get/update, usage, and archive. Archive is the supported retirement transition. |
| Session Automation | Supported with integration exception | `mcp authorize/revoke` manage the owner automation delegate. `session automation-access` is retained for trusted integration clients; MCP-owner mutation remains bridge-internal. |
| Session Files | Read-only evidence | `session file` and `session file-binding` cover list/get/download. Binding create/remove changes session setup and remains Admin/API-only. |
| Session Recordings | Read-only evidence | `session recording` and `session playback` cover retained metadata, segment downloads, manifest, and zip export. Recording policy/start/stop is Admin/API-only; complete/fail is worker-internal. |
| Session Runtime | Supported with observer exception | `session status`, access tickets, and egress diagnostics/probe are supported. `session egress-usage report` is retained for trusted sanitized observer ingestion, not general interactive use. |
| Session Templates | Supported | `session-template` covers create/list/get/update. |
| Sessions | Supported lifecycle | `session` covers create/list/get/status, cancel queued, disconnect all, release, stop, kill, and bounded cleanup. Single-connection disconnect and resource delete remain Admin/API-only. |
| Workflows | Supported operator surface | `workflow` covers definitions, immutable versions, source validation/preview/files, run lifecycle, logs/events, and produced-file downloads. Source snapshots, workspace-input content, credential resolution, worker state/log/file mutation, and operation counters are not owner CLI commands. |

The MCP `health`, `doctor`, `preflight`, `repair`, and default-session commands
also use the configured bridge control surface. That compatibility surface is
outside the frozen gateway OpenAPI contract.

## Diagnostic Order

Run checks in this order so later symptoms are not confused with an earlier
dependency failure.

| Symptom | First check |
| --- | --- |
| API calls fail or Compose looks started but unusable | Gateway health and dependency readiness |
| `Opening handshake failed` or a black connection popup | Served certificate metadata and Chromium QUIC/SPKI trust |
| MCP cannot attach or drives the wrong session | MCP health, doctor, then strict preflight |
| Workflow publication reports source/repository/setup errors | Workflow source validation and `/workspace` trusted-root access |
| Session start remains queued/fails | Host Docker daemon and gateway socket access |
| Camera is unavailable | Linux host, `v4l2loopback`, device mapping, and browser H.264 support |

### 1. Gateway And Compose

```bash
docker info >/dev/null
docker compose -f deploy/compose.yml config --quiet
docker compose -f deploy/compose.yml ps
curl -fsS http://localhost:8932/healthz
curl -fsS http://localhost:8932/readyz
```

`/healthz` proves only that the gateway process is alive. `/readyz` also checks
the configured session store, runtime manager, credential provider, and
recording/workspace artifact stores. Inspect the gateway log when readiness
fails:

```bash
docker compose -f deploy/compose.yml logs --tail=200 gateway
```

### 2. CLI Profile And Authentication

Keep the token process-local unless persistence is explicitly required:

```bash
export BPANE_ACCESS_TOKEN=<owner-bearer-token>
./scripts/bpane profile show
./scripts/bpane identity me
./scripts/bpane identity access-review
```

An `AUTH_REQUIRED` CLI result means no bearer was resolved. HTTP `401`/`403`
means a bearer was sent but the gateway rejected its authentication or owner
scope.

### 3. WebTransport Certificate Trust

Regenerate all local certificate derivatives together after rotation:

```bash
./deploy/gen-dev-cert.sh dev/certs
openssl x509 -in dev/certs/cert.pem -noout -dates
curl -fsS http://localhost:8080/cert-fingerprint
cat dev/certs/cert-fingerprint.txt
curl -fsS http://localhost:8080/cert-hash
cat dev/certs/cert-hash.txt
```

The served and checked-in derivative values must match. Restart/rebuild the web,
gateway, and recording-worker image consumers after certificate rotation. For a
manually launched Chromium, use the QUIC origin and SPKI flags documented in
the README. A healthy HTTP API does not prove WebTransport certificate trust.

### 4. MCP Delegation

```bash
./scripts/bpane mcp health
./scripts/bpane mcp doctor <session-id>
./scripts/bpane mcp preflight <session-id>
```

`doctor` is interactive-friendly; add `--fail-on-issues` to make findings
fatal. `preflight` is strict. Use `mcp repair <session-id>` only after verifying
that the session belongs to the current owner. The local bridge must use its
package-installed `@playwright/mcp`; inspect startup errors and unexpected
runtime downloads with:

```bash
docker compose -f deploy/compose.yml logs --tail=200 mcp-bridge \
  | rg 'npm warn exec|@playwright/mcp@latest|error|ERROR'
```

No `npm warn exec` or `@playwright/mcp@latest` match is expected during a normal
first connection.

### 5. Workflow Source

```bash
./scripts/bpane workflow validate-source <workflow-id>
./scripts/bpane workflow version files <workflow-id> <version>
docker compose -f deploy/compose.yml exec -T gateway \
  sh -lc 'test -r /workspace/.git/HEAD'
```

Local Compose mounts the repository read-only at `/workspace` and configures it
as the only trusted local workflow-source root. Source failures distinguish
validation, Git resolution/access, materialization, archive/snapshot, and
worker/source infrastructure categories. Do not add process-wide Git trust to
work around a rejected path.

### 6. Docker Runtime Access

Direct local compatibility:

```bash
docker info >/dev/null
docker compose -f deploy/compose.yml exec -T gateway \
  docker info --format '{{.ServerVersion}}'
node scripts/validate-docker-runtime-boundary.mjs
docker compose -f deploy/compose.yml logs --tail=200 gateway docker-proxy \
  | rg 'docker|runtime|permission denied|unavailable'
```

The local gateway uses `DOCKER_HOST` to reach the internal `docker-proxy` for
`docker_pool`, workflow workers, recording workers, and volume helpers. Only
the proxy mounts the host socket. An inactive host daemon, unhealthy proxy,
denied required API, or permission failure can leave process liveness available
while readiness and runtime admission fail. The validator checks the structured
Compose contract and live allowed/denied API behavior.

Production-like broker topology:

```bash
./scripts/start-runtime-broker-browser-overlay.sh
node scripts/validate-runtime-broker-browser-overlay.mjs
./scripts/smoke-runtime-broker-isolation.sh
./scripts/smoke-runtime-broker-storage.sh
cd code/web/bpane-client
npm run smoke:runtime-broker-restart -- --headless
```

In this topology, `docker info` inside the gateway is expected to fail: the
gateway has no `DOCKER_HOST`, socket, proxy dependency, or shared Docker-control
network. Diagnose container/volume operations through runtime-broker readiness,
safe broker audit metadata, and proxy/broker logs rather than granting the
gateway direct Docker access.

### 7. Optional Camera Ingress

Camera ingress is Linux-host-only and disabled in the default Compose file. On
the Docker host, verify an intentionally provisioned device before adding the
documented Compose device override:

```bash
uname -s
lsmod | rg '^v4l2loopback'
ls -l "${BPANE_CAMERA_DEVICE:-/dev/video0}"
docker compose -f deploy/compose.yml config | rg -n 'devices:|/dev/video'
```

No `devices` match is expected for the default stack. Enabling camera also
requires browser WebCodecs H.264 encode support. There is no MJPEG fallback; an
unsupported client must remain a clear disabled/unsupported state.

## Validation

```bash
cd code/web/bpane-client
npm test -- --run js/__tests__/bpane-cli.test.ts
npm run check
npm run build
npm run smoke:bpane-cli -- --headless
cd ../../..
node scripts/check-repository-documents.mjs
```

The Compose smoke covers profile permissions, identity, governed resources,
binary transfer, session lifecycle including release, MCP diagnostics/repair,
and cleanup. Run the broader workflow, recording, and session-file smokes when
those command families change.
