# BPANE-00142 Admin App Redesign Requirements Workspace

This workspace splits the large local plan into focused implementation
references. Use it together with
`docs/BPANE-00142_ADMIN_APP_REDESIGN_PLAN.md`, which remains the detailed local
audit trail.

The `*_PLAN.md` file is local and ignored by git. This folder is the versioned
implementation source for requirements, gates, parity checks, and step-by-step
execution.

## Documents

- `01_CURRENT_ADMIN_PARITY.md`: current `/admin/` behavior, tests, selectors,
  and package/config items that must not regress.
- `02_CONCEPT_MAPPING.md`: how `concept.html` maps to the new app without
  copying mock-only or prototype-only parts.
- `03_API_COVERAGE.md`: OpenAPI, gateway route, compatibility endpoint, and
  API-client coverage requirements.
- `04_IMPLEMENTATION_STEPS.md`: implementation sequence with manual checkpoints.
- `05_TEST_AND_SMOKE_MATRIX.md`: unit, build, smoke, and regression validation
  matrix.
- `06_SELECTOR_CONTRACT.md`: selector stability and smoke migration contract.
- `07_PATTERN_LIBRARY.md`: reusable admin UI patterns to introduce while
  building the new app.
- `08_MANUAL_CHECKPOINTS.md`: step-by-step manual validation gates.

## Operating Rules

1. Build the new app beside the current `/admin/` app at `/admin-new/`.
2. Keep current `/admin/` behavior available until parity is proven.
3. Do not migrate a route unless its API coverage, current-app parity, selector
   coverage, and manual checkpoint are clear.
4. Commit implementation slices separately.
5. Keep the pattern library practical: extract reusable patterns only when they
   remove duplication or stabilize repeated operator workflows.

## First Implementation Slice

Start with Step 1 from `04_IMPLEMENTATION_STEPS.md`:

1. Scaffold `code/web/bpane-admin-unified`.
2. Mount it at `/admin-new/`.
3. Add Docker/nginx side-by-side serving.
4. Add the baseline shell and route isolation checks.
5. Keep `/admin/` unchanged.
