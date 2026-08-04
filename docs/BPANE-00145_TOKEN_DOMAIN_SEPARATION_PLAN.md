# BPANE-00145 Token Domain Separation Plan

## Metadata

- Issue: [#145](https://github.com/ITmedes/browserpane/issues/145)
- State: In Progress
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Branch: `feature/BPANE-00145`
- Depends on: #151 validation baseline, #184 compose sharding, #185 GHCR builder
- Last verified: 2026-08-04 at `b6fd13c`

## Business Outcome

Prevent a credential minted for one BrowserPane trust boundary from being
replayed at another boundary, and remove owner bearer credentials from browser
WebSocket URLs and gateway transport warning logs. This provides a stable
credential contract for the current admin-event consumer and the future
admin-new observability route without absorbing the broader OIDC, CSP, or
callback-network controls owned by #146 and #147.

## Example Use Case

An operator signs into BrowserPane, opens a live session, and keeps the admin
console open while an MCP worker automates that session. The browser receives a
session-connect ticket, the worker receives session-automation access, and the
admin console receives a short-lived admin-event credential. Replaying any of
these credentials against another endpoint fails deterministically. The admin
WebSocket authenticates after upgrade without putting an owner bearer or event
credential in its URL, and a rejected WebTransport request cannot write the
raw query string into gateway logs.

## Current Evidence

- Connect and automation managers receive the same root secret.
- Both token classes use the indistinguishable
  `v1.<payload>.<signature>` format and do not bind a purpose.
- Both validators compare signature byte vectors directly instead of using the
  HMAC verifier.
- `/api/v1/admin/events` accepts an owner bearer through the `access_token`
  query parameter.
- The compatibility admin constructs WebSocket URLs containing that owner
  bearer.
- WebTransport rejection warnings include the complete request path, including
  credential query values.

## Scope

1. Add a small shared codec for versioned, purpose-bound HMAC tokens.
2. Derive independent signing keys from the configured root secret for:
   - `session-connect`,
   - `session-automation`,
   - `admin-events`.
3. Emit `v2.<purpose>.<payload>.<signature>` and sign the canonical version,
   purpose, and payload input.
4. Verify signatures with `Hmac::verify_slice` and return typed malformed,
   wrong-purpose, invalid-signature, invalid-payload, and expired errors.
5. Add `POST /api/v1/admin/events/access-tokens`, authenticated with the owner
   bearer, to mint a short-lived admin-event token.
6. Change `GET /api/v1/admin/events` to accept no query credential. After the
   WebSocket upgrade it must require one bounded JSON authentication message
   within five seconds before snapshots start.
7. Update the compatibility-admin event client to mint a fresh event token on
   each connect/reconnect, open a credential-free WebSocket URL, and send the
   scoped token as the first message.
8. Strip query strings and control characters from WebTransport request
   targets before logging rejection diagnostics.
9. Update OpenAPI, README, architecture/project memory, validation guidance,
   and current security status.

## Non-Goals

- OIDC nonce, ID-token verification, refresh-token storage, CSP, or browser
  security headers; #146 owns these.
- Webhook target validation or SSRF controls; #147 owns these.
- Adding the admin-event consumer to admin-new; #156 owns the observability UI.
- Replacing WebTransport session-ticket query transport. The browser transport
  API currently requires this path, so the ticket remains short-lived,
  session-scoped, purpose-bound, and omitted from logs.
- General gateway key rotation, external KMS, audit retention, or enterprise
  credential lifecycle; #70/#76 own those controls.

## Token Contract

### Envelope

`v2.<purpose>.<base64url-json-payload>.<base64url-hmac-sha256>`

The HMAC input is the ASCII byte sequence
`v2.<purpose>.<base64url-json-payload>`. Each manager derives its signing key by
applying HMAC-SHA-256 to the root secret with the context
`browserpane/token-domain/v2/<purpose>`. The explicit purpose supports typed
wrong-purpose rejection; the derived key prevents a parser mistake from
turning purpose substitution into a valid token.

### Migration

- Mint only v2 tokens after deployment.
- Reject v1 connect and automation tokens. Their current maximum lifetime is
  five minutes and clients already mint fresh credentials before reconnect.
- A rollback to the previous release similarly invalidates v2 ephemeral
  credentials; clients recover by minting credentials from the active release.
- No persisted resource or database migration is required.

### Admin-Event Handshake

1. The browser calls `POST /api/v1/admin/events/access-tokens` with its owner
   bearer in the `Authorization` header.
2. The gateway returns a short-lived `admin_event_access_token` and expiry.
3. The browser opens `/api/v1/admin/events` without query parameters.
4. Within five seconds it sends
   `{"message_type":"admin.authenticate","token":"..."}`.
5. The gateway validates the `admin-events` purpose and sends
   `{"message_type":"admin.authenticated"}` before snapshots.
6. Missing, oversized, malformed, expired, wrong-purpose, or invalid tokens
   close the socket with a generic policy failure and never expose token data.

## Implementation Slices

### Slice 1: Purpose-Bound Token Core

- Add the internal codec and focused unit tests.
- Migrate connect and automation managers to v2.
- Add explicit cross-purpose rejection tests.
- Preserve existing public response `token_type` values and client behavior.

### Slice 2: Scoped Admin-Event Access

- Add the admin-event manager and principal claims.
- Wire its TTL through typed gateway configuration with a 60-second default.
- Add the owner-authenticated issuance route.
- Replace query/header upgrade authentication with bounded first-message
  authentication and an acknowledgement.

### Slice 3: Browser Client And Logging

- Update the compatibility admin event client and tests.
- Ensure reconnect mints a fresh event token.
- Sanitize transport request targets before every rejection log.
- Extend the existing reconnect smoke to assert the credential-free URL and
  successful post-restart authentication.

### Slice 4: Contract And Documentation

- Update the frozen owner-scoped OpenAPI additively.
- Remove the owner-bearer query parameter from the WebSocket operation.
- Update README, AGENTS, API coverage, security roadmap, validation matrix, and
  issue evidence.
- Re-run repository document and OpenAPI conformance checks.

## Test Strategy

### Unit

- Stable v2 envelope and signature verification.
- Tampered version, purpose, payload, and signature rejection.
- Connect/automation/admin-event cross-purpose rejection in every direction.
- Expired and malformed payload handling.
- Admin principal round trip, including bounded safe claims.
- Transport request target sanitization for token values, CR/LF, fragments,
  malformed query strings, and path-only input.
- Admin client issuance mapping, first-message authentication, acknowledgement,
  reconnect, close, and HTTP 401 handling.

### Integration

- Owner bearer can mint an admin-event token; missing/invalid owner bearer
  cannot.
- Connect ticket cannot authorize an automation route and automation token
  cannot establish a browser transport session.
- WebSocket rejects missing, malformed, expired, and wrong-purpose first
  messages and streams snapshots after valid authentication.
- OpenAPI includes the issuance operation and no longer advertises query auth.

### Smoke And E2E

- Compatibility-admin event stream connects and receives snapshots.
- Gateway restart triggers reconnect with a freshly minted event token.
- Session connect/reconnect remains functional with v2 connect tickets.
- MCP/workflow automation remains functional with v2 automation tokens.
- Gateway logs contain no raw owner, connect, automation, or event credential.
- Admin-new session/auth smokes remain green even though it does not yet consume
  admin events.

## Rollback

- Revert the additive API route, admin-event handshake, and token codec together.
- No stored data rollback is required.
- Existing ephemeral v2 tokens expire or become invalid after rollback; clients
  mint fresh credentials from the restored version.
- Keep the compatibility admin event client and gateway in one deployable web
  image so the handshake cannot be rolled out independently in local compose.

## Documentation Impact

- `README.md`: describe event-token minting and query-free WebSocket auth.
- `AGENTS.md`: update gateway and compatibility-admin ownership facts.
- `openapi/bpane-control-v1.yaml`: additive access-token route and revised
  WebSocket authentication contract.
- `docs/ADMIN_NEW_API_COVERAGE.md`: classify the new owner operation.
- `docs/SECURITY_RUNTIME_ROADMAP.md`: mark this cleanup implemented after
  validation.
- `docs/VALIDATION_MATRIX.md`: record concrete regression commands/evidence.
- `ARCH.md`: update only if it currently describes credential transport or
  signing topology; otherwise record that no architecture-level topology
  change is required.

## Definition Of Done

- Token classes are cryptographically and syntactically purpose-separated.
- HMAC verification is constant-time through the library verifier.
- Owner bearers and event credentials never appear in admin-event WebSocket
  URLs.
- WebTransport rejection logs never include query credentials or injected log
  control characters.
- Compatibility-admin realtime/reconnect behavior remains operational.
- Gateway, compatibility-admin, admin-new regression, OpenAPI, and compose
  checks pass.
- Issue #145, this plan, README, AGENTS, and API documentation agree.

## Post-Implementation Smoke Sequence

1. Run gateway token/admin-event/transport unit and API tests.
2. Run compatibility-admin check, tests, coverage, and build.
3. Run admin-new check, tests, coverage, and build as an auth/session regression.
4. Start compose and sign in to `/admin/` through Keycloak.
5. Confirm the admin event socket URL has no query string, its first outbound
   message uses a scoped event token, and session/workflow changes update live.
6. Restart the gateway and confirm the admin event stream reconnects with a new
   event token.
7. Connect and reconnect a browser session, then delegate MCP and execute one
   real MCP operation.
8. Present each token class to the other two validators/endpoints and verify
   deterministic rejection.
9. Inspect gateway/web logs for known credential samples and verify none are
   present.
10. Run the compatibility-admin event reconnect, admin-new sessions, MCP, and
    representative workflow compose smokes.

## Evidence Record

- Plan and issue alignment: pending first implementation commit.
- Unit/API validation: pending.
- Compatibility-admin validation: pending.
- Admin-new regression validation: pending.
- Compose smoke evidence: pending.

