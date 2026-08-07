# Control API Compatibility Policy

## Scope

`openapi/bpane-control-v1.yaml` is the canonical owner-scoped BrowserPane
control contract. This policy applies to that document, generated operation
evidence, admin and CLI consumers, workers that use documented operations, and
external integrations built against v1.

Legacy compatibility routes, health probes, WebTransport, MCP transports, and
internal container interfaces are not silently covered by this policy. They
need their own documented contract before compatibility claims apply.

## Source Of Truth

- API behavior is implemented by the gateway router and handlers.
- The supported public wire contract is the OpenAPI document.
- `openapi/bpane-control-v1.operations.json` is generated evidence, not an
  independently edited contract.
- `openapi/bpane-control-v1.classifications.json` owns operator and worker
  audience classification and is consumed by the #158 API companion.
- `openapi/bpane-control-v1.examples.json` contains executable representative
  request/response fixtures.
- `openapi/bpane-control-v1.compatibility.json` inventories checked non-v1
  surfaces separately and must not collide with frozen operation paths.

Code and OpenAPI must change together. A handler-only or document-only public
change is incomplete.

## Compatibility Baseline

Pull requests compare the revision with their base commit. Local checks default
to `origin/main` and accept `--base-ref` for release or maintenance branches.
A release process may add a second comparison with the latest supported v1
release artifact; it must not replace the pull-request comparison.

The baseline and revision are processed locally. Contract content must not be
uploaded to a hosted diff service. The frozen contract is self-contained:
non-fragment `$ref` values are rejected before lint, example, or diff tooling
can resolve or fetch them.

## Allowed V1 Changes

Additive changes are normally compatible when they do not alter existing
semantics:

- new operations and optional request fields,
- new optional response fields,
- new response codes that do not replace an existing documented outcome,
- broader accepted request values,
- clearer descriptions, examples, tags, and deprecation metadata,
- new security alternatives that do not invalidate existing credentials.

Every additive operation still requires a unique operation id, audience
classification, route registration, security class, responses, and tests.

## Breaking V1 Changes

The compatibility gate rejects, at minimum:

- removed paths, methods, parameters, request bodies, or responses,
- newly required request fields or parameters,
- removed or newly optional required response fields,
- narrowed types, formats, ranges, enums, or content types,
- incompatible security requirements,
- renamed operation ids used by generated or external clients,
- response-envelope or lifecycle semantic changes that invalidate a conforming
  v1 client.

A breaking change is not approved by editing generated evidence or suppressing
the check. It requires an additive v1 design or a new versioned contract.
Changes the semantic engine cannot classify also fail closed and require a
reviewed contract design; they are not treated as compatible by default.

## Deprecation And Support Window

Deprecation is additive. Mark the operation or schema as deprecated, identify
the replacement, link the migration issue, and publish the intended version
boundary. Deprecation does not authorize removal from v1.

An externally supported contract version receives an overlap window before
end-of-support. The release policy must publish the exact date and supported
deployment versions; until that exists, v1 has no implicit end-of-support date.
Removal occurs only in a new version after the published overlap window.

## Emergency Corrections

Security, legal, or data-integrity incidents may require immediate behavior
restriction. The response must:

1. record the incident and affected operations,
2. prefer an additive denial/error outcome already allowed by the contract,
3. publish client impact and recovery guidance,
4. use a new versioned contract if the wire shape must break,
5. retain an auditable reviewer decision.

There is no permanent ignore file or silent CI bypass for breaking v1 changes.

## Error, Pagination, Idempotency, And Correlation Rules

- JSON errors use the shared `ErrorResponse` envelope where documented.
- List operations in the current v1 contract are unpaginated. Introducing
  pagination must be additive or versioned and must not silently replace the
  documented list envelope.
- Idempotency exists only where an operation explicitly documents a stable
  request key or idempotent lifecycle behavior. Clients must not infer it from
  HTTP method alone.
- The current v1 contract does not guarantee a public correlation header.
  Future request/trace correlation is additive only after gateway behavior,
  OpenAPI headers, logging, and tests ship together.
- Owner bearer, session automation, and unauthenticated handshake operations
  remain separate generated security classes.

## Required Validation

Run before reviewing a public API change:

```bash
npm ci --ignore-scripts --prefix scripts/openapi
npm test --prefix scripts/openapi
npm run check --prefix scripts/openapi
npm run compatibility --prefix scripts/openapi -- --base-ref origin/main
cargo test -p bpane-gateway openapi_contract
```

The full fast and Compose profiles remain required because schema conformance
does not prove authorization, persistence, lifecycle, browser, worker, or
runtime behavior.
