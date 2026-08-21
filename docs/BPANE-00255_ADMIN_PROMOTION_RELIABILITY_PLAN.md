# BPANE-00255 Admin Promotion Reliability Plan

## Target

- Issue: [#255 Restore post-merge Compose admin-promotion reliability](https://github.com/ITmedes/browserpane/issues/255)
- Branch: `feature/BPANE-00255-admin-promotion-repair`
- Baseline: exact-main Compose run `32481154151` at merge commit `2c997a0b`

## Business case

The delivery loop must be able to trust exact-main promotion evidence after an
administrative merge. A stale OpenAPI expectation currently rejects valid API
growth, while a compatibility MCP smoke can connect an older selected session
instead of the session it just created. Both failures make a healthy product
change look unsafe and prevent the loop from advancing.

## Example use case

An endpoint slice adds reviewed workflow operations and the OpenAPI inventory
grows from 131 to 144 operations. During the following Compose promotion run,
the compatibility lane creates a new MCP session while older stopped sessions
remain in the catalog. The smoke must validate the 144-operation distribution,
identify the one session id absent before the create action, select that row,
and connect it without exceeding the intentional two-runtime limit.

## Evidence and root cause

1. The published classification artifact contains 117 `ui-primary`, 6
   `ui-evidence`, 9 `api-companion`, and 12 `internal-worker` operations: 144 in
   total. The published example catalog contains 22 examples. The API Companion
   smoke still repeats the previous totals of 131 operations and 19 examples.
2. The MCP smoke records only the previously selected id before creating a
   session. Event-driven selection can move to a different old row, which still
   satisfies the current `selected != previous` predicate.
3. The actual newly created session then occupies runtime capacity while the
   smoke requests access for the stale selected session, producing HTTP 409.
4. Compose logs show the preceding session-file runtime was already removed;
   runtime cleanup is therefore not the demonstrated defect.
5. Reusing one compatibility-admin document for two sequential WebTransport
   lifecycles can retain a closed client transport. The MCP smoke tests
   delegation, not reconnect behavior, so the second delegation should start
   from a freshly initialized admin document.
6. A freshly initialized sessions panel is visible before its catalog has
   necessarily loaded. The enabled create control is the existing readiness
   signal and must precede the pre-create catalog snapshot.

## Implementation steps

- [x] Update the reviewed API classification distribution and example count,
  and derive the operation total used by all evidence and UI assertions.
- [x] Introduce a deterministic new-session resolver with focused success,
  unchanged-catalog, malformed-entry, and ambiguous-catalog tests.
- [x] Use complete pre-create catalogs and explicit row selection in both MCP
  admin session creation paths.
- [x] Gate each pre-create catalog snapshot on the enabled session-create
  control and keep ambiguity errors bounded.
- [x] Reset the compatibility admin document between its two independent MCP
  delegation connections.
- [x] Run focused local validation and the affected Compose promotion smokes.
- [ ] Push a focused PR, merge after green checks, and verify exact-main
  Validation and Compose.

## Compatibility and rollout

- No API, protocol, persistence, SDK, deployment, or runtime configuration
  changes are required.
- The two-runtime Compose limit remains unchanged.
- Existing smoke CLI commands and output formats remain compatible.
- Rollback is a normal revert of the smoke-only commit; no data migration is
  involved.
- README and architecture claims are unaffected because product behavior does
  not change.

## Post-implementation smoke sequence

1. Run the new session-selection unit tests.
2. Run validation tooling tests and repository document checks.
3. Start Compose with the default two-runtime limit.
4. Run `smoke:admin-unified-api-companion` and confirm 144 reviewed operations.
5. Run the compatibility admin promotion sequence through MCP delegation in
   the same stack.
6. Confirm each MCP connection targets the session created by its own action
   and no runtime-capacity HTTP 409 occurs.
7. Run the affected pull-request Validation and Compose checks.
8. After merge, confirm exact-main Validation and Compose are green.
