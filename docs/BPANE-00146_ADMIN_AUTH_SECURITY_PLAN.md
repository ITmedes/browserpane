# BPANE-00146 Admin Auth And Browser Security Plan

## Metadata

- Issue: [#146](https://github.com/ITmedes/browserpane/issues/146)
- State: In Progress
- Owner: `thebackplane`
- Lane: Foundation
- Target gate: Foundation Gate
- Branch: `feature/BPANE-00146`
- Depends on: #145 token-domain and URL credential cleanup
- Last verified: 2026-08-04 at `7c034a3`

## Business Outcome

Give `/admin/` and `/admin-new/` one standards-based browser authentication
contract so the new admin can become the default without retaining two
independent security implementations. Reject replayed or untrusted OIDC
callbacks before identity data is displayed, reduce persisted credential
exposure, and add browser response headers that limit script injection,
embedding, referrer leakage, and unintended browser capabilities.

## Example Use Case

An operator opens `/admin-new/`, signs in through Keycloak, leaves the console
open until an access token needs refresh, and later reloads the page. The app
accepts only an authorization response matching its state and nonce, verifies
the ID token against the issuer JWKS, displays only verified identity claims,
and redirects consistently when authentication can no longer be recovered. A
copied callback, wrong-audience token, unsigned token, or embedded admin page is
rejected without exposing token details.

## Current Evidence

- Both admin packages contain duplicate production auth modules.
- Authorization Code with PKCE and state is implemented, but nonce is absent.
- ID-token and access-token payloads are decoded without signature, issuer, or
  audience verification and can currently drive account display data.
- The complete token response, including refresh token, is persisted in
  `sessionStorage`.
- The static nginx routes set cache headers but no CSP, content-type, referrer,
  frame, or permissions policy.
- Existing browser smokes cover successful Keycloak login indirectly, but not
  callback rejection, storage policy, logout, or response headers explicitly.

## Scope

1. Add a framework-neutral `@browserpane/admin-auth` package consumed by both
   admin applications; keep route/UI composition in each app.
2. Use the maintained `jose` library for asymmetric ID-token signature and
   claim verification against OIDC discovery and JWKS.
3. Validate discovery issuer, token issuer, audience/authorized party, expiry,
   nonce, and bounded PKCE transaction age.
4. Persist access and ID tokens only in per-tab session storage; keep refresh
   tokens in memory and fall back to OIDC SSO after a page reload when refresh
   is no longer possible.
5. Make invalid callback, invalid stored identity, failed refresh, logout, and
   API authentication failure clear local state consistently.
6. Apply CSP and standard browser security headers to `/admin/` and
   `/admin-new/` without blocking Keycloak, API, WebTransport, WebSocket,
   microphone, camera, media, worker, or popup session-preview flows.
7. Add focused unit/integration tests and dedicated browser/header smokes for
   both admin routes.

## Non-Goals

- A backend-for-frontend, HttpOnly token cookie, reverse-proxy OIDC module, or
  Keycloak topology redesign. Those require a separate deployment contract.
- Removing the compatibility admin in this slice.
- Changing gateway bearer validation, roles, permissions, or owner semantics.
- Restricting dynamic external connect destinations beyond the current
  configurable IdP, gateway, WebTransport, and MCP integration requirements.
- Treating local demo credentials as a production deployment pattern.

## Security Contract

### OIDC Transaction

- Generate independent PKCE verifier, state, and nonce values with Web Crypto.
- Store only the short-lived transaction in `sessionStorage` with a creation
  timestamp and a ten-minute maximum age.
- Clear the transaction before code exchange completion so a callback cannot
  be replayed in the same tab.
- Require the returned state and verified ID-token nonce to match.

### Identity Verification

- Require discovery `issuer` to match configured issuer after trailing-slash
  normalization.
- Fetch `jwks_uri` with `no-store` and cache the parsed key set only in memory.
- Accept asymmetric signing algorithms supported by the implementation; never
  accept `none` or shared-secret ID-token algorithms.
- Verify signature, issuer, audience, expiry, and authorized party where
  multiple audiences are present.
- Map only validated string identity claims into the admin snapshot.

### Browser Token Storage

- Access and ID tokens may remain in `sessionStorage` because both current
  applications are static SPAs and must survive ordinary navigation/reload.
- Refresh tokens remain in memory and are omitted from persisted JSON.
- Restored ID tokens are reverified before claims or authenticated state are
  exposed.
- Removing all JavaScript-readable bearer material requires a future BFF and
  is not implied by this slice.

### Response Headers

- Admin routes receive a CSP with self-only scripts, no objects, no framing,
  constrained base/form/media/worker sources, and network schemes required by
  configured browser integrations.
- Add `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`, and a
  permissions policy compatible with BrowserPane camera/microphone use.
- Do not emit HSTS from the local HTTP-only web container; deployment ingress
  owns HSTS after TLS termination.

## Implementation Slices

### Slice 1: Shared Auth Core

- Add the package and replace duplicated app modules with narrow re-exports.
- Add OIDC discovery and wire validation, nonce-aware transaction state, ID
  token verifier, and storage policy.
- Add package-level tests for positive and negative security cases.

### Slice 2: Admin Integration And Recovery

- Initialize and reverify restored sessions asynchronously in both shells.
- Align automatic login, invalid-auth recovery, refresh failure, and logout.
- Keep route-specific account presentation and API clients unchanged.

### Slice 3: Browser Security Headers

- Add reusable nginx header directives to both admin locations.
- Add static contract tests and live response-header checks.
- Verify production bundles run under the CSP without browser console errors.

### Slice 4: Documentation And Evidence

- Update README, ARCH, AGENTS, security roadmap, validation matrix, and delivery
  state when behavior or commands change.
- Record exact unit, coverage, build, compose, and smoke evidence.

## Test Strategy

### Unit

- PKCE URL contains S256 challenge, state, and nonce.
- Missing, mismatched, expired, or malformed transaction state is rejected.
- Valid asymmetric ID token passes signature, issuer, audience, expiry, nonce,
  and authorized-party checks.
- Unsigned, wrong-key, wrong-issuer, wrong-audience, wrong-nonce, expired, and
  symmetric-algorithm tokens fail.
- Persisted token JSON excludes refresh tokens and malformed storage is cleared.
- Refresh rotation remains available in memory and failure clears auth state.

### Integration

- Both apps compile and execute against the same auth package.
- Successful login exposes only verified identity claims.
- Restored sessions are reverified before routes render.
- Invalid authentication triggers one bounded redirect instead of a loop.
- nginx contract tests prove both admin locations carry the same headers.

### Smoke And E2E

- Complete Keycloak login for `/admin/` and `/admin-new/`.
- Confirm persisted token state contains no refresh token.
- Exercise logout and automatic SSO login recovery.
- Exercise expired/invalid local auth recovery without an infinite redirect.
- Check live CSP, content-type, referrer, frame, and permissions headers.
- Run representative compatibility-admin and admin-new session/dashboard
  smokes to catch blocked network, media, popup, and WebTransport behavior.

## Rollback

- Revert the shared package, app re-exports, shell initialization, and nginx
  headers together.
- No database or gateway rollback is required.
- Existing per-tab token state can be cleared; operators authenticate again.
- Keep old and new admin builds on one shared auth version in every image.

## Definition Of Done

- Both admin apps use one production auth implementation.
- Identity display never consumes unverified JWT payloads.
- State, nonce, issuer, audience, expiry, signature, and transaction age have
  positive and negative coverage.
- Persisted admin token state contains no refresh token.
- Both routes recover consistently from expiry and invalid authentication.
- Both routes deliver the agreed browser security headers.
- Unit, integration, coverage, build, compose, and browser smoke checks pass.
- Issue #146 and security/delivery documentation agree with the implementation.

## Post-Implementation Smoke Sequence

1. Run shared auth package typecheck, tests, coverage, and build.
2. Run compatibility-admin and admin-new check, tests, coverage where
   configured, and production builds.
3. Build and start the compose web/gateway/Keycloak stack.
4. Fetch `/admin/` and `/admin-new/` and assert the complete security-header
   contract.
5. Sign in to both routes through Keycloak and confirm authenticated content.
6. Inspect per-tab storage and confirm no refresh token is persisted.
7. Log out, sign in again through existing SSO, and exercise invalid/expired
   local state recovery.
8. Run compatibility admin session and admin-new dashboard/session smokes.
9. Inspect browser console and nginx logs for CSP violations, redirect loops,
   token values, or authentication errors.

## Evidence Record

- Plan and issue alignment: pending first implementation commit.
- Shared auth validation: pending.
- Admin integration validation: pending.
- Header contract validation: pending.
- Compose/browser smoke evidence: pending.
