# BrowserPane Prometheus Example

This example scrapes the gateway's private `GET /metrics` endpoint and loads a
starter set of BrowserPane recording and alert rules.

The rules are initial operating proposals. They are not contractual SLOs,
support commitments, or evidence of a tested capacity envelope. Calibrate
thresholds and hold times against a named deployment and workload before using
them for paging or external reporting.

## Files

- `prometheus.yml`: private gateway scrape and rule loading.
- `recording-rules.yml`: bounded service indicators derived from shipped metric
  families.
- `alert-rules.yml`: conservative starter alerts with operator runbook links.
- `rule-tests.yml`: deterministic Prometheus rule behavior tests.

## Validation

Run the repository wrapper from the repository root:

```sh
node scripts/observability/validate-prometheus-rules.mjs
```

The wrapper runs the upstream `promtool` from this immutable image:

```text
prom/prometheus@sha256:69f5241418838263316593f7274a304b095c40bcf22e57272865da91bd60a8ac
```

It checks the Prometheus configuration, both rule files, and the deterministic
rule tests. Docker must be available. The image contains Prometheus and
`promtool` 3.12.0.

## Exposure

Keep Prometheus and the gateway metrics endpoint on a trusted operator network.
The example does not configure authentication, TLS termination, durable
storage, Alertmanager routing, or a public listener.
