# BPANE-00159 Admin Resource Catalogs Plan

## Metadata

- Issue: `#159`
- State: Review
- Lane: Operator Product
- Target gate: Phase 1 Gate
- Depends on: merged `#153` shared admin-new patterns and merged `#158` API companion
- Excludes: session-template catalog work owned by `#124`
- Last verified: `feature/BPANE-00159` at `43f1f536ed596766a5b4b2e50d0c937102d402c1`, 2026-08-07

## Business Outcome

Operators can provision and inspect the remaining backend-backed integration
resources from `/admin-new/`: approved browser extensions, credential bindings,
and signed workflow event subscriptions. The console must expose only lifecycle
actions supported by the frozen control API and must keep credentials and
webhook signing secrets write-only.

## Example Use Case

An operator registers an approved unpacked browser extension and publishes its
installed version, provisions a project-scoped Vault credential binding for a
workflow login, and creates a signed event subscription for the workflow
orchestrator. The operator can later disable the extension, confirm which safe
credential metadata was retained, and inspect webhook delivery failures without
recovering either secret from the UI.

## Current Evidence

- The frozen OpenAPI contract exposes 15 owner-authenticated operations across
  these resource families.
- Extensions support list/create/get, publish-version, enable, and disable. The
  API does not expose update, delete, or version-list operations.
- Credential bindings support list/create/get. Secret payloads are accepted only
  at creation; responses expose provider metadata and an opaque external
  reference, never the secret payload. The API does not expose update/delete.
- Workflow event subscriptions support list/create/get/delete and delivery-list.
  Responses expose `has_signing_secret`, not the signing secret.
- Existing admin-new resource clients use `AuthenticatedApiClient`, strict
  response decoding, route-owned async state, and independent view models.
- Existing workflow extension, credential, credential-injection, and event
  delivery smokes cover backend behavior but not these dedicated admin-new
  catalogs.

## Scope

- Add dedicated catalog, create, and detail routes for approved extensions.
- Add extension version publishing and enable/disable controls to extension
  detail while clearly documenting that installed paths are deployment-managed.
- Add dedicated catalog, create, and read-only detail routes for credential
  bindings, including owner/project scope selection and supported injection
  modes.
- Accept a secret JSON payload or an existing opaque Vault reference on
  credential creation, clear write-only values after submission, and never
  render them from responses.
- Add dedicated catalog, create, and detail routes for workflow event
  subscriptions, including deletion and persisted delivery-attempt health.
- Add navigation, shell routing, unit/component coverage, and a compose-backed
  admin-new smoke covering supported lifecycle and redaction behavior.
- Update current-state and delivery-order documentation after validation.

## Non-Goals

- Session templates remain owned by `#124`.
- No API methods, edit controls, or destructive controls absent from the frozen
  contract will be invented.
- Extension binary upload/package distribution and extension version history are
  not supported by the current API.
- Credential rotation, revocation, provider browsing, and raw secret retrieval
  require future API contracts.
- Event subscription update, replay, pause/resume, generalized resource events,
  and security-event export remain outside this slice.
- Operation-counter catalogs remain separate from these resource lifecycles.

## Decisions And Dependencies

- Use separate routes so each resource remains linkable and independently
  testable: `/extensions`, `/credential-bindings`, and
  `/workflow-event-subscriptions` below `/admin-new`.
- Place approved extensions and credential bindings in `Resources`; place
  workflow event subscriptions in `Govern` because they are outbound operational
  integration policy and delivery-health surfaces.
- Reuse established catalog table, create route, inspector, action feedback, and
  strict client-decoder patterns. Do not introduce a generic abstraction until
  the three concrete models prove meaningful shared behavior.
- Treat `external_ref`, `namespace`, target URLs, event payloads, and delivery
  errors as potentially sensitive operational metadata. Render deliberately and
  never log form secrets.

## Contract Changes

- API/OpenAPI: N/A; consume the frozen operations without changing them.
- Protocol/event schemas: N/A; no transport protocol change.
- Database/migrations: N/A; use existing persisted resources.
- Admin-new: add navigation, routes, catalogs, creation forms, details, and
  supported lifecycle actions for all three families.
- CLI/SDK: N/A; no CLI contract is changed by this admin parity slice.
- Deployment/configuration: N/A; credential smoke requires the existing local
  Vault configuration and event smoke uses an existing safe local receiver.
- README/ARCH/AGENTS/operator docs: update only where user-visible catalog
  availability or current implementation status changes.

## Security And Data Impact

- Every request uses the current owner bearer token and the shared global auth
  failure handler.
- Credential `secret_payload` and subscription `signing_secret` are write-only
  form state. They are not stored outside the active form, copied into URLs,
  logged, or rendered after creation.
- Credential responses may show the opaque `external_ref`; the UI labels it as a
  provider reference and never resolves it.
- Project-scoped credential choices are limited to existing active project
  options; backend project ownership validation remains authoritative.
- Delivery payloads and errors are troubleshooting evidence. Present them only
  on the selected subscription detail route and avoid adding them to global
  search or navigation state.
- Subscription target validation and SSRF controls remain gateway-owned. The UI
  provides local validation and faithfully displays server rejection.

## Migration, Compatibility, And Rollback

- The change is additive and leaves `/admin/`, existing routes, APIs, database
  state, and worker behavior unchanged.
- Rollback removes the new admin-new routes and navigation entries; created
  resources remain valid control-plane resources.
- Because credential bindings have no delete endpoint, automated and manual
  tests must use uniquely named disposable bindings and report retained fixture
  metadata explicitly.

## Observability And Operator Feedback

- Every load and mutation exposes loading, success, and error feedback near the
  initiating control.
- Subscription detail summarizes delivery states and exposes attempt count,
  response status, retry timing, last error, and linked run/event identifiers.
- Extension detail exposes current enablement and latest-version evidence.
- Credential detail states that values are write-only and shows only safe
  metadata returned by the API.
- Client decoding failures remain distinguishable from HTTP/authentication
  failures through the existing admin request error model.

## Implementation Slices

1. **Plan and route foundation (complete)**: create this plan, align issue context, add
   navigation/shell route definitions and tests. Commit boundary: docs and route
   foundation.
2. **Approved extensions (complete)**: types, strict client, view models, catalog/create/
   detail components, version publishing, enable/disable, and focused tests.
   Commit boundary: complete extension catalog.
3. **Credential bindings (complete)**: types, strict client, project options, safe creation
   form, read-only detail, and focused tests. Commit boundary: complete
   credential-binding catalog.
4. **Workflow event subscriptions (complete)**: types, strict client, catalog/create/detail,
   delete, delivery health, and focused tests. Commit boundary: complete event
   subscription catalog.
5. **Battle test and documentation (complete locally; CI review pending)**: compose-backed admin smoke, existing
   extension/credential/event regressions, type/build/coverage, current-state and
   roadmap updates. Commit boundary: validation and status evidence.

## Test Strategy

### Unit

- Strict response decoders reject malformed lists, resources, enum values, and
  delivery attempts.
- View models cover metrics, filters, search, scope/status tones, delivery-health
  summaries, dates, labels, and empty/error states.
- Form models cover required fields, JSON secret validation, URL/event-type
  validation, TOTP fields, labels, and mutually exclusive secret/reference
  input.

### Integration

- Route/component tests verify client calls, auth-failure propagation,
  navigation, action feedback, state refresh, and redirect after creation.
- Credential tests prove submitted secrets do not reappear in rendered detail
  or client resources.
- Event tests cover delivered, pending/retrying, and failed delivery evidence as
  well as deletion failure.
- Extension tests cover version publication and both enablement transitions.

### Smoke And E2E

- Add an authenticated compose smoke that creates and inspects all three
  resource types through admin-new, exercises extension state/version and event
  deletion, and verifies no submitted secret appears in the DOM.
- Use Vault-backed credential creation and a safe local event receiver.
- Run existing workflow extension, credential, credential-injection, workflow
  events, project, egress, workflow, and API companion regressions.

### Coverage And Quality

- Run admin-new unit tests and coverage gate, Svelte check, TypeScript check,
  lint, and production build.
- Run the browser-client smoke registrations and targeted compose smokes.
- Run gateway tests if implementation evidence exposes a backend contract defect;
  otherwise this slice remains a contract-consuming frontend change.

## Manual Test Sequence

1. Start local compose with Keycloak, Postgres, Vault, gateway, web, and workers.
2. Sign into `/admin-new/` as `demo / demo-demo`.
3. Open Extensions, register a reference, publish an absolute installed-path
   version, disable it, then re-enable it.
4. Open Credential bindings, create one using a disposable secret JSON payload,
   and verify detail/list pages show only safe metadata and an opaque reference.
5. Submit an invalid credential payload and verify field/server feedback remains
   visible without losing unrelated form state.
6. Open Workflow event subscriptions, create a signed subscription for the safe
   local receiver, run a workflow, and inspect delivery status and attempts.
7. Delete the subscription and confirm it disappears from the catalog.
8. Refresh every new route directly and confirm authentication and state restore.
9. Verify browser developer tools and rendered HTML contain neither submitted
   credential payload nor signing secret.

## Documentation And Claim Impact

- Mark #159 implemented on the feature branch; merge only after the final
  Compose and required PR checks pass.
- Update `ADMIN_NEW_STATUS.md`, `DELIVERY_ROADMAP.md`,
  `OPEN_ISSUES_CONTEXT.md`, and manual checkpoint commands.
- Update `README.md` only if its admin-new capability summary names these
  catalogs; update `ARCH.md` only if ownership/topology claims change.

## Definition Of Done

- All three resource families are discoverable from navigation and have direct,
  refresh-safe routes.
- Every API-supported action is available and no unsupported action is implied.
- Secrets are write-only and absent from post-create UI state and smoke evidence.
- Delivery health is sufficient to diagnose pending, delivered, and failed
  webhook attempts.
- Unit, component, integration, compose smoke, regression, coverage, type,
  lint, and build checks pass.
- Issue, roadmap, implementation status, and smoke documentation are aligned.

## Post-Implementation Smoke Sequence

1. Run admin-new catalog/client/view-model/form/route tests and coverage.
2. Run Svelte/type/lint/build validation for `bpane-admin-unified`.
3. Start compose with Vault and the safe local workflow-event receiver.
4. Run the new authenticated admin-new resource-catalog smoke.
5. Run existing workflow extension, workflow credential,
   workflow credential-injection, workflow events, unified projects, unified
   egress, unified workflows, and API companion smokes.
6. Confirm fixture cleanup succeeds except for the intentionally retained,
   uniquely named credential binding required by the current no-delete API.
7. Recheck the DOM and smoke logs for submitted credential and signing-secret
   values.

## Evidence Record

- PR: `#201`
- Commits: `419d4b1` through `43f1f53`
- Unit/integration results: 185 admin-new test files and 572 tests passed;
  focused extension, credential-binding, event-subscription, delivery-health,
  and MCP convergence tests passed.
- Compose smoke results: the new
  `smoke:admin-unified-resource-catalogs` passed locally, as did projects,
  egress, workflows, workflow events, workflow extension, workflow
  credentials, credential injection, API companion, and session/MCP
  regressions. GitHub Compose run `31199532428` passed all three jobs: gateway
  API docker-pool, gateway API default, and browser/integration smokes.
- Coverage/build results: admin-new coverage passed with 91.96% statements,
  76.46% branches, 93.87% functions, and 90.06% lines; Svelte check and the
  production build passed.
- README decision: updated because the unified-admin capability summary names
  the resource catalogs.
- ARCH decision: no update required; this slice consumes existing control-plane
  contracts and does not change subsystem ownership or runtime topology.
