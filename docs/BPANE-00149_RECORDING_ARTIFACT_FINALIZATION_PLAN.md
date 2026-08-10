# BPANE-00149 Recording Artifact Finalization Plan

## Metadata

- Issue: [#149](https://github.com/ITmedes/browserpane/issues/149)
- State: In Progress
- Lane: Foundation with a conditional Phase 0 dependency
- Target gate: Foundation Gate; required before recordings are accepted as
  Phase 0 evidence
- Depends on: #145 token-purpose separation, #146 shared authentication, #150
  lifecycle/readiness, and #151 validation baseline (all merged)
- Branch: `feature/BPANE-00149-recording-artifact-finalization`
- Baseline: `main` at `25fdcaa5e4edb54f00353ce776ecd0f9aac8edd9`,
  2026-08-10

## Business Outcome

A recorder worker can finalize only the artifact produced for its assigned
session and recording. BrowserPane no longer treats an arbitrary absolute path
or an ordinary session-automation credential as authority to move a
gateway-local file into retained recording storage. Valid worker recordings
remain downloadable and exportable through the existing operator interfaces.

## Example Use Case

A session configured with `recording.mode=always` launches a recorder worker.
The worker writes
`<staging-root>/<session-id>/<recording-id>.webm`, receives a capability bound
to that session and recording from the gateway, and finalizes the segment after
disconnect. The gateway derives the retained byte count from the staged file,
moves it into managed artifact storage, and marks the segment ready. A caller
holding only ordinary session automation access, or a worker submitting a path
outside staging, through a symlink, or for another session, receives a bounded
authorization or validation error and no file is moved or exposed.

## Current Evidence

- The recording worker already writes one deterministic path under
  `BPANE_RECORDING_OUTPUT_ROOT`:
  `<session-id>/<recording-id>.webm`.
- Gateway and recorder containers share `/tmp/bpane-recordings` in local
  Compose, and finalized artifacts use the separate
  `/tmp/bpane-recording-artifacts` volume.
- `LocalFsRecordingArtifactStore::finalize` currently accepts any non-empty
  absolute `source_path`, then renames or copies it into managed storage.
- The complete and fail routes currently accept ordinary
  `x-bpane-automation-access-token` credentials. An API test explicitly proves
  that behavior.
- The completion request accepts a caller-supplied byte count, so retained-byte
  accounting can diverge from the actual staged file.
- The existing purpose-bound HMAC codec already separates session connect,
  session automation, and admin-event credentials. It is the correct primitive
  for a narrowly scoped recorder-worker capability.
- Recording lifecycle, retention, playback/export, observability, worker, API,
  Compose, compatibility-admin, and admin-new tests already provide a broad
  regression base.

## Scope

- Add a purpose-scoped recording-worker capability bound to one session and one
  recording; issue it only when the gateway launches that worker.
- Require the capability for recording completion and worker-reported failure.
  Owner bearer and ordinary session automation credentials remain valid for
  their documented read/control operations but cannot finalize artifacts.
- Pass the capability to the short-lived worker without logging it and send it
  only on complete/fail requests.
- Configure the local artifact store with both the managed artifact root and
  the trusted recorder staging root.
- Accept only the exact deterministic staged file for the route session,
  recording, and format. Reject empty/relative/aliased paths, escapes,
  mismatched IDs, missing paths, directories, symlinks, and non-regular files.
- Derive retained artifact bytes from filesystem metadata after validation
  instead of trusting request metadata.
- Preserve recording lifecycle, playback, export, retention, project quota,
  admin-new, compatibility-admin, and CLI read/download behavior.
- Update the worker/internal OpenAPI classification and local deployment
  contract.

## Non-Goals

- Replacing the local filesystem artifact store with object storage.
- Generalizing the recording model into the broader artifact API owned by #21.
- Redesigning recording UI, playback format, segmentation, retention, or
  session recording policy.
- Making the recorder worker a general-purpose service principal or granting it
  unrelated workflow/session mutation authority.
- Solving worker polling/log/archive hygiene owned by #165.
- Removing `source_path` from the frozen v1 schema in this slice. It remains an
  internal compatibility field but is constrained to one exact derived path.

## Decisions And Dependencies

1. Reuse `PurposeBoundTokenCodec`; do not add another JWT/HMAC implementation.
2. Use a dedicated token purpose and claims containing `session_id` and
   `recording_id`. A capability is useful only while that recording is in an
   active/finalizing state, which supplies single-recording revocation without
   a second credential registry.
3. Keep ordinary automation access for worker read/connect prerequisites, but
   require the dedicated capability for complete/fail. This is least privilege
   without broad OIDC-role assumptions or a second service-auth stack.
4. Retain `source_path` for compatibility, but compare it to the deterministic
   expected path under the configured staging root before filesystem access.
   Opaque staging IDs can replace the field in a future API version.
5. Reject symlinks rather than resolving them. The worker is trusted to create
   a regular file, and no caller-facing path flexibility is required.
6. Use actual file metadata for retained bytes. Supplied `bytes`, when present,
   becomes an integrity assertion and mismatches fail before persistence.
7. Validate lifecycle state before moving the artifact so retries or terminal
   recordings cannot consume or replace staged files.

## Contract Changes

- API/OpenAPI: complete/fail become internal-worker operations requiring a
  purpose-scoped header in addition to existing route identifiers. The complete
  schema retains `source_path` but documents the exact staging contract and
  actual-byte verification. Error responses cover unauthorized worker
  capability and invalid staged artifacts.
- Protocol/event schemas: N/A; the remote display protocol does not change.
- Database/migrations: N/A unless implementation discovers that persisted
  worker-assignment correlation is required. The planned capability is bound to
  existing session/recording state.
- Admin-new: no interaction change; recording catalog, session recording area,
  downloads, and exports remain regression-covered.
- CLI/SDK: no new command. Existing recording inspection/download/export stays
  unchanged; worker client sends the new internal header.
- Deployment/configuration: `--recording-worker-output-root` becomes the
  authoritative artifact staging root used by both worker launch and artifact
  finalization. It must be absolute, shared with the gateway, and distinct from
  the finalized artifact root.
- README/ARCH/AGENTS/operator docs: document the trusted staging boundary and
  worker-only finalization only where runtime topology or support claims need
  clarification.

## Security And Data Impact

- Authentication: complete/fail no longer accept ordinary automation access as
  sufficient authority.
- Authorization: the worker capability is purpose-, session-, and
  recording-bound and must reject connect, automation, admin-event, wrong
  session, and wrong recording credentials.
- Secrets: the capability is process-local input to one short-lived worker. It
  must not appear in URLs, command output, structured logs, API responses, or
  retained resources.
- Files/artifacts: only one regular staged WebM at the deterministic path can be
  moved. The finalized store continues to expose opaque artifact references.
- Multi-tenant boundary: route IDs, token claims, expected path, and recording
  ownership/state must agree before finalization.
- Usage/quota integrity: actual filesystem bytes become authoritative, closing
  under-reporting and cross-project quota-accounting gaps.
- Availability: invalid staging input fails the recording cleanly without
  reading arbitrary files or corrupting an existing retained artifact.

## Migration, Compatibility, And Rollback

- The owner-facing recording read/download/export contract is unchanged.
- Existing external callers that invoke worker-only complete/fail with an owner
  or automation token will receive `401`; those operations were internal
  lifecycle hooks and are reclassified accordingly.
- The current recorder worker and gateway must be deployed together because the
  worker needs the capability header and the gateway enforces it.
- No stored recording or artifact migration is required.
- Rollback is a coordinated gateway/worker rollback. Already finalized
  artifacts remain readable because artifact references and storage layout do
  not change.
- Startup must reject unusable or overlapping staging/final roots rather than
  accepting a configuration that cannot enforce the boundary.

## Observability And Operator Feedback

- Preserve artifact finalize request/success/failure counters.
- Return bounded error categories for missing/invalid worker capability,
  invalid staging path, missing/non-regular file, metadata mismatch, and store
  failure. Do not echo credentials or full untrusted paths in logs.
- Log session and recording IDs plus a sanitized error category at worker
  finalization failure.
- Keep readiness checks for finalized storage and include staging-root
  usability when recorder workers are enabled.
- Admin surfaces continue to show recording state/error and artifact
  availability; no secret or gateway-local path becomes visible.

## Implementation Slices

1. **Worker capability contract:** add the recording-worker token purpose,
   claims/manager/tests, cross-purpose replay tests, API-state wiring, worker
   launch environment, and worker-client header. Commit boundary: capability
   issuance and transport with routes still covered by focused tests.
2. **Worker-only route authorization:** require and validate the capability on
   complete/fail, bind it to route IDs and active recording state, replace the
   old automation-finalization test with positive/negative worker cases. Commit
   boundary: least-privilege API behavior.
3. **Trusted staging store:** pass the configured output root into the artifact
   store, enforce exact deterministic paths and regular-file/no-symlink rules,
   derive actual bytes, and add exhaustive boundary tests. Commit boundary:
   filesystem and accounting integrity.
4. **Contract and deployment alignment:** update OpenAPI classification,
   examples, Compose wiring, test fixtures, README/ARCH/AGENTS where applicable,
   and generated API coverage evidence. Commit boundary: public repository
   contract alignment.
5. **Battle test and evidence:** run unit, integration, Compose recording,
   admin-new recording, compatibility recording, CLI/API conformance, coverage,
   docs, and affected full validation gates; record evidence in this plan and
   #149. Commit boundary: final evidence only.

## Test Strategy

### Unit

- Capability issue/validate, malformed, tampered, cross-purpose, wrong-session,
  wrong-recording, and terminal-recording cases.
- Exact valid staged path; empty, relative, dot/parent components, outside root,
  wrong session, wrong recording, wrong extension, missing file, directory,
  symlink file, symlink parent, and non-regular file rejection.
- Actual byte derivation and declared-byte mismatch.
- Existing move, cross-device/read-only copy fallback, read, delete, readiness,
  and retention behavior.
- Recording-worker client includes the capability only on complete/fail and
  never serializes it into URL/body/log output.

### Integration

- API owner and ordinary automation tokens can list/get but cannot complete or
  fail recordings.
- Correct worker capability finalizes/fails only its bound recording.
- Staging validation failures do not persist ready metadata or expose content.
- Store completion failure removes no unrelated file and records failure
  observability.
- In-memory/Postgres recording state and project retained-byte accounting remain
  consistent.
- OpenAPI route/security/classification and Axum route conformance stay green.

### Smoke And E2E

- Real recorder-worker Compose flow produces a non-empty WebM, reaches `ready`,
  downloads exact bytes, and generates playback ZIP export.
- Compatibility-admin recording smoke verifies browser-local capture plus
  retained artifact/export behavior.
- Admin-new recording smoke verifies catalog/session detail/download and
  responsive behavior.
- Negative Compose/API smoke submits ordinary automation auth and invalid
  staging correlation and receives bounded rejection without artifact leakage.
- Session disconnect, stop, release, kill, worker exit, and gateway restart
  recording regressions remain green where touched.

### Coverage And Quality

- Run `cargo fmt --all -- --check`, gateway clippy, focused gateway tests, and
  affected workspace tests.
- Run recording-worker typecheck/tests/build and dependency safety.
- Run changed-code coverage for gateway and recording worker where supported;
  record uncovered platform-specific copy/symlink branches explicitly.
- Run OpenAPI lint/conformance and repository document checks.
- Preserve required GitHub checks and the canonical recording/admin Compose
  stages.

## Manual Test Sequence

1. Build the gateway, web image, and recorder-worker image from this branch.
2. Start local Compose and wait for gateway `/healthz` and `/readyz`.
3. Open `/admin-new/`, create a session with recording enabled, and connect.
4. Generate visible browser activity, then disconnect or stop the session.
5. Open the session recording area and global recording catalog; confirm one
   segment reaches `ready`, has non-zero bytes/duration, downloads as WebM, and
   is included in playback ZIP export.
6. Using a session automation token, call the same recording's complete and
   fail routes; confirm both return `401` and the ready artifact is unchanged.
7. In a disposable test session, submit an outside-root, relative,
   wrong-session, directory, and symlink source path with a valid test worker
   capability; confirm bounded rejection and no retained content.
8. Stop, release, kill, and close browser connections in separate disposable
   sessions; confirm every started segment reaches either valid `ready` or an
   explicit terminal `failed` state without orphaned `finalizing` records.
9. Download retained segment and playback export again after gateway restart.
10. Inspect logs and metrics: session/recording correlation is present, worker
    capability and raw unsafe paths are absent, and finalize counters match the
    positive/negative attempts.

## Documentation And Claim Impact

- Update `RISK_REGISTER.md` R-009 and capability maturity only after the full
  gate passes.
- Update `DELIVERY_ROADMAP.md`, `OPEN_ISSUES_CONTEXT.md`, and product release
  gates when #149 moves to Review/Done.
- Update README/ARCH/AGENTS if configuration, topology, or supported recording
  behavior changes materially.
- No investor claim should move beyond prototype evidence solely because this
  boundary is hardened; it is one prerequisite for governed Phase 0 recording
  evidence.

## Definition Of Done

- Ordinary owner/session automation credentials cannot complete or fail a
  worker recording.
- A purpose-scoped worker capability can mutate only its bound active
  recording.
- Finalization accepts only the exact regular staged artifact and rejects all
  named path/file boundary errors.
- Actual artifact bytes drive persisted metadata and project usage.
- Valid worker recording, download, playback/export, retention, and admin
  journeys remain green.
- API/OpenAPI, worker, Compose, README/ARCH/AGENTS, roadmap, risk, maturity,
  issue, and plan evidence are aligned where impacted.
- Required unit, integration, smoke/E2E, coverage, dependency, and document
  gates pass.

## Post-Implementation Smoke Sequence

1. Run focused capability, artifact-store, recording API, lifecycle, retention,
   playback, observability, and project-accounting tests.
2. Run gateway formatting, clippy, focused/full tests, OpenAPI conformance, and
   recording-worker typecheck/tests/build.
3. Run the negative worker finalization API sequence for ordinary automation,
   malformed capability, wrong IDs, and all staged-path/file rejection cases.
4. Build the recorder-worker image and start a clean Compose stack.
5. Run the backend recording artifact/playback smoke and verify exact
   downloadable bytes plus ZIP export.
6. Run compatibility-admin and admin-new recording smokes.
7. Run session stop/disconnect/release/kill and recording restart/recovery
   regressions affected by worker finalization.
8. Run API lint/conformance, repository docs, dependency safety, and the
   applicable canonical validation profile.
9. Inspect sanitized logs, finalize counters, final recording states, and
   retained project bytes.
10. Record exact commands/results, residual risks, and rollback evidence in
    this plan, #149, and the PR.

## Evidence Record

Implementation has not started. Baseline analysis confirms the current
absolute-path and ordinary-automation finalization boundary described above.
