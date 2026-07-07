# Identity And Access Requirements

This document consolidates the external identity, service-principal, identity
mapping, and admin identity-polish requirements.

## Identity Foundation

BrowserPane uses external OIDC for authentication. BrowserPane identity
resources should normalize safe principal metadata and expose access-review
facts without becoming a full identity provider.

Required principal normalization:

- subject,
- issuer,
- display name,
- client id,
- principal type,
- safe group/claim values where explicitly allowlisted,
- no raw token payloads or credentials in API/admin output.

Required access-review endpoints:

- `GET /api/v1/identity/me`,
- `GET /api/v1/identity/access-review`.

Access review must include:

- current principal,
- project summaries,
- resource counts,
- visible session counts,
- active delegated principals,
- registered/unregistered service-principal evidence,
- generated-at timestamp,
- unmapped safe principal signals where useful.

## Service Principal Registry

External automation clients need an owner-scoped registry before BrowserPane
can provide stronger service-principal lifecycle, API keys, audit, and policy
behavior.

Required service-principal metadata:

- id,
- external `client_id`,
- external `issuer`,
- display name and description,
- state: `active` or `disabled`,
- labels,
- allowed scope names,
- optional allowed project ids,
- created/updated timestamps,
- last-seen/last-used timestamps where inferable.

Required routes:

- `POST /api/v1/service-principals`,
- `GET /api/v1/service-principals`,
- `GET /api/v1/service-principals/{id}`,
- `PUT /api/v1/service-principals/{id}`.

Required validation and behavior:

- prevent duplicate active entries for the same issuer/client id,
- reject new automation-owner delegation to disabled registered principals,
- allow unregistered delegated clients for compatibility but mark them as
  unregistered in review output,
- disabling a principal should not automatically delete existing delegated
  sessions; cleanup stays explicit,
- list/detail/admin views must not expose raw token claims or secret material.

## Identity Mappings

Identity mappings connect safe external identity signals to BrowserPane project
access and future policy decisions.

Required mapping metadata:

- mapping id,
- kind: user, group, claim, or service principal,
- external issuer,
- external subject/group/claim value or registered service-principal id,
- project id,
- scopes,
- display metadata,
- lifecycle state,
- created/updated timestamps.

Required behavior:

- evaluate active mappings in access review,
- keep disabled mappings visible but ineffective,
- show mapped project names in admin rows/details,
- expose unmapped safe principal signals for administration,
- preserve compatibility with OIDC as the authentication source,
- store sanitized mapping metadata, not raw tokens or secrets.

## Admin-New Requirements

The unified admin app still needs a route-backed `/admin-new/identity` surface.

That route should provide:

- current principal summary,
- project access review,
- resource counts,
- delegated automation principals,
- service-principal catalog with create/edit/disable/re-enable,
- identity-mapping catalog with create/edit/disable/re-enable,
- project names in rows and details,
- unmapped signal evidence,
- disabled/active/registered/unregistered status badges,
- validation errors close to the affected form controls,
- safe token-claim rendering without raw tokens.

## CLI Requirements

The operator CLI should support:

- `identity me`,
- `identity access-review`,
- `service-principal create`,
- `service-principal list`,
- `service-principal get`,
- `service-principal update`,
- identity mapping commands where supported by the current CLI.

CLI output must stay JSON-stable and redact sensitive token material.

## Manual Smoke

1. Start local compose and sign in as `demo / demo-demo`.
2. Create or select a project.
3. Create a service-principal registry entry for `bpane-mcp-bridge`.
4. Create/select a session and delegate MCP.
5. Open identity/access review and confirm the registered principal appears as
   active with delegated-session linkage.
6. Disable the registered principal.
7. Attempt to delegate MCP to a fresh session and confirm a disabled-principal
   validation error.
8. Re-enable the principal and verify delegation succeeds.
9. Create mappings for a registered service principal and a safe group/claim
   value available in the local token/test fixture.
10. Confirm access review marks active matching mappings effective and disabled
    mappings ineffective.
11. Confirm raw token contents are absent.
12. Run CLI `identity me`, `identity access-review`, and service-principal
    commands.

## Validation

Use the relevant subset:

- `cargo test -p bpane-gateway identity -- --nocapture`
- `cargo test -p bpane-gateway identity_mapping -- --nocapture`
- `cargo test -p bpane-gateway service_principal -- --nocapture`
- `cargo test -p bpane-gateway --test compose_api_surface compose_identity_access_review_api_surface -- --ignored --test-threads=1`
- `cargo test -p bpane-gateway --test compose_api_surface compose_service_principals_api_surface -- --ignored --test-threads=1`
- `cd code/web/bpane-admin && npm test && npm run check && npm run build`
- `cd code/web/bpane-client && npm test -- js/__tests__/bpane-cli.test.ts`
- `cd code/web/bpane-client && npm run smoke:bpane-cli -- --headless`
- `cd code/web/bpane-client && npm run smoke:admin-session -- --headless`

## Out Of Scope

- BrowserPane-issued API keys and client secrets,
- SCIM server implementation,
- SAML configuration,
- break-glass accounts,
- central policy engine,
- immutable audit/event export,
- automatic deletion of existing delegated sessions on principal disablement,
- project quota/admission enforcement directly from identity mappings until the
  governance layer consumes those facts.
