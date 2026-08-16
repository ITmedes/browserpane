# BrowserPane Prometheus And Grafana Example

This example scrapes the gateway's private `GET /metrics` endpoint and loads a
starter set of BrowserPane recording and alert rules. It also provisions a
Grafana operations dashboard over those aggregate indicators.

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
- `grafana/dashboards/browserpane-operations.json`: 20-panel aggregate
  operations dashboard.
- `grafana/provisioning/`: repository-authoritative dashboard and private
  Prometheus datasource provisioning.
- `compose.grafana.yml`: optional private-network Prometheus and Grafana stack
  with no published host ports.

## Validation

Run the repository wrapper from the repository root:

```sh
node scripts/observability/validate-prometheus-rules.mjs
node --test scripts/observability/grafana-*.test.mjs
node scripts/observability/validate-grafana-dashboard.mjs
```

The wrapper runs the upstream `promtool` from this immutable image:

```text
prom/prometheus@sha256:69f5241418838263316593f7274a304b095c40bcf22e57272865da91bd60a8ac
```

It checks the Prometheus configuration, both rule files, and the deterministic
rule tests. Docker must be available. The image contains Prometheus and
`promtool` 3.12.0.

The Grafana validator creates a temporary internal Docker network, an immutable
synthetic scrape target, and the checked-in Prometheus/Grafana stack. It proves
datasource and dashboard provisioning, executes all 19 panel queries through
Grafana's authenticated datasource API, checks error-level startup logs and
host-port isolation, and removes every temporary resource. Grafana OSS 13.0.2
is pinned by multi-architecture digest in the plan and validation contract.

## Private Local Stack

Attach the optional stack only to a network already shared with the gateway:

```sh
export BPANE_OBSERVABILITY_NETWORK=deploy_runtime-broker-api
export BPANE_GRAFANA_ADMIN_USER=bpane-operator
export BPANE_GRAFANA_ADMIN_PASSWORD='operator-supplied-secret'
docker compose -f deploy/examples/observability/compose.grafana.yml up -d --wait
```

The example publishes no host ports. Use an operator-controlled tunnel or a
separate loopback-only Compose override for interactive access. Do not commit
the administrator password or place it in the datasource URL. Stop the example
with the same environment and Compose file using `docker compose ... down`.

## Exposure

Keep Prometheus and the gateway metrics endpoint on a trusted operator network.
Keep Grafana on the same class of trusted operator network. Grafana anonymous
access and sign-up are disabled, optional plugin discovery is disabled, and its
administrator credential must be supplied at runtime. The example does not
configure TLS termination, external identity/RBAC, durable storage,
Alertmanager routing, backup, HA, or a public listener. Those remain deployment
responsibilities.
