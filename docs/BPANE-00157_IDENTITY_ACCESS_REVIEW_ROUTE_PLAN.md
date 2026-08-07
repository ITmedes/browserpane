# BPANE-00157 Identity And Access Review Route Plan

Status: implemented and validated on 2026-08-07.

## Issue

- Canonical issue: [#157 Add admin-new identity and access review route](https://github.com/ITmedes/browserpane/issues/157)
- Branch: `feature/BPANE-00157`
- Work-order item: 13 in `IMPLEMENTATION_WORK_ORDER.md`
- Roadmap slice: F in `NEXT_WORKING_ROADMAP.md`

## Business Case

BrowserPane already authenticates operators through external OIDC and exposes a
sanitized owner-scoped access review. The admin-new application needs one
route-backed place where an operator can answer who they are, which projects
and resources are visible, which external automation identities are registered,
which safe identity signals map to projects, and which delegated clients are
currently attached to sessions.

This slice is an operator review and registry surface. Service-principal scopes,
allowed projects, and identity mappings are metadata plus current access-review
evidence; they must not be presented as complete RBAC enforcement. Issue #176
owns organization/project role and service-principal grant enforcement.

## Example Use Case

An operator investigates why an external automation client cannot be delegated
to a session. In `/admin-new/identity`, they verify the current OIDC principal,
find the client in the service-principal registry, see that it is disabled,
review its delegated-session evidence and intended project metadata, re-enable
it, and confirm the access review refreshes. They can then create a sanitized
project mapping for that registered client without viewing raw tokens, client
secrets, or unrestricted claims.

## Current Implementation State

- The gateway exposes and validates:
  - `GET /api/v1/identity/me`,
  - `GET /api/v1/identity/access-review`,
  - service-principal list/create/get/update,
  - identity-mapping list/create/get/update.
- Access review includes the current principal, project summaries, resource
  counts, mapping effectiveness, safe unmapped signals, registered principals,
  and delegated automation principals.
- Disabling a registered service principal blocks new delegation but does not
  delete existing delegated sessions.
- The compatibility admin contains identity view-model and form logic, but its
  monolithic panel is not the admin-new design baseline.
- Admin-new navigation advertises `/admin-new/identity`, but the route and its
  domain client/components do not exist.
- The operator CLI already covers identity and service-principal operations;
  identity-mapping CLI parity is tracked separately where needed.

## Scope

### 1. Typed Identity Client

- Add admin-new identity types that compose the existing project resource type.
- Add strict response mapping for access review, service principals, identity
  mappings, delegated principals, and safe unmapped signals.
- Add authenticated list/get/create/update client operations.
- Preserve structured API error details and the global authentication-failure
  handler.
- Reject malformed payloads before they enter route state.

### 2. Review View Models

- Build principal, project-access, resource-count, delegation, mapping,
  service-principal, and unmapped-signal projections outside Svelte components.
- Resolve project ids to human-readable project names while retaining a short
  stable identifier.
- Label active, disabled, registered, unregistered, effective, and ineffective
  states consistently.
- Explain the current metadata-versus-enforcement boundary without rendering
  raw token payloads or secret material.

### 3. Service-Principal Management

- Add a searchable catalog with selected-item summary.
- Add create/edit forms using the established admin-new resource patterns.
- Support explicit disable and re-enable actions through the update API.
- Keep field validation adjacent to the affected controls.
- Treat issuer and client id as external identity metadata; never request or
  display client secrets.

### 4. Identity-Mapping Management

- Add a searchable catalog with selected-item summary.
- Add create/edit forms for user, group, allowlisted claim, and registered
  service-principal mappings.
- Resolve project and service-principal selections from the current review.
- Support explicit disable and re-enable actions.
- Make mapping effectiveness review evidence visible and avoid claiming that a
  mapping alone grants fully enforced project authorization.

### 5. Route Composition And Feedback

- Add `/admin-new/identity` as a deep-linkable and refresh-safe route.
- Present current principal and summary evidence before management catalogs.
- Use contained sections/tabs to keep the route scannable rather than placing
  every form on one canvas.
- Provide loading, empty, malformed-response, permission, request, validation,
  action-success, and retry states with established admin-new messages.
- Refresh the complete access review after successful mutations so resource
  counts, mapping effectiveness, and delegation evidence remain coherent.

### 6. Documentation And Issue Alignment

- Update admin-new status, requirements, coverage, checkpoints, implementation
  order, roadmap, and issue-context documents with actual delivered behavior.
- Update `README.md` only if the public capability summary or test commands need
  a corresponding change; otherwise record why no README edit is required.
- Keep #157 acceptance evidence and smoke results aligned with this plan.

## Test Strategy

### Unit

- Strict mapper success and malformed-payload cases for every identity shape.
- Client request method/path/body/authentication/error tests.
- View-model formatting, project-name resolution, filtering, status, and
  redaction tests.
- Form defaults, command construction, conditional fields, labels, scopes,
  project selection, and validation failures.
- Independent component tests for summary, catalogs, selected-item views,
  editors, and messages.

### Integration

- Route load, deep-link refresh, retry, authentication failure, and partial or
  unavailable API behavior.
- Create/edit/disable/re-enable service-principal flows followed by coherent
  access-review refresh.
- Create/edit/disable/re-enable mapping flows followed by effectiveness refresh.
- Safe rendering checks that reject raw-token, authorization-header, secret,
  password, and private-key fields even if unexpected input reaches a fixture.
- Existing compatibility-admin, gateway identity, mapping, and
  service-principal tests remain green.

### Compose Smoke / E2E

1. Start compose and authenticate through local Keycloak as `demo / demo-demo`.
2. Deep-link to `/admin-new/identity`, refresh, and verify the current principal,
   project summaries, resource counts, and empty states.
3. Create a service principal with project/scopes metadata; verify it appears in
   the catalog and access review.
4. Edit it, disable it, re-enable it, and verify each persisted state after a
   page refresh.
5. Create user/claim and registered-service-principal mappings; verify project
   names, safe external identity evidence, and effective/ineffective state.
6. Disable and re-enable a mapping and verify access review changes coherently.
7. Delegate MCP to a session and verify registered/delegated session evidence is
   correlated without exposing bearer tokens or secret material.
8. Exercise invalid issuer/client/mapping/project inputs and verify actionable
   validation or API feedback next to the relevant operation.
9. Run gateway compose identity, service-principal, and mapping tests plus the
   compatibility admin identity regression.

## Acceptance Criteria

- `/admin-new/identity` loads directly and after browser refresh.
- The current principal, project access review, resource counts, delegated
  principals, service principals, mappings, and safe unmapped signals are
  represented consistently.
- Service principals and mappings support create, edit, disable, and re-enable
  through the current API.
- Mutations refresh the complete access review and produce visible feedback.
- Missing permission, unavailable API, validation, malformed payload, and empty
  states are actionable.
- No raw token, authorization header, client secret, password, private key, or
  unrestricted claim payload is rendered or persisted by the admin route.
- The UI states clearly that current registry/mapping fields are review metadata
  until #176 delivers enforced grants.
- Unit, integration, compose smoke, and affected regression checks pass.

## Out Of Scope

- Organization/project-role and generalized service-principal grant
  enforcement (#176).
- Provisioning, deprovisioning, stale-access automation, and break-glass
  lifecycle (#177).
- BrowserPane-issued API keys or client secrets, immutable audit, and retention
  policy controls (#70).
- Central policy-engine behavior (#79).
- Deleting existing delegated sessions when a principal is disabled.
- Rendering raw OIDC tokens, unrestricted claims, credentials, or secret
  material.

## Delivered Evidence

- Added refresh-safe `/admin-new/identity` review and lifecycle management.
- Added strict identity API mappers/clients, independent view models, and
  independently tested route/components.
- Added `smoke:admin-unified-identity` with real Keycloak authentication,
  validation/conflict cases, project/principal selectors, full lifecycle,
  delegation correlation, deep-link refresh, redaction, responsive checks, and
  cleanup.
- Admin-new: 142 test files and 463 tests passed; check, build, and coverage
  passed at 91.47% statements, 76.32% branches, 93.29% functions, and 89.06%
  lines.
- Gateway identity/service-principal unit filters and both focused compose API
  tests passed.
- Compatibility admin identity tests/check/build, operator CLI smoke, and
  compatibility admin session smoke passed.
