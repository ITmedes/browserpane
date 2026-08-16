# BrowserPane Prometheus Alert Runbook

## Purpose And Status

This runbook covers the starter alerts in
`deploy/examples/observability/alert-rules.yml`. The rules and thresholds are
initial operating proposals. They are not contractual SLOs, paging policy, or a
tested capacity claim until calibrated against a named deployment and workload.

Keep Prometheus, gateway metrics, traces, and operator APIs on trusted networks.
Do not copy owner, project, session, workflow, recording, target URL,
credential, payload, browser-content, raw CA, decrypted-traffic, or artifact
reference values into metric labels or alert annotations.

## Common First Response

1. Record the alert name, start time, deployment profile, and reviewed release.
2. Confirm Prometheus itself is healthy and its clock is synchronized.
3. Check aggregate gateway health and readiness from the operator network:

   ```sh
   curl --fail-with-body http://gateway:8932/healthz
   curl --fail-with-body http://gateway:8932/readyz
   ```

4. Check the safe aggregate overview:

   ```promql
   up{job="browserpane-gateway"}
   browserpane:gateway_http_requests:rate5m
   browserpane:gateway_http_5xx:ratio_rate5m
   browserpane:runtime_capacity_utilization:ratio
   ```

5. Correlate with sanitized deployment logs and traces under existing access
   controls. Do not paste raw resource identifiers or secrets into alert labels,
   public tickets, or shared dashboards.
6. Apply only reversible mitigation within the tested deployment envelope.
   Record an escalation owner before changing runtime limits, retention, or
   dependency configuration.

## BrowserPaneGatewayMetricsUnavailable

**Signal:** The configured gateway target is absent or failing scrapes for two
minutes.

**Triage:** Confirm Prometheus target discovery, DNS, operator-network routing,
gateway process health, `/metrics`, `/healthz`, and `/readyz`. A healthy gateway
with a failed scrape usually indicates collector routing or configuration; an
unhealthy gateway is a service incident.

**Mitigation:** Restore the collector-to-gateway path or recover the gateway
using the deployment runbook. Do not expose `/metrics` publicly as a shortcut.

**Recovery:** `up{job="browserpane-gateway"}` remains `1`, fresh samples appear,
and the alert resolves after successful evaluations. Confirm other alerts were
not hidden during the scrape gap.

## BrowserPaneGatewayHighServerErrorRatio

**Signal:** More than five percent of sustained gateway requests are 5xx
responses for ten minutes.

**Triage:** Compare readiness, request rate, and the bounded route-template
breakdown:

```promql
sum by (route, status_class) (
  rate(browserpane_gateway_http_requests_total[5m])
)
```

Check Postgres, the runtime broker, artifact storage, and configured dependency
readiness before attributing the failure to the HTTP layer.

**Mitigation:** Reduce new workload if a dependency or capacity limit is under
pressure. Recover the failing dependency or roll back the reviewed release; do
not suppress 5xx responses or relax readiness to clear the alert.

**Recovery:** The 5xx ratio remains below the proposed threshold under normal
traffic and affected operations succeed through their existing smoke path.

## BrowserPaneRuntimeCapacitySaturated

**Signal:** Active plus starting runtime assignments equal the configured limit
for ten minutes.

**Triage:** Compare all three aggregate gauges:

```promql
browserpane_gateway_runtime_active_assignments
browserpane_gateway_runtime_starting_assignments
browserpane_gateway_runtime_assignment_limit
```

Determine whether assignments are completing normally, startup is stalled, or
admission demand exceeds the tested envelope. This ratio does not measure CPU,
memory, queue depth, or host capacity.

**Mitigation:** Pause or rate-limit new work and resolve stalled starts. Increase
the assignment limit only after host resources and the named capacity profile
support it.

**Recovery:** Utilization stays below one, starting assignments drain, and a
representative new session or workflow starts within its accepted window.

## BrowserPaneWorkflowProducedFileUploadFailure

**Signal:** At least one workflow produced-file upload failed in the rolling
fifteen-minute window.

**Triage:** Check gateway readiness, workflow-worker health, allowed workspace
bindings, artifact-store availability, storage capacity, and the authenticated
aggregate workflow operations snapshot. Keep file names and artifact references
out of telemetry and shared incident text.

**Mitigation:** Restore the workspace/artifact dependency or correct the
reviewed binding policy. Retry only when workflow side effects and idempotency
are understood.

**Recovery:** A controlled workflow uploads its expected produced file, the
failure increase stops, and retained artifacts remain downloadable through the
authorized API.

## BrowserPaneWorkflowEventDeliveryRetrying

**Signal:** Signed callback delivery scheduled at least one retry in the rolling
fifteen-minute window.

**Triage:** Compare attempts, successes, retries, and terminal failures. Verify
the callback receiver's availability, TLS/trust path, credential binding, and
SSRF policy without placing its URL or secret into metrics or alert text.

```promql
increase(browserpane_gateway_workflow_event_delivery_attempts_total[15m])
increase(browserpane_gateway_workflow_event_delivery_successes_total[15m])
increase(browserpane_gateway_workflow_event_delivery_retries_total[15m])
increase(browserpane_gateway_workflow_event_delivery_failures_total[15m])
```

**Mitigation:** Restore the receiver or trusted network path and let the bounded
retry policy proceed. Do not bypass signing, SSRF controls, or retry limits.

**Recovery:** Delivery successes advance, retries stop increasing, and no
terminal-failure alert follows.

## BrowserPaneWorkflowEventDeliveryFailure

**Signal:** At least one callback delivery exhausted its bounded retry policy in
the rolling fifteen-minute window.

**Triage:** Follow the retrying alert checks, then confirm whether the terminal
event can be replayed safely and whether the downstream process already applied
side effects.

**Mitigation:** Recover the receiver first. Use only an authorized, auditable
replay path with explicit idempotency; never edit persisted delivery state or
disable signature validation to force success.

**Recovery:** A controlled event reaches the receiver, attempts and successes
advance together, and terminal failures stop increasing.

## BrowserPaneRecordingArtifactFinalizeFailure

**Signal:** At least one recording artifact failed finalization in the rolling
fifteen-minute window.

**Triage:** Check recording-worker completion, staging-volume access, artifact
store health/capacity, gateway readiness, and the aggregate recording operations
snapshot. Distinguish a worker failure from an artifact handoff failure.

**Mitigation:** Restore the staging or artifact boundary. Preserve failed segment
metadata and temporary evidence according to retention policy; do not mark a
missing artifact ready manually.

**Recovery:** A controlled recording finalizes with a non-empty downloadable
artifact and finalization successes advance without another failure.

## BrowserPaneRecordingWorkerFailure

**Signal:** At least one recording segment entered a failed state in the rolling
fifteen-minute window.

**Triage:** Check recorder-worker startup, session access, browser rendering,
worker termination reason, and staging handoff. A ready browser session does not
by itself prove recorder readiness.

**Mitigation:** Recover the worker launch or browser attachment path. Start a new
linked segment where supported; do not represent discontinuous media as one
continuous recording.

**Recovery:** A new controlled segment reaches ready state, contains browser
content, and is included in the session playback manifest.

## BrowserPaneRecordingPlaybackExportFailure

**Signal:** At least one session playback export failed in the rolling
fifteen-minute window.

**Triage:** Verify retained segment metadata, artifact availability, gateway
storage access, and export capacity. Confirm expired artifacts are not being
treated as current export failures.

**Mitigation:** Restore access to retained artifacts or correct the supported
storage boundary. Do not reconstruct evidence from untrusted or untracked files.

**Recovery:** A controlled export returns a valid zip bundle containing its
manifest/player and expected retained media segments.

## BrowserPaneRetentionFailure

**Signal:** Workflow or recording retention reported at least one cleanup
failure in the rolling one-hour window.

**Triage:** Compare the two bounded failure counters and their candidate/deleted
or cleared counters. Check storage permissions, availability, and capacity while
preserving the configured retention and legal-hold responsibilities.

```promql
increase(browserpane_gateway_workflow_retention_failures_total[1h])
increase(browserpane_gateway_recording_retention_failures_total[1h])
```

**Mitigation:** Restore storage access and rerun only the supported retention
pass. Do not delete artifacts manually or shorten retention to clear capacity
without an approved policy decision.

**Recovery:** A subsequent retention pass completes, expected deletion/clear
counters advance, and no further failures occur for the affected retention
window.

## Escalation And Closure

Escalate immediately when a critical alert persists after one reversible
recovery attempt, when evidence integrity is uncertain, or when mitigation
would cross the tested capacity/security boundary. Include only sanitized
aggregate charts, reviewed release/configuration identifiers, timestamps, and
completed runbook steps.

Close the incident only after the alert resolves, one representative operation
passes, dependent alerts are checked, and any temporary mitigation has a named
owner and expiry. Threshold changes require review and a corresponding rule test.
