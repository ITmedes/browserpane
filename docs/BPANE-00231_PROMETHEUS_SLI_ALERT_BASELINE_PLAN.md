# BPANE-00231 Prometheus SLI And Alert Baseline Plan

## Metadata

- Issue: [#231](https://github.com/ITmedes/browserpane/issues/231)
- Parent: [#178](https://github.com/ITmedes/browserpane/issues/178)
- State: Qualified; ready for review
- Lane: Production
- Target gate: Production Baseline
- Depends on: #150, #178 metrics foundation, #227, #229
- Last verified commit/date: `4c0b3b30`, 2026-08-16

## Business Outcome

Operators receive a versioned, deployable starter pack that turns BrowserPane's
bounded OpenMetrics into understandable service indicators and actionable
alerts. Every alert links to a concrete diagnostic and recovery path. The pack
is an initial operating proposal, not a contractual SLO or an unsupported
availability claim.

## Example Use Case

A pilot workflow finishes browser work but its downstream callback target stops
accepting signed events. The gateway and browser runtimes remain healthy, so a
generic uptime alarm would miss the business failure. The recording rules show
the bounded event-delivery success ratio falling, the terminal-delivery alert
fires, and the runbook tells the operator how to distinguish callback failure
from gateway 5xx pressure, runtime saturation, artifact upload failure, and
collector outage without exposing workflow ids, target URLs, credentials,
payloads, or browser content.

## Implemented Evidence

- `GET /metrics` exports bounded HTTP RED histograms/counters, runtime-capacity
  gauges, and 25 label-free workflow/recording operation counters.
- Workflow/recording authenticated snapshots and OpenMetrics use the same
  counter instances.
- The single-node fixture includes a private OpenTelemetry collector for traces.
- `deploy/examples/observability/prometheus.yml` loads 15 recording rules and 10
  starter alerts from the checked-in backend-neutral rule pack.
- The official digest-pinned Prometheus 3.12.0 `promtool` validates the config,
  rule syntax, and deterministic healthy, failure, hold, recovery, no-traffic,
  counter-reset, and scrape-absence behavior.
- Static contracts enforce the expected rule inventory, bounded labels,
  required alert metadata, valid runbook anchors, and forbidden sensitive
  fields.
- The operator runbook covers safe triage, dependency checks, mitigation,
  escalation, and recovery verification for every starter alert.
- Final calibrated SLOs, dashboards, synthetic checks, capacity envelopes, and
  Alertmanager routing remain explicitly outside this slice and open under
  parent #178.

## Scope

1. Add Prometheus recording rules derived only from stable shipped metrics:
   - gateway request throughput, 5xx ratio, and p95 response latency,
   - aggregate runtime-capacity utilization,
   - workflow produced-file and event-delivery outcome ratios,
   - recording finalization outcome ratio,
   - bounded failure/retry increases for workflow, recording, playback, and
     retention operations.
2. Add conservative starter alerts for:
   - gateway scrape absence,
   - sustained gateway 5xx pressure,
   - sustained runtime-capacity saturation,
   - produced-file upload failures,
   - event-delivery retries and terminal failures,
   - recording finalization and playback export failures,
   - workflow or recording retention failures.
3. Give each alert stable severity/subsystem metadata, a concise summary, and a
   checked-in runbook reference.
4. Extend the example Prometheus configuration to load the rule files.
5. Validate config/rule syntax and deterministic alert behavior with a
   digest-pinned upstream `promtool` toolchain.
6. Add an operator runbook covering triage, safe queries, dependency checks,
   escalation, mitigation, and recovery verification.
7. Synchronize telemetry, roadmap, maturity, risk, validation, issue context,
   README, architecture, and contributor commands where behavior changes.

## Non-Goals

- Grafana dashboards, Alertmanager receivers/routing, paging integrations, or a
  proprietary observability backend.
- Final contractual SLO targets, error-budget policy, support commitments, or
  generic production-ready claims.
- New gateway metrics, traces, dynamic labels, tenant/resource drill-down,
  synthetic probes, or load/capacity envelopes.
- Per-owner, project, session, workflow, recording, URL, credential, payload,
  browser-content, raw-error, or artifact-reference telemetry.
- Publishing Prometheus or gateway metrics on an untrusted network.
- API, OpenAPI, database, browser protocol, admin-new, CLI, or SDK changes.
- Closing parent #178.

## Design Decisions

### Standards and tooling

- Use Prometheus rule and unit-test formats. Do not implement a custom PromQL or
  YAML validator.
- Execute the upstream `promtool` from a digest-pinned Prometheus image so local
  and CI validation use the same parser and evaluator.
- Keep the example optional and backend-neutral; operators may load equivalent
  rules into a compatible managed Prometheus service.

### Indicator semantics

- Ratios use `rate`/`increase` over fixed windows and do not infer success from
  absent traffic. Alerts add minimum-volume gates where a ratio alone would be
  noisy.
- Counter resets are normal because gateway counters are process-local;
  expressions must remain valid across resets.
- Runtime utilization is aggregate `(active + starting) / limit`; it is not a
  CPU, memory, concurrency, or queue-depth claim.
- HTTP indicators retain only the existing bounded method, route-template, and
  status-class dimensions. Subsystem indicators remain label-free.
- Proposed alert thresholds and hold times are conservative examples that must
  be calibrated against a named deployment and workload before contractual use.

### Security and exposure

- Rule labels/annotations contain static operational metadata only.
- Queries and runbooks must not ask operators to collect resource ids, URLs,
  credentials, payloads, browser contents, raw CA material, or decrypted
  traffic into metrics.
- The gateway metrics endpoint and Prometheus remain on a trusted operator
  network; this slice adds no public listener.

## Planned Artifacts

- `deploy/examples/observability/prometheus.yml`
- `deploy/examples/observability/recording-rules.yml`
- `deploy/examples/observability/alert-rules.yml`
- `deploy/examples/observability/rule-tests.yml`
- `deploy/examples/observability/README.md`
- `docs/operations/PROMETHEUS_ALERT_RUNBOOK.md`
- a focused validation script/contract test under `scripts/observability/`
- validation workflow/stage integration using the pinned toolchain

File names may be adjusted to established repository conventions during
implementation, but ownership must remain within the observability example,
operator documentation, and validation boundaries.

## Implementation Slices

### Slice 1: Rule contract and pinned validation

1. Select and document a supported digest-pinned Prometheus toolchain.
2. Add recording rules and starter alert rules using only shipped series.
3. Extend the scrape example to load the rules.
4. Add a narrow command that runs `promtool check config`, `check rules`, and
   `test rules` without writing generated output into the repository.

Commit: `feat(observability): add Prometheus SLI and alert rules`.

### Slice 2: Deterministic behavior and runbooks

1. Add rule fixtures for healthy, firing, hold, recovery, no-traffic,
   counter-reset, and target-absent behavior.
2. Add static contracts for bounded labels, required metadata, stable runbook
   references, and forbidden sensitive/dynamic fields.
3. Add the alert-to-diagnostic operator runbook and recovery checks.

Commit: `test(observability): qualify alert behavior and runbooks`.

### Slice 3: Live qualification and synchronization

1. Integrate the rule validator with required repository validation.
2. Exercise the example against the rebuilt single-node fixture and real
   workflow/recording operations.
3. Verify scrape outage and recovery behavior without exposing Prometheus.
4. Synchronize operator/product/governance documents and record evidence.

Commit: `docs(observability): qualify the Prometheus operations baseline`.

## Test Strategy

### Unit and static contracts

- expected rule groups and stable record/alert names are present,
- every alert has severity, subsystem, summary, description, and runbook data,
- expressions use only allowed shipped metric/recording-rule names,
- no dynamic or sensitive labels/annotations are present,
- the example config loads every checked-in rule file,
- runbook references resolve to existing headings.

### Prometheus semantic tests

- `promtool check config` accepts the scrape and rule configuration,
- `promtool check rules` accepts every rule file,
- `promtool test rules` proves healthy/non-firing and expected firing behavior,
- ratio alerts are volume-gated and do not fire on no traffic,
- `for` durations hold transient conditions below the alert threshold,
- counter resets do not create false terminal-failure alerts,
- target absence/down behavior fires and resolves as documented.

### Live smoke and E2E

- start the single-node fixture and load the example rules,
- execute a workflow with produced-file and callback success,
- finalize one recording and generate a playback export,
- verify recording rules evaluate from real scraped series,
- interrupt the scrape target, observe the availability alert, restore it, and
  verify recovery,
- scan metrics/rules/annotations/evidence for forbidden values,
- rerun the existing single-node and tracing qualifications.

### Regression and quality

- repository document and production-security contracts,
- canonical fast validation profile,
- impacted Compose API metrics lane,
- `git diff --check`, YAML validation, and README/ARCH review.

## Manual Test Sequence

1. Start the single-node fixture and wait for gateway readiness.
2. Start the observability example on the private fixture network.
3. Query Prometheus through an operator-only path, such as `docker exec`, or an
   explicitly loopback-bound test port; do not expose it publicly.
4. Confirm all BrowserPane recording rules are healthy and initial ratios with
   no traffic do not trigger failure alerts.
5. Run the qualification workflow and recording/playback smoke.
6. Query gateway 5xx, runtime utilization, event delivery, artifact upload, and
   recording finalization indicators and confirm values reflect real actions.
7. Stop the gateway or block its scrape path long enough for the configured
   hold and verify the scrape-absence alert plus runbook metadata.
8. Restore the gateway and verify the alert resolves after successful scrapes.
9. Review alert labels and annotations for resource ids, URLs, credentials,
   payloads, browser content, raw errors, and artifact refs; none may appear.

## Definition Of Done

- Issue #231 acceptance criteria pass.
- Prometheus loads and evaluates the checked-in rules with the pinned toolchain.
- Deterministic tests cover healthy, failure, recovery, no-traffic, reset, and
  scrape-absence behavior.
- Every starter alert has a tested, actionable runbook path.
- Existing metrics, workflow, recording, single-node, and tracing behavior stay
  green.
- Documentation labels the indicators and thresholds as initial proposals and
  does not claim a final SLO or unsupported production readiness.
- Parent #178 remains open for broader signals/traces, dashboards, synthetics,
  final calibrated SLOs, and reproducible capacity envelopes.

## Qualification Evidence

Verified on 2026-08-16:

- focused observability contracts: 3 tests passed,
- pinned `promtool`: config valid, 15 recording rules, 10 alerts, and all rule
  unit tests passed,
- targeted validation: validation tooling (104 tests), repository documents
  (84 Markdown, 16 YAML, 3 workflows), and the Prometheus rule stage passed,
- canonical fast validation: all 44 stages passed,
- Rust coverage ratchet: 60.92% line coverage and 64.88% region coverage,
- `admin-new`: 196 test files and 620 tests passed; 92.28% statement, 90.43%
  line, 93.86% function, and 76.74% branch coverage,
- focused Compose gateway health/readiness test: 1 passed, including the
  Postgres outage and recovery path,
- single-node qualification: two isolated workflows, retained produced-file
  evidence across a control-plane restart, gateway Docker denial, secret
  protection, a 291,545-byte recording, and a 297,423-byte playback export,
- live Prometheus smoke on the private fixture network: both rule groups were
  healthy, no healthy-state alerts fired, produced-file failures evaluated to
  zero, recording finalization success evaluated to one, and the gateway
  scrape-absence alert progressed through pending to firing before resolving
  after gateway recovery,
- runtime tracing smoke: caller context propagation, collector outage
  tolerance, recovery, malformed-context rejection, and redaction checks all
  passed.

The live Prometheus container was attached only to the private fixture network,
had no host port, and was removed after qualification. The rebuilt single-node
fixture remains available locally for manual inspection.

## Post-Implementation Smoke Sequence

1. Run focused observability contract tests.
2. Run pinned `promtool` config, rule, and unit-test validation.
3. Run repository docs/security/YAML contracts and the canonical fast profile.
4. Rebuild and start the single-node fixture.
5. Execute real workflow produced-file/callback and recording/playback paths.
6. Verify live recording-rule outputs and non-firing healthy alerts.
7. Interrupt the scrape target, verify the held absence alert, then restore and
   verify resolution.
8. Scan all exposed telemetry and alert metadata for forbidden values.
9. Run the impacted Compose metrics suite plus single-node and tracing smoke.
