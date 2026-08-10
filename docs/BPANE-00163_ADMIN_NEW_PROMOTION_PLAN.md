# BPANE-00163 Admin-New Promotion And Fallback Plan

## Metadata

- Issue: [#163](https://github.com/ITmedes/browserpane/issues/163)
- State: In progress
- Lane: Operator Product
- Target gate: Admin-New Phase 1 Promotion
- Depends on: admin-new route and resource parity through `#153`-`#162`,
  control API conformance through `#179`, and the shared admin security baseline
  through `#146`
- Branch: `feature/BPANE-00163-admin-new-promotion-routing`
- Baseline: `main` at `b1428e8a89566478f02e940c39a55a6c43df0e2c`,
  2026-08-10

## Business Outcome

BrowserPane has one clear default operator entry point without removing the
known compatibility path prematurely. The repository can prove that every
advertised admin-new navigation target exists, the mandatory route journeys
remain covered, authentication and deep-link refresh work, and the legacy
`/admin/` console remains directly reachable if a regression is found.

Promotion in this slice means that the web root redirects to `/admin-new/`.
It does not rename either application, remove `/admin/`, or claim that every
future operator feature is complete.

## Example Use Case

An operator opens `http://localhost:8080/` after upgrading BrowserPane and is
sent to the unified admin console. They authenticate, refresh a session detail
deep link, create and inspect resources, and connect to a browser session. If a
route-specific regression blocks work, they can open `/admin/` directly and
continue the compatibility workflow while the unified app is repaired. A
release reviewer can rerun the promotion stages by stable validation IDs and
see exactly which new-admin and fallback journeys passed.

## Current Evidence And Gaps

- `/admin-new/` exposes 16 visible navigation targets and each currently maps
  to a route-backed SvelteKit page.
- `/admin/` and `/admin-new/` are built and served side by side with the same
  shared OIDC and browser-security baseline.
- All twelve `smoke:admin-unified-*` scripts are represented by independently
  rerunnable canonical Compose stages.
- The executable promotion contract proves visible navigation route coverage,
  the promoted root, and the directly reachable compatibility route.
- Root `/` redirects to `/admin-new/`; `/admin/` and explicit development
  fixtures remain directly reachable.
- Session-template catalog management (`#124`), command palette behavior, and
  operation-counter presentation are not advertised navigation routes. They
  remain explicit post-promotion gaps rather than hidden parity claims.

## Scope

- Add an executable promotion contract covering visible navigation routes,
  default and fallback paths, and the mandatory validation inventory.
- Promote every existing unified-admin smoke to an independently rerunnable
  canonical Compose stage and the hosted browser-integration lane.
- Add a browser smoke that verifies root redirect, unauthenticated login
  recovery, admin-new deep-link refresh, logout/login behavior, and direct
  `/admin/` fallback availability.
- Change the web root default from the development fixture to `/admin-new/`
  while preserving `/admin/`, `/admin-new/`, and explicit development fixture
  URLs.
- Update local setup, architecture, status, validation, and fallback guidance
  so docs match the promoted route behavior.
- Run the focused and full automated gate, then record the manual checkpoint
  result and promotion decision in the issue and PR.

## Explicit Deferrals

- Session-template catalog CRUD remains owned by `#124`; template selection in
  session creation and complete API/CLI management remain available.
- The command palette is an operator-efficiency enhancement, not a navigation,
  authentication, lifecycle, or fallback requirement.
- Dedicated operation-counter presentation remains deferred; current status,
  recordings, workflow runs, observability, API coverage, and CLI evidence stay
  available through their existing routes.
- Removing `/admin/` or reusing `/admin/` for the unified build requires a
  separate removal gate after an accepted fallback observation period.
- Phase N workflow endpoints, Teach Mode, enterprise grants, and production
  deployment claims remain outside this promotion slice.

## Contract Decisions

- `/admin-new/` remains the canonical unified-app base path so saved deep links
  and existing smoke paths do not change.
- `/admin/` remains the compatibility app and must be independently loadable;
  promotion changes only the root entry point and documentation preference.
- Visible route coverage is derived from the checked-in navigation definition
  and filesystem routes, not maintained as an unverified prose list.
- Each mandatory smoke has a stable validation stage ID so failures can be
  rerun without executing the entire Compose profile.
- Promotion evidence distinguishes automated checks from manual checkpoints;
  automated success alone does not claim that every optional route was
  manually accepted.

## Implementation Slices

1. **Executable promotion contract (complete through `ac54d8d`):** validate
   navigation-to-route coverage, define the mandatory unified and compatibility
   smoke inventory, and protect the inventory with focused tests.
2. **Canonical browser regression stages (complete through PR `#210`):** add
   the missing unified-admin Compose stages, add the promotion/fallback browser
   journey, and require all stages in isolated hosted promotion lanes.
3. **Default-route promotion (complete through `e166325` and `5d785ec`):**
   redirect root `/` to `/admin-new/`, preserve
   explicit fixture and legacy routes, and cover Nginx behavior with static and
   live tests.
4. **Documentation and automated decision evidence (complete through
   `3497382`):** align README, ARCH, AGENTS, status, manual checkpoints,
   validation matrix, delivery roadmap, and issue state; run the full gate and
   record residual risks and rollback steps. The final human checkpoint remains
   explicitly separate from the automated gate.

Each completed slice is committed independently. No slice may remove or alias
the compatibility application.

## Detailed Implementation Steps

### Slice 1: Executable Promotion Contract

1. Introduce a small validation module that owns the promotion route and stage
   contract without duplicating application state or API behavior.
2. Load the admin-new navigation definition in its native package tests and
   verify every visible route resolves to a SvelteKit page or documented alias.
3. Assert unique navigation IDs/routes and reject a visible route that has no
   implementation.
4. Define the mandatory admin-new and compatibility stage IDs in one reusable
   validation contract.
5. Extend validation-tool tests so a missing, duplicate, or unknown promotion
   stage fails before Compose starts.

### Slice 2: Canonical Browser Regression Stages

1. Add canonical Compose stages for browser contexts, egress profiles, file
   workspaces, recordings, workflows, workflow runs, and identity.
2. Keep resource-catalog and API-companion stages separate because they cover
   distinct API families and cleanup behavior.
3. Add one bounded promotion smoke for root selection, auth recovery, deep-link
   refresh, logout/login, and direct compatibility fallback.
4. Add all promotion stages to `.github/workflows/compose.yml` and require the
   workflow contract test to compare the exact canonical inventory.
5. Keep each stage independently selectable through `scripts/validate.mjs`.

### Slice 3: Default Route And Fallback

1. Change Nginx root `/` to a temporary redirect to `/admin-new/`; do not cache
   the redirect.
2. Keep `/admin`, `/admin/`, `/admin-new`, and `/admin-new/` behavior explicit
   and protected by the existing security headers.
3. Keep development fixtures reachable only by their explicit filenames.
4. Add static Nginx contract assertions and verify live responses through the
   promotion smoke.
5. Document rollback as restoring the root redirect target to `/admin/`; no
   database, API, or application artifact migration is involved.

### Slice 4: Evidence And Release Decision

1. Update README local entry points and compatibility wording.
2. Update ARCH and AGENTS route/topology facts.
3. Update `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`,
   `VALIDATION_MATRIX.md`, `DELIVERY_ROADMAP.md`, and
   `OPEN_ISSUES_CONTEXT.md` with tested evidence and explicit deferrals.
4. Run fast validation, every promotion Compose stage, selected legacy smokes,
   and the documented manual checkpoint sequence.
5. Publish exact command/results evidence to `#163` and the PR. Close `#163`
   only after merge; retain `/admin/` until a separate removal decision.

## Post-Implementation Smoke Sequence

1. Start a fresh local Compose stack and wait for `/healthz` and `/readyz`.
2. Open `http://localhost:8080/` and verify it redirects to `/admin-new/`.
3. Complete Keycloak login, refresh `/admin-new/sessions`, then log out and log
   in again without stale callback or route state.
4. Run every canonical `compose-admin-new-*` validation stage and confirm all
   visible navigation routes load or complete their focused journey.
5. Run the compatibility auth/session, browser-context, egress, realtime,
   recording, MCP, workflow, and workflow-detail regressions selected by the
   migration matrix.
6. Open `/admin/` directly, create/select a disposable session, and verify the
   compatibility console still connects and disconnects correctly.
7. Switch between both apps and verify selected-session, authentication, and
   live-browser state do not leak across application storage namespaces.
8. Execute the final regression sequence in
   `docs/ADMIN_NEW_MANUAL_CHECKPOINTS.md` and record pass, accepted deferral, or
   blocker for every item.
9. Verify rollback by changing only the root redirect target in a disposable
   local build; confirm no API or persisted-resource migration is required.

## Required Automated Validation

- `node --test scripts/validation/*.test.mjs scripts/ci/*.test.mjs`
- `node scripts/validate.mjs --profile fast`
- `node scripts/validate.mjs --profile compose` for the full canonical gate
- Focused `compose-admin-new-*` stages while implementing each route group
- Selected legacy admin stages and focused package tests/builds
- Repository document and Markdown link checks

## Implementation Evidence

### 2026-08-10: Promotion Contract And Hosted Lane Foundation

- All 16 visible navigation entries resolve to route-backed SvelteKit pages.
- The promotion contract includes every current `smoke:admin-unified-*` package
  script and 11 selected compatibility journeys.
- The canonical Compose catalog exposes 24 independently rerunnable admin
  promotion stages.
- Hosted validation runs unified and compatibility promotion surfaces in two
  parallel jobs instead of extending the existing browser-integration critical
  path.
- Focused admin navigation tests passed: 7 tests.
- Promotion contract, stage catalog, and Compose workflow tests passed: 15
  tests.
- The complete validation-tool stage passed: 89 tests.
- Repository document validation passed: 68 Markdown files, 8 YAML files, and
  3 workflows.
- Root routing and README behavior remain unchanged until the dedicated live
  promotion/fallback smoke is implemented.

### 2026-08-10: Default Route And Live Fallback Evidence

- Exact root `/` returns a non-cacheable `302` to same-origin `/admin-new/` and
  carries the shared admin response-security policy.
- `/admin/` remains directly reachable and explicit fixture URLs such as
  `/index.html` remain served.
- The live promotion smoke completed Keycloak authentication at the promoted
  root, loaded and reloaded `/admin-new/sessions`, opened the compatibility
  console, and returned to the unified dashboard.
- `compose-admin-new-promotion` passed against a rebuilt local web image in
  3.1 seconds.
- Browser-client typecheck and all 674 unit tests passed before live execution;
  20 focused promotion, workflow, stage-catalog, and Nginx contract tests also
  passed.
- README, ARCH, AGENTS, admin status/manual checkpoints, validation matrix,
  delivery roadmap, and issue context now describe `/admin-new/` as the local
  default while retaining `/admin/` as fallback.
- Default routing does not authorize legacy app removal; the final manual
  regression record remains a separate promotion checkpoint.

### 2026-08-10: Full Promotion Inventory

- `node scripts/run-admin-promotion-validation.mjs all` passed all 24 stages in
  one uninterrupted run against the rebuilt local Compose stack in 16 minutes
  47 seconds.
- The run covered all 13 unified stages and 11 compatibility regressions,
  including authentication recovery, resource CRUD, session lifecycle and
  reconnect, event-stream recovery, egress, session files, MCP delegation,
  browser-local recording, retained recording export, workflow execution, and
  workflow-run detail evidence.
- The compatibility recording stage passed at its normal late-suite position
  and produced a 244,646-byte WebM. The smoke now gives the browser encoder a
  2.5-second capture window while retaining strict non-empty artifact, exact
  retained-byte, realtime-refresh, and playback-ZIP assertions.
- Every automated promotion stage has passed both individually where needed and
  as part of the complete ordered inventory. No automated promotion blocker
  remains.

## Completion Criteria

- Root `/` selects `/admin-new/` and `/admin/` remains directly usable.
- Every visible navigation route is contract-tested against a real page.
- Every mandatory unified-admin smoke and selected legacy regression is part of
  the canonical validation inventory and hosted Compose lane.
- Authentication, deep-link refresh, logout/login, and fallback routing have
  live browser evidence.
- Manual checkpoint results, explicit deferrals, rollback instructions, and
  residual risks are recorded in the PR.
- README, ARCH, AGENTS, status, validation, roadmap, and issue context match the
  promoted behavior.
