# BPANE-00287 Compose Qualification Reuse Plan

## Metadata

- Issue: `#287`
- State: Qualified
- Owner: BrowserPane maintainers
- Lane: Foundation
- Target gate: trustworthy post-merge Compose qualification reuse
- Depends on: `#286` and transitively `#283`-`#285`; completed `#273` and `#277`
- Last verified commit/date: `e79164cc3a84` / 2026-08-22

## Business Outcome

BrowserPane avoids repeating expensive qualification after merge when the exact
tree and every relevant test input were already qualified, while any ambiguity
falls back to normal qualification. Main still receives an immediate bounded
canary and independent scheduled/release coverage.

## Example Use Case

A squash merge produces the same git tree qualified on the pull request with
test-plan revision `P` and immutable image manifest `I`. Main verifies trusted,
fresh evidence for that exact input set and runs the bounded canary. A merge
commit or changed workflow revision does not match and runs the normal affected
or full plan.

## Current Evidence

- #273 requires full exact-head Compose evidence while the PR is repairable.
- #277 can rerun failed jobs once on the exact merge SHA, but successful PR
  evidence is not reusable after merge.
- Pushes to `main` currently start the complete Compose matrix even when the
  resulting tree equals the qualified pull-request tree.
- #283-#286 provide structured identity, deterministic state, immutable image
  inputs, and a fail-closed fallback plan.

## Scope

- Define a versioned qualification manifest keyed by git tree, workflow/test-plan
  digest, selector revision, image digests, required lane outcomes, provenance,
  and bounded freshness.
- Publish trusted reusable evidence from qualifying PR workflows.
- Verify evidence from the main workflow without executing untrusted code first.
- Run a bounded main canary after accepted reuse.
- Fall back to #286 qualification for every missing, stale, incomplete,
  mismatched, unsupported, or untrusted case.
- Preserve scheduled, release, and manual full qualification.

## Non-Goals

- No commit-SHA-only, branch-name-only, cache-key-only, or mutable-tag reuse.
- No reuse across changed workflows, selectors, dependencies, images, or plans.
- No bypass of required review, branch protection, scheduled evidence, or
  deterministic failure.
- No indefinite evidence retention.

## Decisions And Dependencies

- Git tree identity is necessary but not sufficient; every executable test input
  and image identity is part of the reuse key.
- Verification is fail closed and occurs in a trusted workflow context.
- Accepted evidence still requires a bounded main canary for deployment/event
  integration and immediate publication feedback.
- Existing #277 retry semantics apply only to a workflow actually dispatched;
  reuse does not reinterpret a prior failure as success.
- #286 remains the sole fallback selector and full/manual backstop.

## Contract Changes

- API/OpenAPI: N/A.
- Protocol/event schemas: N/A; add an internal versioned qualification manifest.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: N/A.
- Deployment/configuration: trusted PR artifact publication and main verifier/
  canary workflow wiring.
- README/ARCH/AGENTS/operator docs: update contributor validation behavior;
  product architecture and support claims remain unchanged.

## Security And Data Impact

- Evidence includes only bounded build/test identity and outcome data under #283
  redaction rules.
- Trusted verification checks artifact provenance, repository, workflow identity,
  tree, digests, completeness, and freshness.
- Untrusted forks cannot publish evidence into the accepted namespace or gain
  privileged registry/artifact access.
- Tampering, parser failure, unsupported versions, and unavailable provenance
  trigger normal qualification.

## Migration, Compatibility, And Rollback

- Begin in audit-only mode: verify potential reuse but still run qualification.
- Compare verifier decisions with full results before enabling reuse.
- Enable only for exact squash/rebase tree matches with complete evidence.
- A workflow input disables reuse globally or for one dispatch.
- Rollback disables acceptance and restores #286 qualification for all pushes;
  scheduled/manual full runs are never changed.

## Observability And Operator Feedback

- Report source PR/run, tree, test-plan/selector/image identity, age, verification
  decision, rejection reason, fallback tier, and canary result.
- Use fixed rejection categories and no source contents or credentials.
- A stable required check distinguishes reused-plus-canary from newly qualified.

## Implementation Slices

1. Freeze manifest, provenance, freshness, and exact-input identity contracts.
2. Publish trusted PR qualification manifests from #283/#285 evidence.
3. Add pure verifier and exhaustive mismatch/tamper fixtures.
4. Integrate audit-only main decision plus #286 fallback and bounded canary.
5. Enable reuse after comparison evidence; retain force-full and scheduled paths.

## Test Strategy

### Unit

- Exact identity, schema versions, freshness, completeness, provenance,
  tampering, tree mismatch, plan/selector/image drift, and fallback decisions.

### Integration

- Squash/rebase exact-tree success; merge-commit mismatch; missing/expired
  artifacts; workflow change; dependency or image drift; fork provenance;
  unavailable GitHub evidence; forced full mode.

### Smoke And E2E

- One trusted exact-tree reuse executes only verifier plus main canary.
- Every negative fixture dispatches and passes the normal #286 plan.
- Scheduled/manual full qualification remains complete.

### Coverage And Quality

- Require complete decision-branch coverage for the verifier.
- Use immutable action revisions, least privilege, bounded artifacts, and
  repository workflow-policy checks.
- Compare audit decisions with full outcomes before enforcement.

## Manual Test Sequence

1. Qualify a PR and inspect its manifest/provenance.
2. Produce an exact-tree squash merge and verify accepted reuse plus canary.
3. Produce a merge-commit mismatch and verify normal qualification.
4. Change the workflow/test-plan revision and verify fallback.
5. Expire or tamper with a fixture manifest and verify fallback.
6. Dispatch manual full qualification and verify all lanes still run.

## Documentation And Claim Impact

Update `VALIDATION_MATRIX.md`, delivery context, and contributor workflow docs.
This reduces duplicate CI work; it is not product evidence or a relaxation of
the Foundation/Production gates.

## Definition Of Done

- Reuse accepts only exact, trusted, complete, fresh input identity.
- Every verification problem falls back to #286 qualification.
- Accepted reuse always runs and passes the bounded main canary.
- Required checks remain stable and branch protection remains fail closed.
- Scheduled/release/manual full qualification remains unchanged.
- Audit comparison and hosted positive/negative evidence are linked in #287.

## Post-Implementation Smoke Sequence

1. Run manifest/verifier unit and workflow-contract tests.
2. Exercise one exact-tree trusted reuse and bounded canary.
3. Exercise merge, workflow, selector, image, expiry, and tamper mismatches.
4. Verify every mismatch dispatches the expected #286 plan.
5. Force full qualification manually and verify all lanes.
6. Review required-check, provenance, redaction, and timing evidence.

## Evidence Record

Record the PR, commit, audit comparison, accepted reuse run, canary run, every
fallback fixture, force-full run, security review, and timing result in #287.
