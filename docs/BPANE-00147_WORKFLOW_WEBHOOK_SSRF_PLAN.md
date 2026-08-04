# BPANE-00147 Workflow Webhook SSRF Plan

## Metadata

- Issue: [#147](https://github.com/ITmedes/browserpane/issues/147)
- State: Review
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Branch: `feature/BPANE-00147`
- Depends on: #145 token-domain and URL credential cleanup
- Last verified: 2026-08-04 on `feature/BPANE-00147` at `fb03270`

## Business Outcome

Prevent workflow event subscriptions from turning the gateway into an outbound
request primitive for internal services, cloud metadata endpoints, local
listeners, or redirect targets. Keep signed webhook delivery available for
approved receivers without building a custom URL parser, HTTP client, DNS
resolver, or IP-network implementation.

## Example Use Case

An operator registers `https://events.example.com/browserpane` for workflow
completion events. BrowserPane parses and resolves the destination, verifies
that every resolved address is globally routable, pins those approved
addresses into the outbound HTTP connection, and sends the signed event without
following redirects. A target using `127.0.0.1`, `169.254.169.254`, an RFC 1918
address, an internal DNS result, or a public URL that redirects internally is
rejected. A local test receiver remains possible only when its exact origin is
configured by the deployment operator.

## Current Evidence

- Subscription validation uses `starts_with("http://")` and
  `starts_with("https://")` instead of URL semantics.
- The shared `reqwest::Client` uses the default redirect policy, which follows
  up to ten redirects.
- DNS answers are not inspected or pinned, leaving private-address and
  time-of-check/time-of-use rebinding paths open.
- Persisted subscriptions are delivered without revalidation.
- Existing delivery tests prove signatures, ordering, retries, and diagnostics,
  but intentionally use unrestricted loopback receivers.
- The compose workflow-event test targets `127.0.0.1:1` and proves delivery
  records exist, not that an approved receiver obtained a signed event.

## Standards And Library Decisions

Follow the OWASP SSRF prevention model: use a standards parser, inspect all A
and AAAA results, disable redirects, apply an explicit allowlist for intentional
exceptions, and avoid a separate validation lookup followed by an unpinned
connection.

Use established components for protocol-sensitive behavior:

- `reqwest::Url` / `url`: URL Standard parsing, host normalization, scheme,
  credential, port, query, and fragment access.
- `ip_network`: IPv4/IPv6 network classification and globally routable checks;
  supplement only with standard-library IPv4-mapped normalization and an
  explicit multicast rejection.
- `tokio::net::lookup_host`: asynchronous system DNS resolution.
- `reqwest::redirect::Policy::none`: no automatic redirect traversal.
- `reqwest::ClientBuilder::resolve_to_addrs`: bind the request hostname to the
  already validated addresses and close the DNS-rebinding/TOCTOU window.
- `reqwest::ClientBuilder::no_proxy`: disable implicit environment proxy
  discovery so a proxy cannot independently resolve and bypass the pinned
  destination. Explicit webhook-proxy support requires a separate secure
  contract.

Do not introduce regex URL validation, hand-written IP bit masks, a custom HTTP
transport, or an unreviewed SSRF helper crate. Run `cargo audit` and the existing
dependency-safety workflow after adding `ip_network` as a direct dependency.

## Destination Policy

### Structural Validation

- Parse the complete value as an absolute URL.
- Accept only `https` by default.
- Require a host and a known or explicit port.
- Reject usernames, passwords, fragments, malformed hosts, and unsupported
  schemes.
- Preserve path and query because they are legitimate webhook endpoint data.
- Store the parser-canonicalized target URL.

### Network Validation

- Resolve domain targets immediately before use and inspect every returned IPv4
  and IPv6 address.
- Normalize IPv4-mapped IPv6 addresses before classification.
- Reject the destination when any answer is non-global, multicast, loopback,
  private, link-local, shared, documentation, benchmarking, reserved, or
  unspecified.
- Reject empty DNS responses and resolution failures with bounded diagnostics.
- Pin the complete approved address set into the request client while retaining
  the original hostname for HTTP Host and TLS SNI/certificate verification.

### Exact-Origin Exceptions

- Add repeatable deployment configuration for exact allowed origins in the form
  `scheme://host[:port]`.
- Reject allowlist entries containing credentials, path data beyond `/`, query,
  or fragments during gateway startup.
- An exact allowed origin may use HTTP or resolve to a non-global address; this
  is the explicit operator-controlled path for compose tests and internal
  enterprise receivers.
- Paths and queries beneath an allowed origin remain usable.
- Wildcards, suffix matching, regexes, and implicit private CIDR exceptions are
  out of scope for this slice.
- Redirects remain disabled even for an allowed origin.

### Enforcement Points

- Validate and canonicalize a target before a subscription is persisted.
- Re-resolve, revalidate, and pin the target immediately before every delivery,
  including subscriptions persisted before this release.
- Record dispatch-time policy rejection as a non-retryable failed delivery with
  a sanitized reason.
- Never include resolved private addresses, credentials, webhook payloads, or
  signing secrets in logs or operator diagnostics.

## Implementation Slices

### Slice 1: Policy And Configuration

- Add a focused destination-policy module around the standard libraries.
- Add exact-origin configuration to `WorkflowConfig` and compose.
- Parse and reject invalid configured origins during startup.
- Add the policy to workflow services and API state.

### Slice 2: API And Dispatch Enforcement

- Replace prefix validation with structural URL parsing.
- Validate and canonicalize subscription targets before persistence.
- Revalidate and pin DNS results before dispatch.
- Disable redirects and record policy failures without retries.

### Slice 3: Test Fixtures And Negative Coverage

- Adapt loopback delivery tests to explicit exact-origin authorization.
- Add parser, IP-literal, DNS-answer, allowlist, pinning, redirect, and persisted
  unsafe-subscription tests.
- Add a fixed compose webhook receiver route and configure only that origin.
- Make the compose e2e verify successful delivery rather than connection
  failure to port 1.

### Slice 4: Contracts And Documentation

- Update README, ARCH, AGENTS, runtime/deployment examples, delivery roadmap,
  risk register, capability maturity, and validation matrix where behavior or
  commands change.
- Update issue #147 with implementation and validation evidence.

## Test Strategy

### Unit

- Valid public HTTPS URL with path/query is canonicalized.
- Relative, malformed, unsupported-scheme, credential-bearing, fragment, and
  default HTTP targets are rejected.
- IPv4 and IPv6 loopback, private, link-local, multicast, unspecified,
  documentation, shared, benchmarking, and reserved literals are rejected.
- IPv4-mapped IPv6 private and loopback values are rejected.
- Mixed public/private DNS answers are rejected; all-public answers pass.
- Empty and failed DNS results fail closed.
- Exact origin matching handles default and explicit ports without wildcard or
  suffix behavior.

### Integration

- Subscription API returns bounded 400 responses for structurally or
  network-invalid targets.
- Explicitly allowed loopback receiver accepts signed events.
- A 3xx response is recorded as failure and its target receives no request.
- DNS answers used for validation are the addresses pinned into the HTTP client.
- A persisted destination that becomes unsafe is rejected at dispatch without
  a network request or retry.
- Existing signature, ordering, retry/backoff, and diagnostics behavior remains
  intact for approved receivers.

### Smoke And E2E

- Gateway compose API tests reject loopback/private/link-local targets that are
  not explicitly configured.
- The configured compose receiver accepts a signed workflow event and delivery
  diagnostics report success.
- Removing the exact-origin configuration causes the same receiver to be
  rejected.
- Full gateway tests, compose API surface tests, dependency audit, and repository
  validation pass.

## Rollout And Compatibility

- Public HTTP webhook targets become invalid unless their exact origin is
  explicitly allowed. This is an intentional secure-default change.
- Existing unsafe subscriptions remain stored for operator visibility but fail
  closed during delivery.
- No database migration is required because canonical URLs remain strings.
- Roll back code and configuration together; no stored-resource rollback is
  necessary.

## Definition Of Done

- URL decisions use the standard parser rather than string-prefix or regex
  logic.
- Every DNS answer is classified and dispatch uses the approved pinned results.
- Redirect following is disabled.
- Exact-origin exceptions are explicit, startup-validated, and documented.
- Creation-time and dispatch-time controls cover new and persisted resources.
- Negative parser, address, DNS, redirect, allowlist, API, and compose cases
  pass alongside existing workflow event tests.
- Dependency and documentation checks pass.
- Issue #147 and the canonical delivery roadmap match the implementation state.

## Post-Implementation Smoke Sequence

1. Run gateway formatting, lint, unit, integration, and dependency checks.
2. Start compose with the fixed local webhook origin explicitly allowed.
3. Submit malformed, credential-bearing, HTTP, loopback, private, link-local,
   multicast, unspecified, and mixed-DNS subscriptions and verify bounded 400
   responses.
4. Create a subscription for the configured receiver and execute a workflow.
5. Confirm one signed event reaches the receiver and delivery diagnostics show
   a successful attempt.
6. Exercise a receiver returning a redirect and confirm BrowserPane does not
   contact the redirect target.
7. Remove the allowed origin, restart the gateway, and confirm the local target
   is rejected at creation and dispatch.
8. Run the gateway compose API surface suites and repository document checks.

## Evidence Record

- `cargo test -p bpane-gateway workflow_event --no-fail-fast`: 21 focused tests
  passed.
- `cargo test -p bpane-gateway --no-fail-fast`: 382 gateway tests and all
  non-compose integration targets passed.
- Changed code passed Clippy with only the repository's pre-existing unrelated
  lint categories suppressed; the strict unsuppressed result was recorded.
- `node scripts/check-dependency-safety.mjs`: Cargo and seven npm lockfiles
  passed the repository policy.
- `scripts/run-gateway-compose-e2e.sh --suite default`: 16 authenticated
  compose API surfaces passed against rebuilt images in 416 seconds, including
  the signed workflow-event delivery receiver.
- `docker compose -f deploy/compose.yml config -q`, repository document checks,
  formatting, and diff checks passed.
