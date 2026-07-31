# BPANE-00171 Workflow Teach Mode Plan

Issue: `#171` Add Workflow Studio Teach Mode and controlled
demonstration-to-workflow publishing

Status: Phase N productization plan. Not implemented.

## Business Case

BrowserPane can execute immutable, Git-backed workflows against governed
browser sessions, but creating and maintaining those workflows still requires
manual implementation. Phase N needs a repeatable authoring path that lowers
the marginal cost of the second and subsequent workflows without allowing an
AI system to modify production automation without review.

Teach Mode combines:

- a prose specification of intent, constraints, inputs, outputs, and Human
  Gates,
- one or more semantically captured human demonstrations,
- a provider-neutral candidate compiler,
- fresh-context replay and regression evidence,
- explicit Human Review before immutable publication.

The published result is a normal BrowserPane workflow version. Teach Mode does
not fine-tune a model and does not introduce a second workflow execution
contract.

## Example Use Case

An operator describes:

> Log into the supplier portal, select the reporting period, download all
> compliance reports, and return the downloaded file list. Stop for Human
> Review if an MFA or consent challenge appears.

The operator performs that process in a BrowserPane training session and
annotates:

- reporting period as a typed input,
- login data as a Credential Binding,
- downloaded files as outputs,
- the expected report-list state as an assertion,
- MFA or consent as a Human Gate.

BrowserPane generates a Playwright TypeScript candidate, input/output schemas,
policy requirements, assertions, and provenance. It replays the candidate in a
fresh Browser Context. An authorized reviewer can then publish the candidate as
an immutable, source-pinned workflow version.

## Current Baseline

Implemented foundations:

- owner-scoped workflow definitions and immutable versions,
- Git-backed workflow source pinned to a commit,
- TypeScript/Playwright workflow execution,
- input and output schemas,
- session recordings,
- workflow events, logs, produced files, and workspace inputs,
- Credential Bindings and Project policy,
- awaiting-input interventions,
- Human and Agent access to the same BrowserPane session.

Missing:

- training draft and demonstration resources,
- semantic action capture,
- prose specification and annotation resources,
- normalized workflow intermediate representation,
- trace-to-candidate compiler boundary,
- candidate review and replay lifecycle,
- controlled correction and candidate patch lifecycle.

Video recording is supporting evidence only. It cannot replace semantic capture
because pixels do not preserve target identity, assertions, variable intent, or
policy boundaries.

## Product Terms

- **Teach Mode:** the complete authoring and controlled-repair capability.
- **Training Draft:** owner/project-scoped authoring resource containing intent,
  constraints, and candidate state.
- **Demonstration:** a governed BrowserPane session trace plus bounded evidence
  and annotations.
- **Semantic Step:** normalized action with page identity, target descriptor,
  preconditions, action, and expected postcondition.
- **Candidate:** generated but unpublished workflow source, schemas, policy
  requirements, and provenance.
- **Scenario:** named replay case derived from a demonstration or correction.
- **Controlled Repair:** generation and validation of a candidate patch after a
  detected workflow deviation.

Avoid calling the output a trained model. The durable output is a versioned,
tested workflow package.

## Target Lifecycle

Training draft states:

1. `draft`
2. `capturing`
3. `captured`
4. `compiling`
5. `review_required`
6. `validating`
7. `publishable`
8. `published`

Terminal or alternate states:

- `rejected`
- `cancelled`
- `failed`

Controlled repair must create a new candidate lineage. It must not mutate the
currently published workflow version.

## Resource Model

### Training Draft

Required fields:

- id, owner, optional project,
- name and prose specification,
- allowed domains and actions,
- typed input/output intent,
- Human Gate and failure policies,
- data-sensitivity and compiler-provider policy,
- source workflow/version when used for repair,
- lifecycle state and timestamps,
- current candidate and validation summary.

### Demonstration

Required fields:

- draft, session, project, actor, and scenario linkage,
- capture start/stop timestamps,
- normalized semantic steps,
- annotation set,
- bounded screenshot/recording/artifact references,
- redaction and retention state,
- capture warnings and unresolved assumptions.

### Semantic Step

Required fields:

- stable step and page/tab identity,
- action type and timestamp,
- semantic target descriptors,
- bounded selector candidates and confidence,
- relevant pre/post URL and document state,
- input/output/credential/assertion annotations,
- upload/download references,
- optional branch or Human Gate metadata.

Raw pointer movement and repeated keyboard noise should not become workflow
steps.

### Candidate And Validation

Required fields:

- generated human-readable plan,
- source tree and entrypoint,
- input/output schemas,
- Credential Binding, File Workspace, Egress, extension, and session
  requirements,
- assertions, timeout, retry, and intervention policy,
- compiler/provider/template identity,
- source demonstration and scenario provenance,
- validation attempts and results,
- review decision and published workflow version reference.

## Security And Governance Boundary

Teach Mode must:

- omit passwords, cookies, authorization headers, TOTP values, and unrestricted
  payload bodies from traces,
- replace sensitive values with Credential Binding or typed secret references,
- apply Project, Egress, File, Recording, Retention, and actor policy,
- make capture scope and external compiler use explicit,
- redact evidence before storage or compiler invocation,
- deny external compiler providers for unapproved data classes,
- audit capture, compilation, review, validation, publication, and repair,
- keep unpublished candidates inaccessible to unrelated owners/projects.

The compiler receives a minimized, policy-approved representation. It does not
receive unrestricted browser state by default.

## Implementation Slices

### Slice 1: Contract And Threat Boundary

Deliver:

- resource schemas and lifecycle,
- OpenAPI paths and stable error codes,
- retention/deletion semantics,
- actor/project authorization matrix,
- capture and compiler data-flow threat analysis.

Manual checkpoint:

1. Create, inspect, cancel, and delete an empty draft through the API.
2. Confirm cross-owner/project access fails.
3. Confirm invalid lifecycle transitions return stable errors.

### Slice 2: Semantic Demonstration Capture

Deliver:

- training-purpose session linkage,
- page/tab identity,
- semantic action and target capture,
- pre/post state and bounded evidence references,
- normalization and secret-redaction pipeline.

Manual checkpoint:

1. Perform a local deterministic example manually.
2. Inspect the normalized timeline.
3. Confirm pointer noise is removed and secrets are absent.
4. Confirm video remains optional supporting evidence.

### Slice 3: Specification And Annotation Studio

Deliver:

- prose intent editor,
- allowed-domain/action and policy fields,
- input/output, credential, assertion, branch, and Human Gate annotations,
- unresolved-assumption and warning model.

Manual checkpoint:

1. Annotate one captured demonstration.
2. Reload the route and confirm draft state persists.
3. Confirm forbidden or conflicting annotations fail visibly.

### Slice 4: Candidate Compiler

Deliver:

- provider-neutral compiler interface,
- normalized intermediate representation,
- generated plan, TypeScript source, schemas, requirements, and provenance,
- local deterministic compiler fixture for tests,
- explicit external-provider policy gate.

Manual checkpoint:

1. Generate a candidate from the example.
2. Inspect plan, source, schemas, requirements, warnings, and provenance.
3. Confirm a denied external provider receives no payload.

### Slice 5: Review, Replay, And Publication

Deliver:

- source tree and diff review,
- reviewer approval/rejection,
- fresh-context validation with named scenarios,
- positive and negative replay results,
- publication through the existing Git-backed immutable workflow contract.

Manual checkpoint:

1. Validate with a fresh Browser Context.
2. Confirm publication is blocked after a failed scenario.
3. Approve a passing candidate.
4. Run the resulting workflow through the normal workflow API.

### Slice 6: Controlled Repair

Deliver:

- failed-transition detection from run evidence,
- correction-mode Human Handoff,
- correction demonstration capture,
- candidate patch and source diff,
- existing plus new scenario regression,
- explicit promotion into a new workflow version.

Manual checkpoint:

1. Change the deterministic fixture.
2. Observe a controlled workflow failure.
3. Complete the changed step manually.
4. Generate and validate the patch.
5. Confirm the published version remains unchanged until approval.

## Admin-New Surface

Recommended routes:

- `/admin-new/workflows/teach`
- `/admin-new/workflow-training/[draft_id]`

The draft detail should use tabs or route-backed subareas for:

- specification,
- demonstrations,
- annotations,
- candidate plan,
- source/diff,
- scenarios and validation,
- provenance and review.

Do not place the complete Studio inside the compact Operations Overlay.

## API And CLI Direction

The API should own lifecycle and remain usable without the Admin UI. The CLI
should eventually support:

- draft create/get/list/cancel,
- capture start/stop/status,
- candidate compile/status,
- validation start/results,
- review approve/reject,
- publish,
- repair create/status.

The CLI must not print secrets, unrestricted evidence, or compiler payloads.

## Dependencies And Issue Boundaries

- `#20`: reusable page/tab/session inspection primitives.
- `#21`: generalized artifact and evidence resources.
- `#47`: supported workflow source, packaging, publishing, and execution.
- `#71`: Human Handoff and intervention semantics.
- `#171`: Teach Mode lifecycle, semantic capture, candidate generation, replay,
  publication gate, and controlled repair.

Near-term security, validation, and admin promotion work remains ahead of this
Phase N slice. Teach Mode must not displace issues `#145` through `#163`.

## Test Strategy

Unit:

- lifecycle transitions,
- trace normalization,
- selector ranking,
- redaction,
- annotations and conflicts,
- compiler input minimization,
- candidate provenance,
- publication gate.

Integration:

- Postgres resource persistence,
- session and demonstration linkage,
- artifact references and retention,
- fresh-context validation,
- immutable workflow publication,
- controlled repair lineage,
- authorization and policy denial.

Smoke/E2E:

- deterministic local portal fixture,
- manual capture to generated candidate,
- positive and negative replay,
- publication and normal workflow execution,
- drift/correction candidate without automatic mutation,
- external compiler policy denial and secret-redaction checks.

## Post-Implementation Smoke Sequence

1. Start the local stack and log into `/admin-new/`.
2. Create a Project, test Credential Binding, and File Workspace.
3. Create a Teach Mode draft with prose intent, allowed domain, typed
   input/output, and Human Gate definitions.
4. Start capture and perform the local deterministic portal workflow.
5. Annotate an input, credential, output, assertion, and Human Gate.
6. Stop capture and verify the normalized timeline contains no raw secret.
7. Generate and review the candidate plan, schemas, source tree, requirements,
   warnings, and provenance.
8. Replay in a fresh Browser Context and confirm the expected output.
9. Run invalid-input, missing-element, denied-domain, and denied-file-policy
   cases.
10. Approve and publish the passing candidate.
11. Execute the immutable version through the normal workflow API.
12. Change the fixture, capture a correction, and generate a candidate patch.
13. Confirm the published version remains unchanged until explicit approval.
14. Verify API/CLI authorization, retention, audit, and external-provider denial
   paths.

## Exit Criteria

Teach Mode is Phase N-ready when:

- a second workflow can be authored from prose plus demonstrations without a
  separate execution contract,
- every published version is reproducible and source-pinned,
- multiple demonstrations are retained as regression scenarios,
- sensitive data is demonstrably excluded from trace/compiler boundaries,
- controlled repair produces an approved candidate version rather than
  mutating production,
- API, Admin-New, CLI, unit, integration, smoke, and negative security coverage
  are all present.
