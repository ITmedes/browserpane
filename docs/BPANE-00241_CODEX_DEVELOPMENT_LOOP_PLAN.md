# BPANE-00241 Codex Development Loop Plan

## Metadata

- Issue: `#241`
- State: Done through PR #243; requirements-specification follow-up is `#246`
- Owner: BrowserPane maintainers / Codex
- Lane: Foundation
- Target gate: Repeatable, bounded contributor delivery
- Depends on: `#173` delivery governance (complete), a live `state:ready`
  issue or one roadmap-prioritized `state:qualified` candidate, and a matching
  focused implementation plan
- Last verified commit/date: `8618d83c` / 2026-08-20

## Business Outcome

BrowserPane maintainers can move implementation work out of one long-lived
conversation and into short, auditable Codex sessions without losing the
project's issue, planning, validation, review, and documentation discipline.
The local shell driver owns orchestration while each Codex session owns only
one proposal or one repair.

## Example Use Case

A maintainer starts the loop from a clean and synchronized `main` checkout.
After `#47` is merged, the Ready queue is empty while `#172` remains Qualified.
The loop starts a separate qualification session, verifies that `#47` is
complete, `#172` is the documented next Pilot Value slice, and the canonical
issue plus `BPANE-00172_PHASE_0_WORKFLOW_ENDPOINT_PLAN.md` contain a bounded
scope, acceptance criteria, and executable test evidence. The qualifier posts
an auditable readiness comment and promotes only `#172` to `state:ready`.

A fresh proposal session then implements that Ready issue on a
`codex/BPANE-*` branch, runs the relevant local checks, and opens one ready PR.
The shell driver waits for GitHub checks. If a check fails, one fresh repair
session diagnoses and fixes that failure. With automatic merge disabled, the
driver stops at a green PR for human review; with explicit opt-in, it first
verifies that the tested branch contains current `main` and then merges.

The same run must stop without promotion when the checkout contains unrelated
work, no Qualified candidate is safely promotable, requirements are ambiguous,
the issue lacks a focused plan, another Codex PR is active, or repair safety
cannot be established.

## Current Evidence

- `../grattis-app/grattis/dev_loop/` provides a proven local
  propose/watch/repair pattern with bounded waits, PID and PR locks, STOP-file
  handling, check-set convergence, branch currency checks, and a run journal.
- BrowserPane uses live GitHub issue lifecycle labels and canonical documents,
  not Grattis `docs/routine-state.json` receipts.
- BrowserPane validation is centralized in `scripts/validate.mjs`; the
  `Validation` workflow runs on pull requests, while the full `Compose`
  workflow currently runs on `main`, schedules, and manual dispatch.
- The installed Codex CLI supports non-interactive `codex exec`, JSONL event
  output, explicit sandboxing, final-message files, and JSON Schema-constrained
  output.
- `docs/CURRENT_CONTEXT.md` records the fresh-session entry point and identifies
  `#172` as the next qualification candidate after completed `#47`.
- The baseline and qualification follow-up merged through PR #243. Issue #246
  owns later routing of evidence-backed requirements gaps to a separate
  documentation PR.

## Scope

- Add a maintained `dev_loop/` with a Codex-native driver, proposal routine,
  qualification routine, repair routine, result schema, tests, and operator
  README.
- When no Ready issue exists, inspect exactly one roadmap-prioritized
  `state:qualified` candidate in a separate Codex session and promote it only
  after dependencies, issue/plan alignment, scope, risks, acceptance, and test
  evidence pass a bounded readiness audit.
- Select exactly one live `state:ready` issue according to the documented
  delivery order; require a matching `docs/*_PLAN.md` before implementation.
- Run Codex non-interactively and retain raw JSONL, schema-valid final output,
  prompts, and a token/duration journal under ignored generated directories.
- Keep polling, retry policy, cancelled-run reruns, branch updates, optional
  merging, and stop behavior in the shell driver.
- Default to supervised operation with `AUTO_MERGE=0`.
- Add a read-only `--check` preflight and shell-level parser/configuration
  tests.

## Non-Goals

- A hosted agent service, GitHub App, merge queue, or cross-repository scheduler.
- Automatic issue creation, deduplication, roadmap reprioritization, or
  promotion without the explicit qualification contract.
- Editing product code, creating branches or PRs, merging, or resolving product
  decisions inside the qualification session.
- Copying Grattis run history, lock state, cloud fix workflow behavior, Maven
  commands, Claude CLI flags, or dollar-cost assumptions.
- Claiming Compose smoke coverage on PRs where GitHub does not run Compose.
- Allowing model sessions to poll CI, merge, close issues, weaken gates, or
  modify the loop implementation unless a future issue explicitly owns it.

## Decisions And Dependencies

- Live GitHub labels are the execution-state source of truth. Local documents
  explain ordering and requirements but do not create a second machine queue.
- The loop consumes Ready work first. If no issue is Ready and
  `AUTO_QUALIFY=1`, one separate session may promote exactly one Qualified
  candidate after a deterministic readiness audit. `AUTO_QUALIFY=0` preserves
  manual queue management.
- The qualifier follows the explicit order in `CURRENT_CONTEXT.md` and
  `DELIVERY_ROADMAP.md`; labels or issue numbers alone never authorize
  reprioritization. It comments the evidence before changing labels and
  verifies the resulting `state:ready` label.
- In the completed #241 baseline, missing plans, material issue/plan drift,
  unresolved decisions, unmet dependencies, overlap, or ambiguous order stop
  without label mutation. Issue #246 narrowly extends that behavior for
  evidence-backed omissions while preserving hard stops for real decisions.
- A `codex/BPANE-` branch prefix and one open PR under that prefix form the
  cross-process mutex. The checkout also has a PID lock.
- The proposal and repair routines never wait for checks and never merge. The
  shell driver is the only CI waiter and optional merger.
- Automatic merge is opt-in. This preserves a supervised adoption path while
  retaining the closed-loop mode requested for controlled environments.
- The driver never resets, stashes, stages, or reverts pre-existing local work.
  A dirty checkout is a hard stop.
- GitHub `Validation` is the PR convergence gate. Issue-specific Compose/E2E
  evidence must be run locally or explicitly deferred to post-merge/manual
  workflow evidence and described truthfully in the PR handoff.

## Contract Changes

- API/OpenAPI: N/A; contributor tooling only.
- Protocol/event schemas: one local JSON Schema for Codex routine results.
- Database/migrations: N/A.
- Admin-new: N/A.
- CLI/SDK: adds `dev_loop/loop.sh` as a repository contributor command, not a
  product CLI.
- Deployment/configuration: local `codex`, `gh`, `git`, `jq`, and Bash are
  required; no runtime deployment change.
- README/ARCH/AGENTS/operator docs: document the loop in `dev_loop/README.md`
  and add the contributor guardrail to `AGENTS.md`. `ARCH.md` is unaffected
  because runtime architecture does not change.

## Security And Data Impact

- `codex exec` receives repository and authenticated CLI access. The default
  unattended mode therefore requires an isolated, trusted workstation and
  least-privilege GitHub credentials.
- Raw JSONL and prompts can contain source, command output, issue content, or
  diagnostic data. Generated runs stay local and ignored; routines must not
  print secrets or resolved credential values.
- The driver must not discover credentials from repository-local token files,
  echo environment secrets, alter authentication configuration, disable
  workflows, or bypass branch protection.
- Sandbox and approval settings are explicit and configurable. The README must
  explain why network-capable unattended execution is high trust.

## Migration, Compatibility, And Rollback

The change is additive. Existing manual Codex sessions, GitHub workflows, and
repository commands remain unchanged. Rollback consists of removing the
versioned `dev_loop/` files and their contributor references; generated run
data is already ignored. The loop must leave open PRs and branches intact on
failure so a maintainer can inspect or continue them manually.

## Observability And Operator Feedback

- Console phases distinguish preflight, qualification, proposal, CI wait,
  repair, branch update, merge decision, and stop reason.
- Per-run directories retain exact prompts, JSONL events, final JSON results,
  and `journal.tsv` rows.
- Journals record iteration, PR, outcome, repair count, wall time, input tokens,
  cached input tokens, output tokens, and reasoning output tokens.
- Errors name the non-destructive recovery action. They must not conceal dirty
  trees, check timeouts, merge conflicts, exhausted repairs, or no-work states.

## Implementation Slices

1. Add the focused issue/plan and generated-state ignore rules.
2. Port the shell orchestration to Codex structured execution and BrowserPane
   GitHub/validation semantics.
3. Replace Grattis proposal and repair prompts with BrowserPane-specific
   routines and a strict result schema.
4. Add read-only preflight, parser/configuration tests, and contributor docs.
5. Run shell/document validation, review the diff, and commit the coherent
   tooling slice without staging unrelated certificate files.
6. Add the bounded qualification follow-up: routine, structured outcomes,
   driver transition, issue-label verification, tests, and aligned contributor
   documentation.

## Test Strategy

### Unit

- Parse every allowed structured routine status and reject malformed output.
- Accept schema-valid `QUALIFIED` and `NO_QUALIFICATION` results while rejecting
  promotions without an issue number or no-candidate results without a reason.
- Extract token usage from representative Codex JSONL completion events.
- Extract unique GitHub run IDs from check links.
- Validate booleans, merge methods, numeric bounds, and required files.
- Consume `STOP` once and recognize stale/live PID locks through isolated
  fixture paths where practical.

### Integration

- Source the driver in library mode so tests exercise its real functions
  without invoking Codex or GitHub mutations.
- Exercise qualification context creation and the no-Ready transition with
  mocked Codex/GitHub boundaries; assert that proposal follows only after the
  selected issue is visibly `state:ready`.
- Run `--check` to validate installed tools, Codex CLI availability, GitHub
  authentication, repository metadata, and dirty-state reporting without
  switching branches or editing files.
- Validate the final-result JSON Schema with `jq` and the installed Codex CLI
  option parser.

### Smoke And E2E

- The implementation slice will not start a real autonomous proposal because
  it would claim `#47` and create a product PR. The documented supervised smoke
  is the first explicit acceptance run after this tooling PR lands.
- A disposable issue/branch should be used later to exercise failure repair,
  cancelled rerun, branch update, STOP, and opt-in merge behavior end to end.

### Coverage And Quality

- Run `bash -n` for the driver and shell tests.
- Run the driver test script.
- Run `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents`.
- Review shell code for quoting, `set -euo pipefail`, bounded waits, and
  non-destructive Git behavior.

## Manual Test Sequence

1. Start from a clean `main` checkout synchronized with `origin/main`.
2. Confirm `codex --version`, `gh auth status`, `git`, `jq`, and Bash are
   available.
3. Run `./dev_loop/loop.sh --check`; expect a read-only report.
4. Run `./dev_loop/tests/loop_test.sh`; expect all isolated tests to pass.
5. With no Ready issue, review the next Qualified issue and its focused plan.
6. Run `ITERATIONS=1 AUTO_QUALIFY=1 AUTO_MERGE=0 ./dev_loop/loop.sh` under
   supervision.
7. Confirm the qualification session comments its evidence, promotes exactly
   one issue, and exits before proposal starts.
8. Confirm exactly one Codex branch and PR are created and no unrelated file is
   staged.
9. Inspect the PR handoff, local validation evidence, JSONL/final result, and
   journal.
10. Let GitHub checks finish; expect the driver to stop with the green PR open.
11. Clean up or merge manually. Enable `AUTO_MERGE=1` only after the supervised
    path and branch protections are verified.

## Documentation And Claim Impact

This is contributor automation, not product maturity evidence. It does not
advance Foundation, Phase 0, Production, or Phase N gates and must not appear as
a runtime capability. `docs/CURRENT_CONTEXT.md` may mention it only as an
optional clean-session execution aid. `README.md`, `ARCH.md`, OpenAPI, and
investor claims require no product change.

## Definition Of Done

- Issue `#241` and this plan agree on scope, use case, acceptance criteria, and
  smoke sequence.
- The Codex-native driver and routines contain no Grattis/Claude/Maven/fix-
  workflow assumptions.
- Structured output, local logs, bounded convergence, locks, STOP, branch
  currency, and opt-in merge behavior are implemented.
- An empty Ready queue invokes at most one bounded qualifier; successful
  promotion is verified through GitHub before proposal, while ambiguous or
  blocked candidates stop without mutation.
- Unit-style shell tests and read-only integration preflight pass.
- Repository baseline and document validation pass.
- Generated loop files are ignored and unrelated local changes remain
  untouched.
- Usage and security boundaries are documented.

## Post-Implementation Smoke Sequence

1. Run `./dev_loop/loop.sh --check` and confirm it reports tools, auth,
   repository, active Codex PR, and cleanliness without mutation.
2. Run `./dev_loop/tests/loop_test.sh`.
3. From clean synchronized `main` with no Ready issue and one eligible
   Qualified candidate, run one supervised iteration with
   `ITERATIONS=1 AUTO_QUALIFY=1 AUTO_MERGE=0`.
4. Verify the qualifier records dependencies, plan path, scope, risks,
   acceptance, and test evidence before promoting exactly one issue.
5. Verify the selected issue is visibly Ready and has a matching focused plan
   before proposal starts.
6. Verify one ready PR contains `Closes #<issue>`, a routine handoff, and
   truthful local/deferred validation evidence.
7. Verify the shell, not Codex, waits for GitHub checks.
8. Inspect the ignored run directory for qualification/proposal prompts,
   JSONL, final results, and token
   journal artifacts.
9. Exercise repair, qualification halt, no-candidate, and STOP behavior on
   disposable fixtures before
   enabling unattended mode.

## Evidence Record

- Issue: <https://github.com/ITmedes/browserpane/issues/241>
- Source pattern reviewed: local `../grattis-app/grattis/dev_loop/`
- Issue metadata: authored by the approved project identity, labeled
  `priority:P1`, `lane:foundation`, and `state:in-progress`, with milestone
  `Foundation Gate`.
- Baseline commits: plan `a0a0f524`; implementation `b6f32c14`; merged on
  `main` before the qualification follow-up.
- Validation:
  - `bash -n dev_loop/loop.sh`
  - `bash -n dev_loop/tests/loop_test.sh`
  - `./dev_loop/tests/loop_test.sh` - 24/24 checks passed, including
    status-specific results, token parsing, GitHub check exit-code handling,
    post-merge workflow outcomes and conditional Rust-builder selection,
    identity/permission policy, STOP, and stale locks.
  - `node scripts/validate.mjs --stage repository-baseline --stage
    repository-documents` - passed with 43 tracked JSON files, 94 Markdown
    files, 19 YAML files, and 3 workflows.
  - installed Codex CLI option parse for approval, JSONL, sandbox, repository,
    output schema, and final-message output - passed.
  - real `run_session` read-only Codex round trip - emitted
    `thread.started`, `turn.started`, `item.completed`, and `turn.completed`,
    wrote a schema-valid `NO_PROPOSAL`, and exposed parseable usage metadata.
  - `./dev_loop/loop.sh --check` - approved project identity and `ADMIN`
    permission were verified; the read-only report correctly rejected the
    current non-main dirty implementation checkout. An unapproved identity was
    rejected before repository GitHub operations.
- Runtime/product tests: N/A; this slice changes contributor orchestration and
  no BrowserPane runtime, API, protocol, UI, CLI, or deployment contract.
- Root `README.md`, `ARCH.md`, OpenAPI, and investor material: no change needed;
  contributor usage is isolated in `dev_loop/README.md`, `AGENTS.md`, and the
  current-context/roadmap documents.
- Supervised first autonomous iteration: completed against #47. The loop
  produced PR #242, which passed its required validation, merged at
  `22150090`, and closed #47.

### Qualification Follow-Up Evidence

- Branch: `feature/BPANE-00241-qualification` from `22150090`.
- Issue comment: the approved project identity recorded the bounded
  qualification business case, example, acceptance, and safety limits before
  implementation.
- `bash -n dev_loop/loop.sh dev_loop/tests/loop_test.sh` - passed.
- `jq -e . dev_loop/schemas/routine-result.schema.json` - passed.
- `./dev_loop/tests/loop_test.sh` - 42/42 checks passed, including
  schema-valid qualification/no-qualification results, nounset-safe context
  generation, an empty-Ready successful promotion gate, rejection when the
  reported issue is not exclusively Ready, rejection of multiple promotions,
  manual-qualification bypass, no-candidate short-circuiting, identity policy,
  CI outcomes, STOP, and lock behavior.
- `node scripts/validate.mjs --stage repository-baseline --stage
  repository-documents` - passed with 47 tracked JSON files, 95 Markdown
  files, 19 YAML files, and 3 workflows.
- Live read-only boundary check through the repository-local `gh` config:
  `thebackplane`, `ADMIN`, zero Ready issues, and 32 Qualified issues.
- Real #172 label/comment mutation is deferred until this follow-up is merged;
  running the version under test from a feature branch would correctly violate
  the qualifier's clean synchronized `main` precondition.
- Root `README.md`, `ARCH.md`, OpenAPI, and runtime/product tests: no change
  required because this follow-up changes contributor orchestration only.
