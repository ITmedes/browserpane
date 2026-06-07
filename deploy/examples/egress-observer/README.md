# BrowserPane Egress Observer Example

This example shows where outbound network communication should be tracked:
at the configured egress proxy, not in the BrowserPane gateway or browser
stream. BrowserPane records the selected egress profile and emits safe runtime
correlation metadata; the proxy records destinations, status, bytes, and timing.

The example uses Squid as a plain forward proxy. HTTPS traffic is not
man-in-the-middle inspected, so proxy logs normally contain `CONNECT host:443`
for TLS traffic rather than request paths or response bodies. The same compose
file also starts an auth-enforcing Squid proxy at
`bpane-egress-auth-observer:3130` with local test credentials
`proxy-user / proxy-pass` so proxy-auth success and rejection paths can be
validated without a production proxy.

For local full HTTPS inspection, also start the mitmproxy-based TLS observer in
`compose.tls.yml`. It listens as `bpane-egress-tls-observer:3129` and mints
per-site certificates from the local egress CA keypair. BrowserPane then
installs the CA certificate into the docker runtime's Chromium trust store. The
egress profile must be created with `traffic_observation.mode=tls_intercept`
plus a `sensitive_log_sink_ref` so decrypted-log routing remains explicit in
the control-plane metadata.

## Start The Observer Proxy

Start the normal BrowserPane stack first, then start this observer on the same
Docker network:

```bash
docker compose -f deploy/compose.yml up --build
docker compose -f deploy/examples/egress-observer/compose.yml up --build
```

The compose file starts:

- `bpane-egress-observer:3128` for metadata-only proxy observation without
  authentication.
- `bpane-egress-auth-observer:3130` for metadata-only proxy observation with
  Basic proxy authentication.

If your main compose project uses a non-default network name, pass it
explicitly:

```bash
BPANE_EGRESS_OBSERVER_NETWORK=<compose-project>_bpane-internal \
  docker compose -f deploy/examples/egress-observer/compose.yml up --build
```

## Start The TLS-Intercept Observer

Prepare mitmproxy's local CA material from the same CA certificate that
BrowserPane will install into the runtime, then start the TLS observer alongside
the plain observer:

```bash
deploy/examples/egress-observer/prepare-mitmproxy-ca.sh
docker compose -f deploy/examples/egress-observer/compose.tls.yml up
```

If your main compose project uses a non-default network name, pass it exactly as
with the plain observer:

```bash
BPANE_EGRESS_OBSERVER_NETWORK=<compose-project>_bpane-internal \
  docker compose -f deploy/examples/egress-observer/compose.tls.yml up
```

The TLS observer logs decrypted HTTP request lines and response status codes.
Use it only for local development or an approved sensitive-log sink.

## Create An Egress Profile

Point an owner-scoped BrowserPane egress profile at the observer. The proxy name
is resolvable from docker-backed runtime containers because both compose stacks
share the same network.

```bash
./scripts/bpane egress-profile create local-egress-observer \
  --description "Local Squid access-log observer" \
  --label observer=local-squid \
  --proxy-url http://bpane-egress-observer:3128 \
  --bypass-rule localhost \
  --bypass-rule 127.0.0.1 \
  --bypass-rule "*.local"
```

To validate proxy authentication, create a credential binding through the admin
app or `/api/v1/credential-bindings` with a JSON payload containing
`username=proxy-user` and `password=proxy-pass`, then point an egress profile at
the auth observer:

```bash
./scripts/bpane egress-profile create local-auth-egress-observer \
  --description "Local authenticated Squid access-log observer" \
  --label observer=local-squid-auth \
  --proxy-url http://bpane-egress-auth-observer:3130 \
  --proxy-credential-binding-id <credential-binding-id> \
  --bypass-rule localhost \
  --bypass-rule 127.0.0.1 \
  --bypass-rule "*.local"
```

Running `./scripts/bpane egress-profile diagnostics probe
<egress-profile-id>` performs a real proxy request. A valid binding reports a
healthy profile reachability probe. A missing binding, unavailable credential
provider, malformed secret payload, or rejected proxy credential reports a
sanitized failure reason without returning the secret value.

For the TLS-intercept observer, include the CA and sensitive-log metadata:

```bash
./scripts/bpane egress-profile create local-tls-observer \
  --description "Local mitmproxy TLS observer" \
  --label observer=local-mitmproxy \
  --proxy-url http://bpane-egress-tls-observer:3129 \
  --bypass-rule localhost \
  --bypass-rule 127.0.0.1 \
  --custom-ca-ref file:///workspace/dev/egress-ca.pem \
  --custom-ca-name "BrowserPane Local Egress Test CA" \
  --traffic-observation-mode tls_intercept \
  --sensitive-log-sink-ref siem://browserpane/local-egress \
  --sensitive-log-sink-name "Local Egress SIEM"
```

On `localhost`, the admin app creates the plain proxy and TLS-interceptor
profiles automatically for the signed-in owner if they are missing. The manual
commands above are still useful for CLI-only testing or non-local deployments.

Create or start a session with the returned profile id:

```bash
./scripts/bpane session create \
  --label purpose=egress-observer \
  --egress-profile-id <egress-profile-id>
```

## Read The Logs

Squid writes access logs to container stdout:

```bash
docker compose -f deploy/examples/egress-observer/compose.yml logs -f egress-proxy
docker compose -f deploy/examples/egress-observer/compose.yml logs -f egress-auth-proxy
```

The TLS observer writes mitmproxy flow logs to container stdout:

```bash
docker compose -f deploy/examples/egress-observer/compose.tls.yml logs -f egress-proxy
```

Use the BrowserPane runtime labels to correlate a proxy client IP back to the
session and egress profile:

```bash
deploy/examples/egress-observer/correlate-session-ip.sh
```

Docker-backed BrowserPane runtime containers carry these labels:

- `browserpane.session_id`
- `browserpane.egress_profile_id`
- `browserpane.egress_observation_mode`
- `browserpane.egress_proxy_configured`
- `browserpane.egress_proxy_auth_configured`
- `browserpane.egress_bypass_rule_count`
- `browserpane.egress_custom_ca_configured`
- `browserpane.egress_tls_interception_enabled`
- `browserpane.egress_sensitive_log_sink_configured`

The gateway also emits a startup audit log named
`starting docker runtime with egress observer correlation` with the session id,
container name, egress profile id/name, and sanitized profile flags. Use that
log to join control-plane events with proxy access logs.

If a profile references `proxy.credential_binding_id`, BrowserPane resolves the
secret at runtime launch and configures Chromium proxy authentication from a
session-local auth file. The credential value is not written to proxy URLs,
Docker labels, API diagnostics, CLI output, or gateway startup logs. Profile
reachability probes resolve the same binding and send proxy credentials only to
the configured proxy.

## Report Sanitized Usage Counters

BrowserPane exposes `/api/v1/sessions/{id}/egress-usage` for sanitized byte
counter ingestion. The local reporter reads the example Squid containers and
Docker runtime labels, correlates proxy client IPs to BrowserPane sessions, and
posts only byte deltas plus safe observer metadata to the gateway. It does not
send proxy URLs, response status, headers, timing, payload, credentials, CA
material, or decrypted traffic.

Preview the correlated reports first:

```bash
BPANE_ACCESS_TOKEN=<owner-token> \
  node deploy/examples/egress-observer/egress-usage-reporter.mjs \
  --since 10m \
  --dry-run
```

Dry-run output is sanitized JSON and does not call the API or advance the local
watermark. To report the same batch, run without `--dry-run`:

```bash
BPANE_ACCESS_TOKEN=<owner-token> \
  node deploy/examples/egress-observer/egress-usage-reporter.mjs \
  --since 10m \
  --state /tmp/bpane-egress-usage-state.json
```

The example Squid log format exposes the transferred response or tunnel byte
field, so the reporter maps that value to `rx_bytes_delta` and leaves
`tx_bytes_delta=0`. Production deployments that need request-byte accounting
should implement that in the proxy or secure-web-gateway collector and call the
same BrowserPane API with sanitized deltas.

Local validation:

```bash
node --test deploy/examples/egress-observer/egress-usage-reporter.test.mjs
node --check deploy/examples/egress-observer/egress-usage-reporter.mjs
```

## Production Pattern

For production, point egress profiles at the enterprise egress proxy or secure
web gateway that already owns URL policy and log retention. BrowserPane should
not log request bodies or full decrypted traffic by default. A typical observer
pipeline is:

1. BrowserPane session resource/status exposes `network_identity` and
   `effective_egress`.
2. Gateway launch logs and docker labels map `session_id` to runtime container,
   egress profile, and container IP.
3. The egress proxy logs outbound traffic.
4. A log shipper or SIEM collector tails proxy logs and joins them with the
   BrowserPane session/profile correlation metadata.

For authenticated enterprise proxies, keep proxy-auth verification at the proxy
boundary: the proxy owns authentication accept/reject logs, while BrowserPane
exposes only sanitized binding/configuration state plus profile/session
diagnostics.
