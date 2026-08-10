# BPANE-00162 Operator CLI Parity Plan

## Metadata

- Issue: [#162](https://github.com/ITmedes/browserpane/issues/162)
- State: In progress; implementation slice 1 complete
- Lane: Operator Product
- Target gate: Admin-New Phase 1 Promotion
- Depends on: control API conformance through `#179`, admin-new resource
  catalogs through `#159`, and project governance through `#161`
- Branch: `feature/BPANE-00162`
- Baseline: `main` at `ac9849df09b2b237afd615a28eab40551da78d0f`,
  2026-08-10

## Business Outcome

Operators and automation use one supported `bpane` command surface for the
owner-scoped resources exposed by the control API and admin-new. Commands share
the same profile precedence, explicit token-persistence policy, structured JSON
output, stable exit codes, binary transfer behavior, and diagnostic language.
The CLI also points operators to accurate local checks for certificates, MCP,
workflow source access, Docker runtime access, and optional camera support.

This slice closes operator-surface gaps without creating a second API contract.
The frozen OpenAPI document and gateway behavior remain authoritative, and
worker-only or secret-resolution endpoints stay outside the owner CLI.

## Example Use Case

An operator prepares a project-scoped workflow without opening the admin app.
They initialize a local profile, create a file workspace, upload an input file,
inspect an approved extension and credential-binding metadata, publish a
workflow version, start a run, wait for completion, and download its produced
file. If the local environment is not ready, the documented checks distinguish
an untrusted development certificate, an unavailable MCP bridge, an inaccessible
workflow source, a Docker socket problem, and unavailable camera ingress. No
long-lived bearer token or resolved credential value is printed.

## Current Implementation Evidence

- `scripts/bpane` is the repository-level entrypoint for
  `code/web/bpane-client/scripts/bpane-cli.mjs`.
- The canonical CLI already covers profiles, identity/access review, service
  principals, identity mappings, sessions, session templates, projects,
  egress profiles, reusable browser contexts, and MCP delegation diagnostics.
- CLI profile writes enforce `0600`; token persistence requires
  `--save-token`; flags override environment, which overrides profile values.
- Success and error responses are JSON and usage, authentication, API, and
  unexpected failures have stable exit codes.
- Workflow definitions and runs use a separate `workflow-cli.mjs` with a
  different default URL, no shared profile support, process-exit helpers, and a
  separate error/output contract.
- File workspaces, workspace files, approved extensions, credential bindings,
  workflow event subscriptions, session files/bindings, and recording exports
  have owner APIs and admin-new routes but no canonical `bpane` commands.
- `run-bpane-cli-smoke.mjs` covers the established command families against
  Compose, but not the newer resource catalogs or workflow commands.
- README and ARCH describe both CLIs separately and contain the relevant local
  setup facts, but they do not yet provide one concise diagnostic decision path.

## Scope

- Preserve `scripts/bpane` and the package `bpane` binary as the canonical
  operator entrypoint.
- Add shared CLI request/body/binary helpers where they remove real duplication
  and keep all commands on the existing profile/auth/error contract.
- Add owner-safe command coverage for:
  - file workspaces and workspace-file upload/list/download/delete,
  - workflow definitions, immutable versions, source validation, and source
    inspection,
  - workflow runs, wait/intervention/cancel, logs/events, and produced-file
    download,
  - approved extensions and version publication/state transitions,
  - credential-binding create/list/get metadata without secret resolution,
  - workflow event-subscription lifecycle and delivery diagnostics,
  - read/download-oriented session files, bindings, recordings, and playback
    exports where the owner API supports them.
- Keep `workflow-cli.mjs` temporarily compatible by routing it through shared
  behavior or clearly marking it as a compatibility entrypoint; do not maintain
  two independent implementations.
- Add an explicit command-support inventory tied to the frozen OpenAPI families,
  including reasoned deferrals.
- Expand unit and Compose smoke coverage for representative lifecycle, binary,
  validation, auth, and API error paths.
- Consolidate README and ARCH operator guidance around `./scripts/bpane` and add
  a local diagnostic sequence for certificate, MCP, workflow source, Docker
  socket, and camera failures.

## Explicit Deferrals

- Worker-only recording completion/failure and workflow credential-resolution,
  source-snapshot, and workspace-input content endpoints.
- Secret values from Vault or proxy credential bindings.
- Admin event-stream tokens and long-running observability streaming.
- Legacy automation-task compatibility routes unless a current operator flow
  still depends on them.
- Generalized API-key issuance, organization RBAC, immutable audit export, DLP,
  and Phase N workflow endpoints owned by `#70`, `#176`, `#80`, and `#172`.
- Replacing the established CLI parser with a new framework solely for style.
  The current parser is extensively tested; a migration needs an independent
  measured benefit and compatibility plan.

## Contract Decisions

- The gateway and `openapi/bpane-control-v1.yaml` remain the only HTTP contract.
- New commands use the existing `requestGateway` and `CliError` boundaries.
- `--body-json` and `--body-file` are escape hatches for structured resources;
  ergonomic flags are added only for common, unambiguous operations.
- Binary uploads require an explicit input path. Downloads require an explicit
  output path and report path, byte count, and media type as JSON.
- Commands never print access tokens, signing secrets, resolved credentials,
  proxy secrets, CA material, or file contents to standard output.
- Destructive commands remain explicit and operate on one named resource unless
  a separately bounded dry-run/confirmation contract exists.
- Compatibility aliases may remain, but documentation names one preferred
  command and tests prove aliases do not diverge.

## Implementation Slices

1. **Canonical resource-transfer foundation and file workspaces** (complete):
   add reusable JSON-file and binary-output helpers, file-workspace
   create/list/get plus file list/upload/download/delete, focused unit tests,
   and representative Compose smoke coverage.
2. **Workflow CLI convergence**: move workflow definition/run commands onto the
   canonical profile/auth/error contract, preserve the old workflow entrypoint
   as a thin compatibility wrapper, and cover wait/intervention/artifact paths.
3. **Governance resource parity**: add approved-extension, credential-binding,
   and workflow event-subscription commands with safe metadata and state
   transitions; reject secret-resolution operations.
4. **Session evidence transfers**: add owner-facing session file/binding and
   recording/playback inspection/download commands without exposing worker-only
   mutation routes.
5. **Diagnostics and documentation**: publish the API-family support inventory,
   consolidate README/ARCH examples, and add accurate local certificate, MCP,
   workflow source, Docker socket, and camera diagnostic steps.
6. **Battle test and promotion evidence**: run focused and full CLI tests,
   coverage, typecheck/build, Compose CLI/MCP/workflow/resource smokes, negative
   cases, documentation validation, and compatibility regressions.

Each completed slice is committed independently. The plan and issue are updated
with evidence before PR creation.

## Slice 1 Detailed Steps

1. Extend usage and option validation with file-workspace commands and explicit
   `--body-file`, `--file-name`, `--media-type`, and `--provenance-json` inputs.
2. Reuse the current gateway request boundary for JSON calls and the existing
   binary response boundary for downloads; add only the missing safe upload and
   body-file helpers.
3. Implement workspace create/list/get and file list/upload/download/delete with
   strict positional validation and URL encoding.
4. Add unit tests for request shape, binary integrity, nested provenance,
   missing input/output, malformed JSON, unsupported options, auth failures, and
   API failures.
5. Extend the Compose CLI smoke to create a project-scoped workspace, upload a
   deterministic file, verify metadata/hash behavior, download exact bytes,
   and delete the file. The retained workspace metadata remains attached to the
   archived smoke project because the frozen API has no workspace delete route.
6. Update plan evidence and commit the slice before workflow convergence starts.

## Slice 1 Evidence (2026-08-10)

- Added canonical `file-workspace` create/list/get and nested file
  list/upload/download/delete commands to `./scripts/bpane`.
- Added JSON body-file parsing, exact binary input/output handling, explicit
  media type and safe file-name headers, and object-only provenance metadata.
- Focused CLI suite: 52 tests passed, including exact bytes, URL encoding,
  malformed body files, conflicting body sources, missing files/options,
  provenance validation, header injection, and missing authentication.
- TypeScript check and Node syntax checks passed.
- Live `/healthz` and dependency-aware `/readyz` passed before the Compose
  smoke.
- Expanded `smoke:bpane-cli` passed against local Compose, including
  project-scoped workspace creation, upload/list/download/delete, exact-byte
  comparison, project allowlist linkage, and all existing session/MCP paths.
- README and ARCH now document the canonical file-workspace surface and the
  absence of a workspace metadata delete route. No gateway, OpenAPI, database,
  protocol, support-matrix, or runtime-topology change was required.

## Validation Strategy

### Unit

- Command parsing, repeated options, inline `=` preservation, body-file parsing,
  and unsupported-option rejection.
- Profile precedence, missing profile, explicit token persistence, redaction,
  and `0600` writes remain covered.
- Expected method, encoded path, headers, request body, and result JSON for each
  new command.
- Missing ids/options, malformed JSON, invalid enum/number, unsafe output shape,
  `401`/`403`, `404`, `409`, `422`, and `5xx` error mapping.
- Exact binary upload/download bytes and no content emission to stdout.
- Worker-only and secret-resolution commands fail as usage errors without a
  network request.

### Integration And Compose

- Existing CLI smoke remains green for profile, identity, project, session,
  context, egress, MCP, and cleanup behavior.
- Representative create/get/update/state/delete operations run against the live
  gateway for each newly supported resource family.
- Binary workspace input and workflow output round trips compare exact bytes.
- MCP doctor, strict preflight, repair, and endpoint smoke remain unchanged.
- Workflow CLI compatibility smoke and canonical `bpane workflow` smoke produce
  equivalent run/resource results during migration.

### Documentation

- Every documented command appears in `bpane --help` and is executable through
  `./scripts/bpane`.
- README and ARCH use the canonical command names and actual local ports/routes.
- Local diagnostic links and paths exist and do not recommend printing or
  persisting bearer tokens unnecessarily.
- README, ARCH, AGENTS, OpenAPI, and support-matrix impact is assessed in the
  handoff. OpenAPI changes are not expected.

## Post-Implementation Smoke Sequence

1. Start Compose and wait for `/healthz` and `/readyz`.
2. Log in through local Keycloak and initialize a temporary CLI profile without
   persisting the owner token; confirm the profile file mode is `0600`.
3. Run identity/access review, project list, session list/status, and MCP doctor
   plus strict preflight through `./scripts/bpane`.
4. Create a file workspace, upload a deterministic file, list it, download it,
   compare exact bytes, and delete it.
5. Inspect extension and credential-binding metadata, create a signed workflow
   event subscription, inspect delivery diagnostics, and delete it.
6. Create/inspect a workflow and version, start a project-scoped run, wait for a
   terminal state, inspect logs/events, and download a produced file.
7. Inspect session files/bindings and recording playback metadata; download an
   available artifact/export without invoking worker-only routes.
8. Exercise missing auth, invalid profile, malformed body JSON, missing
   input/output, unavailable resource, policy conflict, and gateway failure.
9. Run CLI unit/coverage/typecheck/build plus `smoke:bpane-cli`, MCP endpoint,
   workflow CLI, workflow workspace, workflow events, file-workspace, recording,
   and affected admin-new compatibility smokes.
10. Follow the documented certificate, MCP, workflow source, Docker socket, and
    camera diagnostic sequence on local Compose and verify recovery guidance.

## Rollback

- Each command family is an additive dispatch branch and can be reverted by
  slice without changing the gateway or persisted resources.
- Keep `workflow-cli.mjs` functional until canonical workflow command parity and
  compatibility smoke evidence are complete.
- Do not remove existing command names or profile fields in this issue.
- No database, OpenAPI, protocol, or runtime migration is expected.
