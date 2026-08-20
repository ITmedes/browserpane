# Workflow Package Authoring

BrowserPane Phase 0 supports one workflow package: a reviewed TypeScript
entrypoint executed by Playwright from Git source pinned to a full commit SHA.
The public executor value is `playwright`; Node 22, Playwright 1.59, control API
`v1`, and package format `browserpane.workflow-package/v1` are the supported
runtime tuple. Additional executors and uploaded source archives are not part
of this contract.

## Author From A Clean Checkout

The deterministic reference package is under
`dev/workflows/phase0-package/`. It contains a TypeScript entrypoint, an
imported module, Draft 2020-12 input/output schemas, stable negative outcomes,
and an example publication request. The matching page is
`dev/web-fixtures/workflow-package-fixture.html`.

1. Clone BrowserPane and check out the reviewed commit.
2. Start the supported local Compose stack and authenticate as an owner.
3. Create a workflow definition with `./scripts/bpane workflow create`.
4. Copy `publish-request.example.json`, replace reviewer/time and every
   resource binding with reviewed values, and preserve the runtime tuple.
5. Validate the source with `workflow validate-source`; review every listed
   file and the resolved commit.
6. Replay the positive and applicable negative cases in a fresh context. Mark
   a scenario `not_applicable` only when the package truly has no such path.
7. Publish with:

   ```bash
   ./scripts/bpane workflow publish <workflow-id> \
     --body-file /absolute/path/to/reviewed-publish-request.json
   ```

8. Inspect with `workflow version get`, Admin-New, or the version API. All
   three show the same commit, schemas, requirements, evidence, compatibility
   state, and warnings.

Publication resolves the supplied Git ref before persistence. The stored
version, package manifest, source commit, and evidence are immutable; a source
change requires another version. Existing pre-contract versions remain
readable as `legacy`. Existing rows with another executor are visibly
`unsupported` and are never rewritten.

## Required Contract

Both `input_schema` and `output_schema` must be JSON objects whose `$schema` is
`https://json-schema.org/draft/2020-12/schema`. BrowserPane bounds each encoded
schema to 64 KiB, 32 nested levels, and 4,096 JSON nodes. Publication-time
validation freezes metadata; Workflow Endpoint invocation-time input/output
enforcement belongs to #172.

The manifest must explicitly declare:

- project binding (a UUID, or `null` for an owner-scoped package);
- fresh or reusable Browser Context selection;
- Egress Profile selection, including explicit `null`;
- browser capabilities and recording policy;
- allowed Credential Binding, extension, and File Workspace UUIDs;
- a 1–3,600 second timeout plus non-empty assertions, safe cancellation
  points, and semantic side-effect checkpoints;
- reviewer, review time, approved decision, fresh-context replay, and one
  result for every Phase 0 scenario kind.

Declared resource IDs are authorization inputs, not proof of access. Session,
project, and run policy still authoritatively validate them when used. Default
session extension IDs must be a subset of the package extension allowlist.

## Credential Binding Contract

Workflow source and package metadata contain binding IDs only. Resolved values
are fetched through the run-scoped worker boundary and remain worker-local:

| Mode | Secret payload consumed by the worker | Worker-local `credentials.apply` result |
| --- | --- | --- |
| `form_fill` | field selectors and values | mode and field count |
| `cookie_seed` | bounded cookie definitions | mode and cookie count |
| `storage_seed` | local/session storage entries | mode and entry counts |
| `totp_fill` | selector plus TOTP secret/config | mode, generated code, timing, and digit count in worker memory |

`credentials.load`, `credentials.generateTotp`, and the `totp_fill` apply
result are privileged in-memory helpers for reviewed workflow code. The apply
result retains the generated code for compatibility with existing reviewed
workflows, but that code is not a safe persisted result. A workflow must never
log, return, upload, or include credential values in errors. The gateway does
not persist resolved
payloads in package metadata, source preview, owner resources, or diagnostics.
Before persistence, the worker also scrubs known resolved payload values and
TOTP-shaped codes from captured stdout, stderr, terminal errors, and structured
output. This is a defense-in-depth boundary, not permission for source to print
secrets.
CLI examples should use `--body-file` for write-only secret creation so values
do not enter shell history.

## Regression And Side Effects

Every publication records `happy_path`, `validation`, `missing_element`,
`authentication_challenge`, `portal_failure`, `runtime_failure`,
`cancellation`, and `ambiguous_post_side_effect`. Happy path must pass. Other
cases may be `not_applicable`, but absence needs review justification outside
the manifest. Use stable error prefixes like the reference package. Place safe
cancellation points before irreversible actions and name checkpoints
immediately after them; an interrupted post-checkpoint outcome must be treated
as ambiguous, not automatically retried.

Package publication does not add a general scheduler, Teach Mode, Human
Handoff, automatic repair, or a package marketplace. The capability remains
Prototype until #172 and #174 provide the bounded Phase 0 endpoint and selected
workflow evidence.
