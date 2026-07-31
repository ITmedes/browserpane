# BPANE-00173 Delivery Governance Plan

Issue: [#173](https://github.com/ITmedes/browserpane/issues/173)

Status: In progress

Target gate: Delivery governance baseline

Last reviewed: 2026-07-31

## Purpose

Turn the current implementation inventory, open issues, product requirements,
and management claims into one executable delivery system. The result must let
a contributor identify the next shippable slice, understand its dependencies,
follow a bounded implementation plan, and provide evidence for a named release
gate without reconstructing status from several long documents.

## Business Case

BrowserPane already has substantial implementation and planning coverage, but
the execution state is spread across issue bodies and overlapping roadmap,
audit, requirements, and investor documents. That makes it difficult to answer
four operational questions consistently:

1. What works now and at which maturity level?
2. What is the next executable slice?
3. Which dependencies and risks prevent a capability claim?
4. What evidence promotes the result into a Pilot, Production, or Phase N gate?

This slice creates those answers without changing product runtime behavior.

## Example Use Case

A contributor begins the next implementation session. They open
`DELIVERY_ROADMAP.md`, find issue #151 as the first Ready product slice, confirm
that it targets the Foundation gate, and open its bounded `*_PLAN.md`. The plan
identifies the dependency remediation, CI stages, failure fixtures, coverage
baseline, documentation impact, and smoke evidence required for Done.
A stakeholder can separately use the maturity matrix to see that workflow
execution exists as a Prototype while the stable BPM Workflow Endpoint remains
a Planned capability under #172.

## Current State

- 47 issues were open before this governance slice, with no milestones,
  assignees, or usable priority/status taxonomy.
- The existing work order contains a valuable dependency and rationale
  inventory, but its numbered list mixes security, Pilot value, admin-new
  promotion, production hardening, enterprise controls, and research.
- Issue-specific plans exist for #171 and #172, but they are feature-level
  specifications rather than bounded single-PR execution plans.
- Management material distinguishes current evidence in several places, but
  it has no maintained claim-to-evidence register.
- CI is not enforced and branch protection has no required status checks.

## Scope

### Slice 1: Canonical Delivery Documents

- Add `DELIVERY_ROADMAP.md` as the operational source of delivery order.
- Add `CAPABILITY_MATURITY_MATRIX.md` as the source of capability status and
  evidence.
- Add `PRODUCT_PHASES_AND_RELEASE_GATES.md` for Foundation, Phase 0, Phase 1,
  Production Baseline, and Phase N promotion criteria.
- Add `RISK_REGISTER.md` for active product and delivery risks.
- Add `PLAN_TEMPLATE.md` with Definition of Ready and Definition of Done.

### Slice 2: Focused Ownership

- Add focused issues for Phase 0 delivery, protocol conformance, authorization,
  identity lifecycle, platform observability, API compatibility, and
  open-source governance.
- Cross-reference broad existing owners instead of duplicating their scope.
- Record remaining scope from closed #52 under active focused issues.

### Slice 3: Existing Plan Reconciliation

- Make the delivery roadmap authoritative for current sequencing.
- Keep the long implementation work order as a rationale and dependency
  catalog.
- Add maturity and gate context to the #171 and #172 specifications.
- Align the issue map, validation matrix, identity requirements, and docs index.

### Slice 4: Management Claim Governance

- Add a claim-to-evidence register to the investment repository.
- Classify claims as Current Evidence, Pilot Target, Roadmap, or Hypothesis.
- Link Phase 0, Workflow Endpoint, Teach Mode, protocol, and production claims
  to their canonical issues.
- Capture publication checks without coupling product delivery to presentation
  rendering.

## Non-Goals

- Implement product runtime behavior.
- Produce detailed plans for every Backlog issue.
- Claim that a passing local test suite constitutes Production Readiness.
- Replace GitHub issue execution state with duplicated prose.
- Resolve legal licensing choices without an explicit reviewed decision.

## Definition Of Ready For Future Slices

A product issue is Ready only when it has:

- one accountable owner or owner role,
- a business outcome and example use case,
- bounded scope and explicit non-goals,
- acceptance criteria and a post-implementation smoke sequence,
- resolved or explicitly accepted dependencies,
- security, data, migration, rollback, observability, and compatibility impact,
- API, admin-new, CLI, documentation, and deployment applicability,
- a dedicated `docs/*_PLAN.md` for the selected shippable slice.

## Definition Of Done For Future Slices

Done requires:

- acceptance criteria implemented,
- unit and integration coverage for changed contracts,
- relevant compose smoke/E2E evidence,
- negative and recovery-path validation,
- no unexplained coverage regression,
- migration and rollback/recovery evidence where applicable,
- README, ARCH, OpenAPI, AGENTS, and operator docs updated or marked N/A,
- admin-new and CLI parity implemented or explicitly deferred to an owner,
- observability and safe operator feedback for new state transitions,
- issue, plan, maturity matrix, and target-gate evidence updated.

## Validation

- Validate every new local Markdown link and GitHub issue reference.
- Confirm no closed issue is named as owner of unfinished scope.
- Confirm the roadmap has exactly one first Ready implementation slice.
- Confirm all management claims in the register have a maturity and evidence or
  issue owner.
- Confirm no product, generated, certificate, or unrelated user file changed.

## Post-Implementation Smoke Sequence

1. Open `docs/DELIVERY_ROADMAP.md` and identify the next Ready issue.
2. Trace that issue through dependencies, maturity, risk, and target gate.
3. Use `docs/PLAN_TEMPLATE.md` to verify the issue can produce a bounded plan.
4. Trace runtime, admin-new, Workflow Endpoint, Teach Mode, identity, protocol,
   and production claims through the maturity matrix.
5. Verify every new focused issue includes business case, example use case,
   scope, non-goals, acceptance criteria, and smoke sequence.
6. Verify the investor claim register distinguishes current evidence, Pilot
   target, roadmap, and hypothesis.
7. Check both worktrees and confirm only intended documentation changes exist.

## Exit Criteria

- The next implementation session can start from one unambiguous Ready issue.
- Phase 0 value work and production hardening have separate but connected gates.
- Every material audit gap has a canonical issue or an explicit existing owner.
- Management claims can be reviewed without reading implementation internals.
- Historical audit documents remain available without acting as competing
  execution roadmaps.
