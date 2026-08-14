# BrowserPane Capability Maturity Matrix

Governance issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Last verified: 2026-08-14 on
`feature/BPANE-00178-platform-metrics-foundation`

## Maturity Definitions

| Level | Meaning |
| --- | --- |
| Implemented | A working code path exists with focused tests. No broader readiness claim is implied. |
| Prototype | End-to-end behavior is demonstrable in the supported local profile, but one or more security, compatibility, operability, or scale gates remain. |
| Pilot-ready | The capability is bounded by an agreed use case, deployment, runbook, failure paths, and acceptance evidence. |
| Production-ready | Security, recovery, observability, compatibility, release, and supported-scale gates are satisfied. |
| Planned | A canonical issue/specification exists but the target contract is not implemented. |
| Hypothesis | Product or commercial value remains to be validated; it must not be presented as delivered capability. |

A higher level requires evidence from the corresponding release gate. UI
availability, documentation, or one happy-path smoke alone cannot promote a
capability.

## Current Matrix

| Capability | Current maturity | Evidence | Primary remaining owner |
| --- | --- | --- | --- |
| Linux/Chromium browser runtime | Prototype | `bpane-host`, typed broker contract/policy, gateway-isolated Docker-host topology, restart/storage/compose/session smokes | #66 deployment packaging, #168 |
| Tile-first remote rendering and ROI H.264 | Prototype | `bpane-protocol`, host capture, TypeScript compositors, media tests | #175, #168, #169 |
| BrowserPane remote protocol | Prototype | Rust frame types and TypeScript client interoperate | #175 specification/conformance |
| Shared live sessions and reconnect | Prototype | gateway session hub, client session tests, multisession/reconnect smokes | #169, #178 |
| Owner-scoped control API | Prototype | 131 governed OpenAPI operations, generated classification inventory, 19 executable examples, Axum route recognition, semantic diff, and contract-derived admin-new API/coverage/docs companions | #75 release compatibility; domain and production gates |
| Projects and policy bindings | Prototype | API/store resources and admin-new project views | #161, #176, #79 |
| Session templates | Implemented API; incomplete operator product | API/store and legacy coverage | #124 admin-new catalog |
| Reusable browser contexts | Prototype | lifecycle, limits, and clone/export/import paths covered through API, CLI, compatibility admin, and admin-new smokes | Production storage/provider gates |
| File workspaces and session-file bindings | Prototype | local artifact provider, API/admin/CLI smokes | #21, #80 |
| Egress profiles and sanitized usage | Prototype | proxy/TLS fixtures, diagnostics, policy bindings | #72, #76, #178 |
| Credential bindings | Prototype | Vault KV v2 provider and workflow/egress integration | #159, #70, #76 |
| Browser extensions | Prototype | approved extension metadata and docker workflow/session support | #159, #72 |
| MCP session delegation | Prototype | explicit session endpoint, bridge proxy, smoke coverage | #69, #70, #176 |
| Workflow publishing and execution | Prototype | pinned git sources, worker lifecycle, runs/events/logs/files, finite worker request deadlines, bounded process output, package tests, and compose admission/workspace evidence | #47 and production integration gates |
| Stable BPM Workflow Endpoint | Planned | Detailed contract and slices in #172 | #172 |
| Workflow Teach Mode | Planned | Demonstration-to-candidate specification in #171 | #171 |
| Workflow Human Handoff | Partial prototype | run input/hold/resume primitives exist | #71, #154 |
| Recording lifecycle and playback | Prototype | recorder worker, worker-only exact-path finalization merged through PR #212, measured-byte accounting, finite worker requests, single-flight finalize polling, segmented artifacts, admin/CLI downloads, and off-thread playback export | #21 artifact/provider lifecycle |
| Generalized artifact/evidence model | Partial prototype | recordings, workspace files, workflow produced files remain separate resources | #21, #70, #28 |
| Workflow event delivery | Prototype | signed delivery, retry/backoff, diagnostics, DNS/IP/redirect policy, pinned delivery, finite worker requests, compose E2E | #28 retained event/audit lifecycle |
| OIDC login and current-principal identity | Prototype | Keycloak compose, JWT validation, identity/access-review APIs | #146, #157 |
| Service-principal registry | Implemented metadata; grants not fully enforced | registry CRUD and disabled-delegation guard | #176, #70 |
| Organization/project RBAC | Planned | Current mappings are descriptive and owner-scoped | #176 |
| Provisioning/deprovisioning and break-glass | Planned | Explicitly outside current identity baseline | #177 |
| Admin-new resource console | Prototype | dashboard and major resource catalogs/details | #153-#163, #124 |
| Admin-new default promotion | Implemented | Root route selects `/admin-new/`; promotion contract and smoke evidence merged through PRs #210 and #211 | Separate compatibility-admin removal decision |
| Gateway health/readiness/drain | Implemented | Public liveness/readiness probes, configured dependency checks, SIGINT/SIGTERM readiness withdrawal, bounded HTTP/WebTransport drain, unit and compose failure-path evidence on #150 branch | #66, #74, #178 production packaging/HA/telemetry |
| Platform telemetry and SLOs | Partial prototype | Gateway OpenMetrics HTTP RED and aggregate runtime-capacity metrics with bounded-label/redaction unit and Compose coverage | #178 tracing, subsystem metrics, SLOs, alerts, synthetics, and load envelopes |
| Compose deployment | Prototype | Direct local compatibility plus gateway-isolated production-like broker topology, dependency readiness, static/live boundary checks, and restart parity | #66, #166, #178 |
| Kubernetes/Fargate/cloud adapters | Planned | Architecture options, no production support claim | #66 deployment adapters; #214 shared typed launch contract |
| HA and disaster recovery | Planned | No supported HA/DR contract | #73, #74 |
| Supply-chain/release governance | Planned | Local tests and Dependabot exist; no enforced CI/SBOM/signing | #151, #75 |
| Open-source governance | Gap with explicit issue | AGPL root conflicts with Cargo MIT metadata; contributor policy absent | #180 |
| Phase 0 reference workflow | Planned | Management proposition exists; delivery owner newly defined | #174 |

## Evidence Rules

For each promotion update this table with:

- merged issue/PR,
- code or contract path,
- automated test and coverage evidence,
- compose/E2E evidence,
- security and negative-path evidence,
- operator runbook or recovery evidence,
- supported deployment and capacity evidence,
- README/ARCH/OpenAPI and management-claim impact.

Claims must use the exact maturity language. In particular:

- Live View, recording, workflow execution, and MCP are current Prototype
  evidence, not a Production SLA.
- Stable external BPM invocation is Planned until #172 P0 is implemented.
- Teach Mode, AI authoring, and controlled repair are Planned under #171.
- Autonomous self-healing and autonomous high-impact decisions remain
  Hypotheses/non-goals rather than current promises.
- The custom protocol is implemented, but broad compatibility or standard
  claims require #175.
