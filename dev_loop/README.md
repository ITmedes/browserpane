# BrowserPane Codex Development Loop

`dev_loop/loop.sh` moves BrowserPane delivery from one long conversation into
short, bounded Codex sessions while retaining the repository's issue, plan,
test, review, and documentation rules.

```text
Ready issue? -- yes -------------------------------------> propose implementation PR
     | no
     v
qualify one Qualified issue
     | ready --------------------------------------------> propose implementation PR
     | resolvable requirements gaps -> specify docs PR --+
     | unresolved decision / unsafe state -> stop        |
                                                         v
                                           driver watches checks
                                             | red -> repair -> watch (bounded)
                                             | green/current -> review (default)
                                             | opt-in merge -> exact-head Compose
                                             |                 | red -> repair
                                             |                 | green -> merge
                                             + merged -> next iteration requalifies
```

Qualification, requirements specification, proposal, and repair use separate
Codex sessions. They never wait for CI and never merge. The shell driver owns
those operations, so each model session has one auditable responsibility.

## Prerequisites

- A clean local `main` checkout that can fast-forward to `origin/main`.
- `awk`, `bash`, `df`, `git`, `gh`, `jq`, and the Codex CLI on `PATH`.
- `gh` authentication as `thebackplane` or another explicitly approved
  project identity that can read checks, push branches, create/edit PRs, and
  update issue labels. Personal identities are not accepted.
- Existing Codex authentication. For automation, scope `CODEX_API_KEY` to the
  process rather than storing it in this repository.
- At least one live GitHub issue with `state:ready`, or one
  roadmap-prioritized `state:qualified` candidate, with a matching focused
  `docs/BPANE-<five-digit-issue>_*_PLAN.md`.

The loop deliberately refuses a dirty tree. It never stashes, resets, stages,
or reverts pre-existing changes. Preserve local certificate changes or other
work yourself before starting it. It also refuses to start a local Codex phase
with less than 50 GiB free on the repository filesystem by default.

## Start Safely

First run the read-only preflight and isolated tests:

```bash
./dev_loop/loop.sh --check
./dev_loop/tests/loop_test.sh
```

Then run one supervised iteration. A green PR remains open for human review:

```bash
ITERATIONS=1 AUTO_QUALIFY=1 AUTO_MERGE=0 ./dev_loop/loop.sh
```

`AUTO_QUALIFY=1` is the default. Set it to `0` when a maintainer wants to
manage the Ready queue manually.

After the supervised path and branch protection are verified, enable the
closed merge loop explicitly:

```bash
AUTO_MERGE=1 ./dev_loop/loop.sh
```

`AUTO_MERGE=1` does not bypass branch protection. A green/current PR that
still needs approval, has requested changes, or is blocked by another branch
policy remains open and stops with a specific journal outcome. Restart the
loop after an authorized reviewer or repository administrator resolves that
gate. Once the merge gate is otherwise open, the driver manually dispatches
the full `Compose` workflow against the exact PR head and merges only after it
passes. A failed branch run enters the same bounded repair cycle as an ordinary
PR check. Any repair, branch update, or concurrent head change invalidates the
old Compose evidence. The merge API call also carries the tested head SHA, so a
last-moment head change is rejected by GitHub rather than published unchecked.
After that loop-owned merge, the first failed required workflow is eligible for
one failed-job rerun by default. The driver stays on the same Actions run,
required workflow, and exact merge SHA; a repeated failure, evidence mismatch,
rerun rejection, unavailable run, or timeout remains terminal.

When the approved project identity has repository `ADMIN` permission and is
explicitly authorized to land green/current PRs directly, opt into GitHub's
admin merge path separately:

```bash
AUTO_MERGE=1 ADMIN_MERGE=1 ./dev_loop/loop.sh
```

This mode still waits for green checks, requires a current branch, and refuses
content conflicts or `CHANGES_REQUESTED`. It only bypasses a remaining required
review or generic branch-policy block.

Stop cleanly between phases from another shell:

```bash
touch dev_loop/STOP
```

Ctrl-C also stops the driver. A restart adopts the oldest open
`codex/BPANE-` PR rather than proposing a second one. Draft Codex PRs are left
open for manual inspection because a terminated specification or proposal
session may not have finished its validation.

## What Is Versioned

| Path | Purpose |
|---|---|
| `loop.sh` | Synchronize, qualify, specify or propose, watch, repair, update, optionally merge, and journal. |
| `routines/qualify.md` | Audits and either promotes one Qualified issue or identifies one resolvable requirements gap set. |
| `routines/specify.md` | Reconciles one selected issue and focused plan in a documentation-only PR. |
| `routines/propose.md` | Implements one canonical Ready issue and opens one ready PR. |
| `routines/repair.md` | Diagnoses one failed check set or merge conflict and makes one bounded repair decision. |
| `schemas/routine-result.schema.json` | Constrains the final output of every non-interactive Codex session. |
| `tests/loop_test.sh` | Exercises parsing, usage accounting, STOP, identity, schema, and configuration without GitHub mutation. |

Generated `runs/`, `.lock/`, and `STOP` state is ignored. Run directories
contain exact prompts, raw Codex JSONL events, stderr diagnostics, final result
JSON, and a token journal. Treat those local files as potentially sensitive
diagnostic data.

## Configuration

| Variable | Default | Meaning |
|---|---:|---|
| `ITERATIONS` | `0` | `0` continues until stopped or no Ready work remains. |
| `MAX_REPAIRS` | `4` | Repair sessions allowed for one PR. |
| `MAX_SPECIFICATION_CYCLES` | `3` | Requirements-specification PR cycles allowed in one loop run. |
| `MAX_UPDATE_BRANCH` | `3` | Times a green PR may be updated after `main` moves. |
| `CI_TIMEOUT_SECONDS` | `5400` | Maximum wait for one PR check set. |
| `POLL_SECONDS` | `30` | PR check polling interval. |
| `FAIL_FAST` | `1` | Repair after the first failure and grace period. |
| `FAIL_FAST_GRACE` | `120` | Time to collect nearby sibling failures. |
| `AUTO_RERUN_CANCELLED` | `2` | Driver reruns for cancelled Actions runs. |
| `POST_MERGE_FAILED_RERUNS` | `1` | Failed-job reruns for each required exact-SHA post-merge workflow; accepts `0` through `3`. |
| `SETTLE_SECONDS` | `180` | Wait for a new check set after a push/update. |
| `SESSION_TIMEOUT_SECONDS` | `10800` | Watchdog for one Codex session. |
| `PRE_MERGE_TIMEOUT_SECONDS` | `7200` | Maximum wait for exact-head Compose before an automatic merge. |
| `POST_MERGE_TIMEOUT_SECONDS` | `7200` | Maximum wait per post-merge main workflow. |
| `MIN_FREE_DISK_GB` | `50` | Minimum binary GiB available on the repository filesystem before local work; `0` explicitly disables the minimum. |
| `AUTO_QUALIFY` | `1` | `1` audits and promotes one Qualified issue when the Ready queue is empty. |
| `AUTO_MERGE` | `0` | `1` enables merge after green/current verification. |
| `ADMIN_MERGE` | `0` | `1` explicitly permits `gh pr merge --admin` for a green/current review- or policy-blocked PR; requires `AUTO_MERGE=1` and repository `ADMIN` permission. |
| `MERGE_METHOD` | `squash` | `squash`, `merge`, or `rebase`. |
| `DEFAULT_BRANCH` | `main` | Published base branch. |
| `BRANCH_PREFIX` | `codex/BPANE-` | Cross-process PR lock prefix. |
| `PRE_MERGE_WORKFLOW` | `compose.yml` | Manually dispatched exact-head workflow required before automatic merge. |
| `POST_MERGE_WORKFLOWS` | `auto` | Main workflows that must pass before the next issue; `auto` adds the Rust builder when its paths changed. |
| `CODEX_BIN` | `codex` | Codex executable. |
| `CODEX_SANDBOX` | `danger-full-access` | Sandbox passed to unattended `codex exec`. |
| `APPROVAL_POLICY` | `never` | Approval policy passed before `codex exec`. |
| `ALLOWED_GITHUB_LOGINS` | `thebackplane` | Comma-separated project identities allowed to run the loop. |
| `MODEL` | CLI default | Optional Codex model override. |
| `CODEX_PROFILE` | none | Optional Codex configuration profile. |

`danger-full-access` plus `never` is intentionally explicit because
specification, proposal, and repair sessions need repository writes, networked
GitHub access, and local validation without an interactive approval prompt.
Use this only on a controlled workstation with credentials and repository
access you are willing to delegate. Override the sandbox when a narrower local
setup still supports the required work.

## BrowserPane-Specific Delivery Rules

- Live GitHub labels own execution state; there is no duplicate local queue.
- The loop consumes `state:ready` first. When that queue is empty and
  `AUTO_QUALIFY=1`, a separate session may promote exactly one
  `state:qualified` issue after verifying roadmap order, dependencies, focused
  plan, bounded scope, risks, acceptance criteria, and test/smoke evidence.
- When the direction is decided but the issue/plan contract has concrete gaps
  that current evidence can resolve, qualification may instead route exactly
  one issue to a fresh specification session. That session may reconcile the
  canonical issue and directly related planning docs in one PR, but it cannot
  implement product code or change lifecycle labels.
- A genuinely external decision on the first candidate does not make the whole
  queue executable or empty. Qualification traverses only the finite ordered
  fallback queue named by the canonical roadmap, keeps each deferred issue and
  blocker visible without mutation, and stops at the first promotable or
  evidence-backed specification candidate. It returns `NO_QUALIFICATION` only
  after that documented queue is exhausted.
- A missing focused plan may itself route to specification when exactly one
  issue owns the scope and its issue plus canonical repository evidence are
  sufficient to create the complete plan without inventing product, legal,
  security, commercial, or external-stakeholder policy.
- Qualification never edits Git, creates a PR, implements code, waits for CI,
  or merges. Unresolved product/security/legal decisions, unmet dependencies,
  ambiguous ordering, or ownership conflicts stop without mutation.
- A successful `QUALIFIED` result may include a concise rationale. The rationale
  is informational; the driver still verifies the original Qualified queue,
  exact Ready count, and selected issue lifecycle before proposal.
- A specification PR must merge before a later iteration reruns qualification.
  Specification and product implementation never occur in the same iteration.
- Every issue needs a focused `docs/*_PLAN.md` with business case, use case,
  contract impact, security, tests, manual smoke, and Definition of Done.
- Proposal PRs use `Closes #N` only after the full bounded issue is complete.
- Local validation uses `scripts/validate.mjs` and the repository's package and
  Compose wrappers. Exact evidence is recorded.
- `Validation` runs automatically on pull requests. Full `Compose` remains a
  `main`, schedule, or manual-dispatch workflow instead of consuming runner
  capacity for every human PR.
- In automatic-merge mode, the driver dispatches full `Compose` against the
  exact current PR head before merge. It adopts an existing exact-head run on
  restart, routes red evidence to bounded repair while the PR remains open, and
  rejects evidence from an older head.
- After merge, the driver still waits for `Validation` and `Compose` on the
  exact merge SHA before it starts another issue. It also waits for the CI Rust
  builder publication when that workflow's path filter matches the PR. After a
  successful exact-head pre-merge gate, each first failed published-main
  workflow may rerun failed jobs on that same run and merge SHA within the
  separately bounded `POST_MERGE_FAILED_RERUNS` budget. Cancelled-run retries
  retain their independent budget. Exhaustion, mismatched evidence, rejected
  dispatch, an unavailable run, or timeout remains a fail-closed stop.
- README, ARCH, AGENTS, OpenAPI, Admin-New, CLI, and operator documentation stay
  aligned when an implementation changes their contract.

## Stop Outcomes

The driver leaves branches and PRs intact for diagnosis:

- `no-proposal`: no safe Ready issue was available.
- `no-qualification`: no Qualified candidate passed the bounded readiness
  audit and no evidence-backed specification cycle was safe.
- `specification-failed`: the dedicated requirements session or its structured
  result failed.
- `specification-unverified`: the reported issue, lifecycle state, PR, or
  commit did not match the driver's live verification.
- `specification-budget-exhausted`: the run consumed
  `MAX_SPECIFICATION_CYCLES`; inspect the remaining decision or recurring gap
  before restarting.
- `low-disk`: available repository-filesystem capacity fell below
  `MIN_FREE_DISK_GB` or could not be measured; clean local storage or adjust
  the threshold explicitly before restarting.
- `qualified-awaiting-proposal`: qualification completed and STOP was consumed
  before a proposal session started.
- `qualification-awaiting-specification`: a gap set was identified and STOP
  was consumed before the specification session started.
- `green-awaiting-review`: checks passed and automatic merge was disabled.
- `review-required`: checks passed and automatic merge was enabled, but branch
  protection still requires an authorized approval.
- `changes-requested`: checks passed, but review feedback blocks the merge.
- `merge-policy-blocked`: checks passed, but another branch policy blocks the
  merge; inspect the PR without starting a code repair.
- `base-state-stale`: local ancestry is current while GitHub still reports the
  PR behind; wait for merge-state convergence and rerun.
- `merge-state-unavailable` or `merge-state-unknown`: GitHub merge state could
  not be read or classified safely.
- `merge-command-failed`: GitHub still reports the PR ready after a failed
  merge command; inspect the command/policy response directly.
- `admin-merge-failed`: explicit admin merge was requested, but GitHub still
  refused the green/current PR; inspect identity and branch bypass policy.
- `update-branch-failed`: the PR is behind but GitHub could not update it and
  did not report a content conflict; inspect the live update response.
- `draft-pr`: an interrupted proposal left an incomplete draft.
- `ci-timeout`: the PR check set did not conclude in time.
- `pre-merge-compose-failed`: exact-head Compose could not converge within the
  bounded repair budget; the PR remains open and unmerged.
- `repairs-exhausted`: the bounded repair budget was consumed.
- `base-keeps-moving`: `main` moved repeatedly under a green PR.
- `halted`: Codex found an ambiguous or unsafe state.
- `post-merge-failed`: a required workflow on the exact merged SHA exhausted
  its failed/cancelled retry budget, produced mismatched or unavailable
  evidence, rejected the rerun, or timed out, so no later issue was started.

The journal is the first place to inspect after an unattended run. Its bounded
`post_merge_workflows` column records workflow name, run id, final attempt,
failed and cancelled rerun counts, and fixed conclusion without raw logs.
