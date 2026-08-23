# Compose Reliability Baseline And Flake Ledger

Status: pre-instrumentation baseline for BPANE-00283

Reviewed: 2026-08-23

## Sample And Method

This baseline covers the 30 most recent completed `Compose` workflow runs at
the time of review, from run `32371724777` through `32589059270`. The sample
contains pushes, manual dispatches, and one scheduled run. It is not the future
30-run unchanged-scheduled target window.

The run inventory came from `gh run list --workflow Compose --limit 30`. Lane
conclusions and current-attempt start/completion times came from the GitHub
Actions jobs API. Full wall time is the longest current-attempt lane duration,
not the workflow API's created-to-updated interval; the latter includes time
between manually rerun attempts. Historical failures without deterministic
stage evidence remain `unknown` instead of being inferred from log text.

## Baseline Result

- 18 of 30 runs ended successful (60.0%).
- 16 of 30 were successful on attempt 1 (53.3% first-pass success).
- Nine ended failed and three ended cancelled. Cancellations are separated from
  first-pass failures.
- Five records were on attempt 2: two ended successful and three still failed.
- Median full wall time was 16.06 minutes; the observed range was 12.08 to
  20.57 minutes.
- Lane medians were 15.07 minutes for gateway default, 12.70 for gateway
  docker-pool, 16.06 for browser integrations, 10.68 for unified admin, and
  14.96 for compatibility admin.
- Only one run was scheduled, so the >=95% first-pass target over 30 unchanged
  scheduled runs cannot yet be evaluated. That target window starts only after
  the v1 evidence contract is present on the unchanged scheduled tree.

Initial engineering targets remain <=8 minutes for canary, <=12 minutes for an
affected-area run, <=15 minutes full median, and >=95% first-pass success over
30 unchanged scheduled runs. These are engineering targets, not product SLOs.

## Run Inventory

`Failed lane` is retained only as coarse pre-v1 evidence. `Class` is terminal
`unknown` unless repository evidence already identifies the cause. Run
`32584253673` is the documented product failure that led to #280; superseded
concurrency cancellations are infrastructure outcomes, not passing runs.

| Run | Event | Attempt | Conclusion | Wall min | Failed lane | Class |
| ---: | --- | ---: | --- | ---: | --- | --- |
| 32589059270 | push | 1 | success | 16.92 | — | — |
| 32588496962 | push | 1 | cancelled | 12.08 | — | infrastructure |
| 32587607368 | workflow_dispatch | 1 | success | 16.85 | — | — |
| 32584253673 | push | 2 | failure | 16.60 | admin-compatibility | product (#280) |
| 32582292900 | workflow_dispatch | 2 | success | 17.13 | — | — |
| 32562165212 | push | 1 | success | 17.15 | — | — |
| 32561601341 | push | 1 | cancelled | 13.07 | — | infrastructure |
| 32560886312 | push | 1 | cancelled | 16.43 | admin-unified | unknown + cancelled |
| 32559531743 | workflow_dispatch | 1 | success | 16.70 | — | — |
| 32558245570 | workflow_dispatch | 1 | failure | 15.17 | admin-compatibility, browser | unknown |
| 32557421970 | workflow_dispatch | 1 | failure | 14.72 | admin-compatibility, browser | unknown |
| 32538701324 | push | 2 | failure | 15.03 | browser, admin-compatibility | unknown |
| 32533464416 | push | 1 | success | 16.30 | — | — |
| 32528801113 | push | 1 | success | 20.55 | — | — |
| 32524429776 | push | 1 | success | 16.15 | — | — |
| 32521326576 | push | 1 | success | 15.58 | — | — |
| 32506614254 | push | 1 | failure | 15.85 | admin-unified | unknown |
| 32504021980 | push | 1 | success | 16.05 | — | — |
| 32500100672 | push | 2 | success | 15.15 | — | — |
| 32487417743 | push | 1 | success | 15.98 | — | — |
| 32481154151 | push | 2 | failure | 15.15 | admin-unified | unknown |
| 32478683433 | push | 1 | failure | 20.57 | admin-unified | unknown |
| 32475609343 | push | 1 | success | 15.35 | — | — |
| 32448465834 | push | 1 | success | 15.90 | — | — |
| 32447354213 | push | 1 | success | 15.83 | — | — |
| 32442913128 | schedule | 1 | failure | 16.07 | admin-compatibility | unknown |
| 32400433778 | push | 1 | failure | 15.42 | admin-compatibility | unknown |
| 32398334792 | push | 1 | success | 17.18 | — | — |
| 32381808133 | push | 1 | success | 16.15 | — | — |
| 32371724777 | push | 1 | success | 17.47 | — | — |

## Reviewable Flake Ledger

The pre-v1 failures above remain in the inventory. After BPANE-00283, add a
ledger row only from the retained v1 JSON/JUnit evidence, using the tested tree,
workflow/test-plan hashes, image digests, run attempt, lane, first failed stage,
failure class, cleanup outcome, and disposition. Never promote `unknown` to a
flake, pass, or retry based only on log text.

| Run / attempt | Input identity | Lane / stage | Class | Cleanup | Disposition |
| --- | --- | --- | --- | --- | --- |
| 32584253673 / 2 | pre-v1; merge SHA `02b2c1dd1a5f` | admin-compatibility / admin validation | product | succeeded | Protocol bootstrap defects owned by #280; fixed by PR #281. |
| 32442913128 / 1 | pre-v1; scheduled SHA `51101a3480dc` | admin-compatibility / admin validation | unknown | succeeded | Do not count as a flake; deterministic evidence was unavailable. |

The ledger is intentionally small: repeated symptoms without typed evidence do
not justify a guessed classification. The v1 artifacts provide the fields
needed for future entries without retaining credentials, identity claims,
requested URLs, browser content, or unbounded logs.
