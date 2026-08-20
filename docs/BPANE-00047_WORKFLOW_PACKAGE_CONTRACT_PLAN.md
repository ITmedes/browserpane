# BPANE-00047 Workflow Package Contract Plan

Issue: [#47](https://github.com/ITmedes/browserpane/issues/47)

Status: Ready

Lane: Pilot Value

Target gate: Phase 0 Operational Proof prerequisite

Depends on: merged Foundation validation, source hardening, worker lifecycle,
store parity, and API-conformance baselines

Last verified: 2026-08-20 on `main` at `2adbfdee`

## Business Outcome

Freeze one supported, reproducible workflow package and publication contract
before external BPM callers depend on BrowserPane execution.

## Example Use Case

An engineer maintains a TypeScript/Playwright workflow in Git. BrowserPane
resolves the reviewed ref to an immutable commit, validates its entrypoint and
declared resource requirements, publishes a version, and replays it in a fresh
session. API, CLI, and Admin-New show the same commit, schemas, requirements,
and publication state without exposing credentials.

## Current Baseline

Implemented:

- workflow definitions and immutable version resources,
- Git source resolution and commit pinning,
- bounded source listing/preview and path/symlink protection,
- TypeScript/Playwright worker execution,
- entrypoint, input/output schema metadata, resource allowlists, source
  snapshots, logs, events, and produced files,
- workflow CLI helpers and Admin-New workflow/source/run views.

Gaps to freeze:

- `executor` remains a free-form string although only Playwright is supported,
- source, entrypoint, schema, resource, and compatibility requirements are not
  described as one supported package contract,
- credential-injection payload/redaction behavior needs one stable public
  statement,
- publication and regression evidence is spread across implementation and
  smoke helpers rather than one repeatable example.

## Scope

1. Define Playwright TypeScript as the only supported Phase 0 executor and
   reject unsupported executor values through the public contract.
2. Require Git-backed source, a resolved immutable commit, a validated relative
   entrypoint, and explicit workflow/runtime compatibility metadata.
3. Adopt JSON Schema Draft 2020-12 for declared input/output schemas while
   leaving invocation-time enforcement to `#172`.
4. Freeze Credential Binding behavior for `form_fill`, `cookie_seed`,
   `storage_seed`, and `totp_fill` where supported, including redaction.
5. Document required project, context, egress, workspace, extension, recording,
   and capability bindings.
6. Add a deterministic multi-file example workflow with positive and negative
   regression scenarios.
7. Keep API, CLI, Admin-New, OpenAPI, README, and architecture descriptions
   aligned.

## Non-Goals

- Workflow Endpoint invocation, grants, idempotency, or run outcomes (`#172`).
- Teach Mode, training, generation, or repair (`#171`).
- Human Handoff (`#71`).
- Additional executors, source uploads, or a package marketplace.
- Runtime input/output enforcement beyond package publication; `#172` owns
  invocation-time enforcement.

## Dependencies And Risks

- Foundation validation, auth, source hardening, store parity, and API
  conformance are merged.
- Existing workflow versions may contain unsupported executor strings; define
  compatibility and migration behavior before enforcing the allowlist.
- Never resolve or persist target-site secrets in source, package metadata,
  logs, or previews.

## Contract Changes

- API/OpenAPI: constrain the version executor/package metadata and document
  stable validation problems without changing owner auth semantics.
- Protocol/event schemas: no remote-stream protocol change; existing workflow
  run events must continue to identify the immutable workflow version.
- Database/migrations: preserve existing versions; add only additive metadata or
  migration behavior required to validate newly published versions.
- Admin-New: create/publish/inspect surfaces must present the same source commit,
  entrypoint, schemas, resource bindings, and validation failures.
- CLI: expose the same supported executor/package fields and stable failures.
- Deployment/configuration: keep Git source access and worker-image assumptions
  explicit; do not add uploaded source archives or another executor image.
- README/ARCH/AGENTS/operator docs: describe one supported package model and
  keep Phase 0 versus future authoring claims explicit.

## Security And Data Impact

- Resolve source inside the approved Git root and keep existing path/symlink
  containment.
- Never render or persist resolved Credential Binding values in source preview,
  package metadata, logs, events, errors, or examples.
- Treat declared project, workspace, extension, context, egress, and credential
  ids as authorization inputs, not proof that the caller may use them.
- Bound source metadata, schema size/complexity, file count, and preview output.

## Migration, Compatibility, And Rollback

- Existing immutable versions remain readable and executable under their stored
  contract.
- Enforcement applies to new publication unless an explicit compatibility
  migration is proven safe for existing rows.
- Unsupported legacy executor values must produce a visible compatibility state
  rather than being silently rewritten.
- A rollback may remove new publication enforcement only if it leaves already
  published version data readable and does not mutate immutable records.

## Observability And Operator Feedback

- Emit bounded validation/audit evidence for publication success and denial,
  source resolution, and executor selection without source or secret contents.
- Admin-New and CLI errors must identify the invalid field and remediation.
- Existing workflow run/session correlation remains authoritative; this slice
  does not create a second package telemetry model.

## Implementation Slices

1. Audit workflow version resources, worker assumptions, CLI, Admin-New,
   examples, and OpenAPI for contract drift.
2. Add a typed supported-executor boundary and stable validation errors.
3. Freeze package/source/schema/resource and credential-injection contracts.
4. Add or update the deterministic example package and regression scenarios.
5. Align API, CLI, Admin-New, OpenAPI, README, ARCH, and support statements.
6. Run focused plus Compose workflow validation and record evidence on `#47`.

## Test Strategy

### Unit

- supported/unsupported executor validation,
- Git source and resolved commit requirements,
- entrypoint/root/path rejection,
- schema dialect and bounded metadata validation,
- credential contract/redaction mapping.

### Integration

- in-memory/Postgres publication parity,
- source resolution and immutable version persistence,
- worker execution from the pinned multi-file snapshot,
- API/CLI/Admin-New resource-shape consistency.

### Smoke And E2E

- publish and inspect the example from a mutable ref resolved to a commit,
- execute it in a fresh session,
- prove credential values remain absent from public evidence,
- reject unsupported executor, invalid source, and unsafe entrypoint,
- update source and prove the original version remains immutable.

### Coverage And Quality

- Gateway validation/store/API changes require focused Rust unit and contract
  coverage plus formatting and clippy.
- Admin-New, CLI, and worker changes require package tests, type checks, builds,
  and affected coverage ratchets.
- Run the existing source, credential, workspace, extension, failure,
  cancellation, reconnect, and Compose workflow smokes touched by the contract.

## Post-Implementation Smoke Sequence

1. Start the supported local Compose stack and authenticate.
2. Publish the deterministic Git-backed Playwright example.
3. Verify the resolved commit, entrypoint, schemas, requirements, and files
   through API, CLI, and Admin-New.
4. Execute valid input in a fresh session and inspect output/evidence.
5. Exercise invalid package metadata, unsupported executor, source path, and
   credential-denial cases.
6. Update the Git ref and verify the existing version remains pinned while a
   new publication receives a new version.
7. Run existing workflow, credential, workspace, extension, reconnect,
   cancellation, failure, CLI, and Admin-New smokes affected by the contract.

## Definition Of Done

- Playwright TypeScript and Git commit-pinned publication are explicit and
  enforced as the Phase 0 package contract.
- One deterministic example proves publication, inspection, execution, and
  immutable update behavior.
- API, CLI, Admin-New, OpenAPI, README, and ARCH do not describe competing
  workflow package models.
- `#172` can consume the published version without inventing another package
  contract.

## Documentation And Claim Impact

- Update README and ARCH if package publication or runtime assumptions change.
- Update OpenAPI and generated Admin-New API/coverage evidence for contract
  changes.
- Keep the capability at Prototype until the bounded #172/#174 Phase 0 evidence
  passes; completing #47 alone is not a Pilot-ready claim.

## Evidence Record

Record the PR, commit, migration/compatibility decision, test and coverage
results, Compose smoke run, deterministic example commit, and reviewed residual
risks on issue `#47`.
