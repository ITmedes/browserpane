# BrowserPane Implementation Plan Template

Copy this structure into `docs/BPANE-<five-digit-issue>_<TOPIC>_PLAN.md` only
when the canonical issue is moving to Ready or In Progress.

## Metadata

- Issue: `#NNN`
- State: Ready / In Progress / Review
- Owner:
- Lane:
- Target gate:
- Depends on:
- Last verified commit/date:

## Business Outcome

State the user/operator/business result, not the internal refactor.

## Example Use Case

Describe one concrete actor, workflow, expected outcome, and failure or policy
case.

## Current Evidence

List implemented code, API, UI, CLI, tests, migrations, and known gaps verified
against the current branch.

## Scope

Define the smallest coherent shippable result.

## Non-Goals

Name adjacent work that remains owned elsewhere.

## Decisions And Dependencies

Record accepted decisions, unresolved blockers, issue links, and why the
dependency order is correct.

## Contract Changes

- API/OpenAPI:
- Protocol/event schemas:
- Database/migrations:
- Admin-new:
- CLI/SDK:
- Deployment/configuration:
- README/ARCH/AGENTS/operator docs:

Use `N/A` with a reason instead of omitting a surface.

## Security And Data Impact

Cover authentication, authorization, secrets, URLs/logs, sensitive data,
retention, egress, files/artifacts, multi-tenant boundaries, and abuse limits.

## Migration, Compatibility, And Rollback

Define additive/breaking behavior, migration ordering, downgrade limitations,
feature flags, recovery, and the tested rollback or forward-fix path.

## Observability And Operator Feedback

Define metrics, logs, traces, events, health/readiness, alerts, admin messages,
and CLI/API error semantics. Apply redaction and cardinality rules.

## Implementation Slices

Each slice must be independently reviewable and leave the branch consistent.
Record intended commit boundaries when the work is larger than one commit.

## Test Strategy

### Unit

Include success, validation, boundary, and failure cases.

### Integration

Cover store/provider/runtime/API boundaries and in-memory/Postgres parity where
applicable.

### Smoke And E2E

Cover the real supported deployment, user workflow, policy denials, recovery,
and impacted admin-new/CLI surfaces.

### Coverage And Quality

Record baseline, changed-code coverage, exclusions, clippy/type/build/lint,
dependency/security scans, and why any gap is accepted.

## Manual Test Sequence

Provide reproducible setup, action, expected result, negative case, recovery,
and cleanup steps for the end user.

## Documentation And Claim Impact

Update capability maturity, roadmap/gate evidence, README/ARCH/OpenAPI, and the
investor claim register when the change affects externally visible claims.

## Definition Of Done

- Acceptance criteria complete.
- Required checks and relevant compose/E2E smokes pass.
- Negative and recovery paths pass.
- Migration/rollback evidence recorded.
- UI/CLI/API/docs parity complete or explicitly owned.
- Observability/operator feedback complete.
- Issue, plan, maturity, risk, and gate evidence updated.

## Post-Implementation Smoke Sequence

The canonical issue and this plan must contain the final runnable sequence.

## Evidence Record

Link the PR, commit, test output, screenshots/artifacts where relevant, reviewed
risks, and gate decision.
