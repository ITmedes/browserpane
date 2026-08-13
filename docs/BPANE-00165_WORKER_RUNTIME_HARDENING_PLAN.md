# BPANE-00165 Worker Runtime Hardening Plan

Status: In Progress

Issue: [#165](https://github.com/ITmedes/browserpane/issues/165)

Branch: `feature/BPANE-00165-worker-runtime-hardening`

## Outcome

Bound workflow and recording worker resource use so noisy child processes,
slow control-plane calls, supervisor polling, and large ZIP exports cannot
silently consume unbounded memory or block gateway request threads.

This slice also establishes the missing unit-test floor for both worker
packages. It uses the Node.js test runner and standard cancellation APIs rather
than introducing a package-specific test framework or HTTP client.

## Business Case

BrowserPane workers execute user-selected workflows and long-running recording
jobs. Their output volume and the latency of their dependencies are not fully
controlled by BrowserPane. A production-shaped runtime therefore needs bounded
logs, explicit network deadlines, single-flight polling, and archive generation
that does not monopolize asynchronous request threads.

## Example Use Case

A workflow prints several megabytes of diagnostic output while the gateway is
temporarily slow. At the same time, an operator downloads a multi-segment
recording bundle. BrowserPane must retain only a bounded, diagnostically useful
tail of each worker stream, fail the slow control request with an actionable
timeout, avoid starting a second poll while the first is outstanding, and keep
unrelated API requests responsive while the ZIP is built.

## Current Gaps

- The workflow worker buffers complete entrypoint stdout and stderr in memory.
- Gateway workflow and recording supervisors use `wait_with_output`, which
  buffers complete child output.
- Worker gateway and OIDC requests have no explicit timeout.
- Recording finalization polling is sequential in implementation but lacks a
  regression test proving that calls cannot overlap.
- Recording playback and browser-context export ZIP creation runs synchronous
  CPU work on Tokio request threads.
- Recording-worker and workflow-worker only have TypeScript build checks; they
  have no package unit-test command enforced by repository validation.
- Several binary responses clone complete byte buffers before constructing an
  Axum body.

## Scope

### 1. Worker Test And Deadline Foundation

1. Add package-local `node:test` suites executed through `tsx`.
2. Add a finite, configurable gateway request timeout to both control clients.
3. Apply the same finite timeout to OIDC client-credential requests.
4. Preserve caller-provided abort signals and report timeout failures with the
   operation and configured duration, without exposing credentials.
5. Wire tests into the canonical fast validation and hosted worker jobs.

### 2. Bounded Output And Polling

1. Replace unbounded workflow-entrypoint stream concatenation with a bounded
   tail collector per stream.
2. Preserve complete output below the limit and add one explicit truncation
   marker above it.
3. Replace gateway supervisor `wait_with_output` usage with concurrent bounded
   stdout/stderr draining so full pipes cannot deadlock a child process.
4. Keep the final non-empty diagnostic line available to lifecycle failure
   handling.
5. Add unit tests for exact-boundary, truncation, split UTF-8/chunk, and noisy
   process behavior.
6. Add a recording-service test proving finalize polls are sequential and stop
   after a terminal state.

### 3. Archive Runtime Isolation

1. Read recording segment artifacts asynchronously, then build the playback ZIP
   inside `spawn_blocking`.
2. Build browser-context export ZIPs inside `spawn_blocking`.
3. Preserve archive format, filenames, manifests, content types, and error
   mapping.
4. Remove full-buffer response clones where ownership can move directly into
   the response body.
5. Add large valid archive tests and a responsiveness test that demonstrates
   unrelated Tokio work progresses while packaging runs.

### 4. Integration And Documentation

1. Keep worker environment defaults explicit in compose/runtime launch
   configuration.
2. Update `AGENTS.md`, `README.md`, `ARCH.md`, and validation documentation only
   where commands, topology, or support behavior changes.
3. Mark #149 and risk R-009 done after its merge; update #165/R-018 evidence as
   this slice progresses.
4. Keep the issue body and labels aligned with the implementation and evidence.

## Non-Goals

- Streaming archive responses or replacing current artifact stores.
- A general worker SDK or cross-package refactor.
- Workflow execution deadlines, retry policy, or Human Handoff semantics owned
  by workflow endpoint issues.
- Platform-wide metrics/SLO implementation owned by #178.
- Generalized artifact APIs owned by #21.

## Acceptance Criteria

- Both worker packages have enforced unit-test commands and tests for request
  timeout behavior.
- Worker HTTP and OIDC requests have finite defaults and configurable values.
- Workflow entrypoint and gateway-supervised process output cannot accumulate
  beyond the configured per-stream bound.
- Recording finalize polling is demonstrably single-flight.
- Recording playback and browser-context ZIP construction do not execute on
  Tokio request threads.
- Existing archive formats and worker lifecycle behavior remain compatible.
- Workflow, recording, browser-context export, admin recording, and relevant
  compose tests pass.

## Validation Plan

### Unit And Package Tests

```bash
cd code/integrations/recording-worker && npm test && npm run build
cd code/integrations/workflow-worker && npm test && npm run build
cargo test -p bpane-gateway recording_lifecycle
cargo test -p bpane-gateway workflow_lifecycle
cargo test -p bpane-gateway recording_playback
cargo test -p bpane-gateway browser_context
```

### Repository Validation

```bash
cargo fmt --all -- --check
cargo clippy -p bpane-gateway --all-targets -- -D warnings
node scripts/validate.mjs --profile fast
```

### Post-Implementation Smoke Sequence

1. Start the supported compose stack and verify `/healthz` and `/readyz`.
2. Run a workflow fixture that emits output above the configured limit; verify
   the run terminates normally, retained stdout/stderr is bounded, and a clear
   truncation marker is present.
3. Delay a worker-facing control endpoint beyond the configured request
   timeout; verify the worker fails predictably and the run/recording reaches an
   actionable failure state.
4. Start an always-recorded session, connect, interact, stop it, and verify one
   ready downloadable WebM plus a valid playback ZIP.
5. Export a large multi-segment recording while polling `/readyz` and another
   catalog endpoint; verify both remain responsive.
6. Export and import a browser context with profile data and verify the restored
   context remains usable.
7. Run the workflow, recording, admin-new recordings, and browser-context
   compose/browser smokes selected by the validation matrix.

## Rollback

- Request timeouts and output bounds remain configurable, so defaults can be
  raised without restoring unbounded behavior.
- Archive task isolation does not change persisted formats and can be reverted
  independently if runtime evidence exposes a regression.
- Worker test commands are additive and must remain even if an implementation
  detail is rolled back.

## Evidence

Record exact commands, counts, coverage, compose fixtures, and hosted checks
here as each implementation slice completes.
