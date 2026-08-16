# BPANE-00233 Grafana Operations Dashboard Plan

## Metadata

- Issue: [#233](https://github.com/ITmedes/browserpane/issues/233)
- Parent: [#178](https://github.com/ITmedes/browserpane/issues/178)
- State: Qualified; Ready for Review
- Lane: Production
- Target gate: Production Baseline
- Depends on: #150, #178 metrics foundation, #227, #229, #231
- Last verified commit/date: `0aa2d04a`, 2026-08-16

## Business Outcome

Operators receive a versioned, automatically provisioned dashboard that turns
BrowserPane's existing aggregate indicators into a first operational view. The
dashboard answers whether the gateway is reachable, serving errors or slow
requests, constrained by runtime assignments, or observing workflow,
recording, playback, or retention failures. It remains a starter operations
view rather than a contractual SLO, support, HA, or capacity claim.

## Example Use Case

A pilot workflow finishes its browser interaction but downstream processing is
late. The operator opens one dashboard and sees a healthy gateway, normal p95
latency, spare runtime capacity, zero produced-file failures, but increasing
event-delivery retries. That points to callback delivery rather than browser,
gateway, or storage pressure. The operator follows the checked-in alert runbook
without querying raw owner, project, session, workflow, URL, credential,
payload, or artifact data.

## Current Evidence

- `GET /metrics` exports bounded gateway HTTP RED, runtime-capacity, workflow,
  event-delivery, recording, playback, and retention metrics.
- The observability example ships 15 Prometheus recording rules, 10 starter
  alerts, deterministic rule tests, and alert-specific runbooks.
- A source-controlled Grafana OSS 13.0.2 stack provisions a private Prometheus
  datasource and the 20-panel `BrowserPane Operations` dashboard without
  checked-in credentials or public host ports.
- Static contracts protect the datasource, provisioning, stable panel/query
  inventory, layout, neutral no-activity semantics, and sensitive-data bounds.
- The live validator checks Grafana health, datasource health, dashboard
  provisioning, all 19 query panels, error-level logs, private exposure, and
  cleanup against digest-pinned Prometheus and Grafana images.

## Scope

1. Add a provisioned private Prometheus datasource with a stable UID and no
   embedded credentials.
2. Add a provisioned BrowserPane operations dashboard covering:
   - gateway scrape health, aggregate request rate, 5xx ratio, and p95 latency,
   - runtime active/starting/limit values and aggregate utilization,
   - workflow produced-file and event-delivery outcomes,
   - recording finalization, worker, and playback export outcomes,
   - workflow and recording retention failures.
3. Give panels stable numeric ids, descriptions, units, legends, threshold
   semantics, grid positions, time range, refresh interval, and no-data
   behavior.
4. Add a private-network local Compose example using immutable Prometheus and
   Grafana OSS images.
5. Add deterministic JSON/provisioning/query/security contracts and live
   Grafana provisioning/query smoke.
6. Document interpretation, private exposure, authentication/TLS/storage
   responsibilities, version compatibility, and limitations.
7. Synchronize contributor commands, architecture, README, telemetry, roadmap,
   maturity, risk, validation, and issue context.

## Non-Goals

- Final calibrated SLOs, error budgets, paging policy, contractual thresholds,
  support commitments, or generic production-ready claims.
- Alertmanager receivers/routing, durable Prometheus/Grafana storage, public
  exposure, or a proprietary observability backend.
- New gateway metrics/traces, log ingestion, synthetic checks, load tests, or
  reproducible capacity envelopes.
- Per-owner, project, session, workflow, recording, URL, credential, payload,
  browser-content, raw-error, artifact-reference, or egress-content telemetry.
- Dashboard variables that discover or select dynamic BrowserPane resources.
- Admin-new dashboard embedding, OpenAPI, database, browser protocol, CLI, or
  SDK changes.
- Closing parent #178.

## Design Decisions

### Standards and compatibility

- Use Grafana's documented file provisioning for datasources and dashboards;
  do not create a custom dashboard renderer or provisioning API.
- Use the classic Grafana dashboard JSON model because it remains broadly
  importable across Grafana OSS, Enterprise, Cloud, and managed services.
- Qualify with Grafana OSS 13.0.2 pinned to the multi-architecture digest
  `sha256:5dad0df181cb644a14e13617b913b261a54f7d4fd4510721dba420929f35bea2`.
- Keep provisioning source-controlled and `allowUiUpdates: false`; repository
  changes remain authoritative.

### Query and panel semantics

- Prefer the 15 approved recording rules. Use only bounded aggregate source
  gauges for scrape health and active/starting/limit values where no recording
  rule exists.
- Do not add dashboard variables. The baseline is one aggregate deployment
  view and cannot select owners or resources.
- Ratios remain undefined when there is no qualifying activity. Panel no-data
  text and descriptions must distinguish absence of traffic from success.
- Runtime utilization is assignment usage, not CPU, memory, queue depth, or
  tested concurrency.
- Starter thresholds are visual orientation only and mirror the checked-in
  alert proposals where applicable. They require workload calibration.

### Security and operation

- The datasource URL is an internal service address and contains no userinfo,
  query, fragment, or credential.
- Prometheus and Grafana stay on a trusted operator network. The example does
  not publish either service to the host or public network.
- The example must not ship a default administrator password. Live smoke uses
  an ephemeral generated credential and queries through `docker exec`.
- Grafana authentication, TLS termination, RBAC, durable storage, retention,
  backup, and availability remain operator-owned deployment concerns.

## Planned Artifacts

- `deploy/examples/observability/compose.grafana.yml`
- `deploy/examples/observability/grafana/provisioning/datasources/prometheus.yml`
- `deploy/examples/observability/grafana/provisioning/dashboards/browserpane.yml`
- `deploy/examples/observability/grafana/dashboards/browserpane-operations.json`
- `scripts/observability/grafana-dashboard-contract.mjs`
- `scripts/observability/grafana-dashboard-contract.test.mjs`
- `scripts/observability/validate-grafana-dashboard.mjs`
- updates to the observability example and operator/product/governance docs

File names may be adjusted to established repository conventions while the
ownership boundaries remain unchanged.

## Implementation Slices

### Slice 1: Provisioned dashboard contract

1. Add the immutable Grafana toolchain and datasource/dashboard providers.
2. Add the stable operations dashboard using only approved aggregate queries.
3. Add a deterministic structural contract for provisioning, panels, queries,
   units, descriptions, thresholds, links, layout, and sensitive fields.

Commit: `feat(observability): add Grafana operations dashboard`.

### Slice 2: Live provisioning and query qualification

1. Add a private-network Compose example without host port publication.
2. Add a bounded validator that launches Grafana with ephemeral credentials,
   verifies provisioning through its API, and removes all temporary resources.
3. Qualify dashboard queries against live Prometheus data from real
   workflow/recording operations and prove scrape loss/recovery visibility.

Commit: `test(observability): qualify Grafana dashboard provisioning`.

### Slice 3: Required validation and synchronization

1. Add static dashboard contracts to the fast validation floor and immutable
   live provisioning validation to the named Compose-class floor.
2. Synchronize README, architecture, telemetry, roadmap, issue context,
   maturity, risk, validation, and contributor commands.
3. Run focused, fast, single-node, tracing, and impacted Compose evidence.

Commit: `docs(observability): qualify the Grafana operations baseline`.

All three implementation slices are complete. Visual qualification found and
closed clipped guidance, narrow operator labels, and misleading colored
no-activity states before final evidence was recorded.

## Test Strategy

### Unit and static contracts

- provisioning YAML parses and points to the checked-in dashboard directory,
- datasource UID/type/access/URL/default/editability are exact and safe,
- dashboard UID/title/tags/schema/time/refresh/editability are stable,
- panel ids are unique, contiguous by contract, and have bounded grid geometry,
- expected operational domains, titles, descriptions, units, and targets exist,
- PromQL references only approved recording rules or bounded aggregate gauges,
- ratios, durations, rates, counts, and utilization use correct units,
- no variables, dynamic identifiers, hidden drill-downs, or forbidden sensitive
  fragments exist in queries, legends, links, annotations, or descriptions,
- repository/runbook links are static and valid.

### Live Grafana validation

- the pinned Grafana image starts with read-only provisioning inputs,
- health becomes ready within a bounded deadline,
- the private Prometheus datasource provisions with the expected UID,
- the dashboard provisions with the expected UID and panel inventory,
- Grafana's datasource proxy can query representative dashboard expressions,
- no host/public port is published and no default password is checked in,
- cleanup runs on success, failure, timeout, and interruption.

### Live smoke and E2E

- run the single-node workflow produced-file/callback path,
- finalize a recording and generate a playback export,
- verify representative panel queries against real rule outputs,
- interrupt and restore the gateway scrape target and observe dashboard source
  health loss and recovery,
- scan Grafana API responses and dashboard metadata for forbidden values,
- rerun single-node and runtime-tracing qualifications.

### Regression and quality

- repository documents and production-security contracts,
- canonical fast validation profile,
- impacted Compose gateway health/readiness metrics lane,
- `git diff --check`, JSON/YAML parsing, README/ARCH review.

## Manual Test Sequence

1. Start the single-node fixture and wait for gateway readiness.
2. Start the observability Prometheus and Grafana example on the private fixture
   network with an operator-supplied Grafana administrator password.
3. Reach Grafana only through an operator-controlled tunnel or explicitly
   loopback-bound temporary port.
4. Open `BrowserPane Operations` and confirm every panel loads without a
   datasource or provisioning error.
5. Run a workflow with produced-file/callback activity and a recording with a
   playback export.
6. Confirm gateway, runtime, workflow, recording, playback, and retention panels
   reflect real values or documented no-activity states.
7. Stop the gateway long enough to lose scrapes, confirm source-health loss,
   restore it, and confirm recovery.
8. Inspect dashboard variables, links, legends, queries, and API output; no
   resource ids, URLs, credentials, payloads, browser content, raw errors, or
   artifact references may appear.

## Definition Of Done

- Issue #233 acceptance criteria pass.
- The pinned Grafana image provisions the datasource and dashboard without
  manual import or checked-in credentials.
- Static contracts cover all panels, queries, semantics, and sensitive-data
  restrictions.
- Live smoke proves provisioning, representative real queries, scrape outage,
  and recovery on a private network.
- Existing metrics, alerts, runbooks, workflow, recording, single-node, and
  tracing behavior remain green.
- Documentation clearly labels the dashboard as an initial aggregate baseline.
- Parent #178 remains open for broader telemetry, final SLOs/error budgets,
  Alertmanager routing, synthetics, and capacity/load evidence.

## Qualification Evidence

- Focused Grafana unit/static/live validation: 9 tests passed; 20 panels and
  all 19 live queries passed against the pinned toolchain.
- Full fast validation: all 44 stages passed, including 113 validation-tool
  tests, Rust workspace tests and coverage, 620 admin-new tests at 92.28%
  statement coverage, 674 browser-client tests at 92.88%, worker packages,
  OpenAPI governance, dependency policy, and documentation/security checks.
- Real visual inspection: the dashboard rendered all 20 panels against the
  single-node gateway at 1600x1200 and collapsed to a single-column 768x1024
  operator view without horizontal overflow or browser-console errors.
- Real scrape interruption: `up{job="browserpane-gateway"}` transitioned from
  `1` to `0` after stopping the gateway and returned to `1` after recovery.
- Focused Compose readiness: one health/readiness case passed, including the
  expected Postgres dependency interruption and recovery.
- Single-node qualification passed with retained state across restart,
  distinct runtime containers, Docker boundary denial, protected worker
  secrets, a 39-byte produced file, a 281149-byte recording, and a
  287027-byte playback export.
- Runtime tracing smoke passed across collector outage/recovery with caller
  context continuation, malformed-context tolerance, gateway/broker spans,
  and no sensitive marker exposure.
- Temporary observability containers, networks, credentials, and visual-test
  exposure were removed after validation.

## Post-Implementation Smoke Sequence

1. Run focused Grafana dashboard contract tests.
2. Run pinned live Grafana provisioning validation.
3. Run Prometheus config/rule/unit validation unchanged.
4. Run repository docs/security/JSON/YAML contracts and canonical fast profile.
5. Rebuild and start the single-node fixture.
6. Execute workflow produced-file/callback and recording/playback paths.
7. Verify representative dashboard queries and no-activity behavior.
8. Interrupt the scrape target, verify dashboard source loss, restore it, and
   verify recovery.
9. Scan dashboard metadata/API evidence for forbidden values.
10. Run impacted Compose metrics and runtime-tracing smoke.
