# Open GitHub Issues Context

Created: 2026-07-10

This document maps the current `docs/` workspace to the live open GitHub
issues for `ITmedes/browserpane`. It is the bridge between the consolidated
local planning docs and the public issue tracker.

Source check:

- fetched through the GitHub API on 2026-07-10,
- open issues excluding pull requests: 45 after the reverse docs-to-issues
  audit,
- open issue range: `#6` through `#170`,
- focused docs-derived implementation issues created on 2026-07-10: `#145`
  through `#170`,
- cross-reference pass on 2026-07-10 verified that every open issue has a
  docs source or docs cross-reference section and links back to this file,
- closed admin-redesign lineage issue: `#142`, closed as completed on
  2026-07-07 and updated on 2026-07-10 to point at the consolidated docs.

## Issue Roles

| Role | Issues | How to use |
| --- | --- | --- |
| Umbrella tracker | `#6` | Keep open for high-level roadmap context only. Do not use as the implementation issue for feature PRs. |
| Focused current admin resource issue | `#124` | Use for the session-template catalog route when that slice is selected. |
| Focused docs-derived work-order issues | `#145` through `#170` | Use these as canonical implementation issues for work-order items 1 through 26. |
| Product/platform backlog | `#20`, `#21`, `#28`, `#30`, `#31`, `#47`, `#66`, `#69`, `#70`, `#71`, `#72`, `#73`, `#74`, `#75`, `#76`, `#79`, `#80` | Keep as roadmap and enterprise/product context. Prefer the matching focused issue from `#145`-`#170` when an implementation slice is covered there. |
| Closed admin redesign lineage | `#142` | Historical design record for admin-new. It is not open; route remaining admin implementation slices through the relevant existing open issue. |

## Open Issue Matrix

| Issue | Current docs context | Work-order owner | Priority interpretation |
| --- | --- | --- | --- |
| `#6` Epic: Integration-ready control plane and focused issue tracker | `README.md`, `SOURCE_PLAN_INVENTORY.md`, `IMPLEMENTATION_WORK_ORDER.md` | Entire work order | Umbrella only. Use focused issues for implementation. |
| `#20` Per-session observability, logs, and tab/page inspection APIs | `DOMAIN_REQUIREMENTS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` | Items 11, 12, 20, 22 | Active admin-new parity and later API model work. |
| `#21` Artifact, browser-output, and recording export APIs | `DOMAIN_REQUIREMENTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md` | Items 5, 11, 21, 27 | Recording boundary is near-term; generalized artifact model is later. |
| `#28` Resource event subscriptions and security-event export | `SECURITY_RUNTIME_ROADMAP.md`, `DOMAIN_REQUIREMENTS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md` | Items 3, 12, 21, 27 | Webhook SSRF is near-term; generalized events/security export are later. |
| `#30` Debug and support bundles | `REVIEW_FINDINGS_COVERAGE_AUDIT.md`, `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md` | Items 22, 27 | Deferred production/support packaging. |
| `#31` Mobile and device-mode sessions | `IMPLEMENTATION_WORK_ORDER.md`, `DOMAIN_REQUIREMENTS.md` | Item 27 | Deferred product capability. Needs dedicated requirements if promoted. |
| `#47` Workflow publishing and supported execution interfaces | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 10, 15, 18, 21 | Partly admin-new parity, partly later workflow productization. |
| `#66` Compose, Kubernetes, and AWS Fargate deployments | `RUNTIME_OPERATOR_REQUIREMENTS.md`, `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 6, 18, 22, 23, 27 | Health/readiness and docs are nearer term; full deployment packaging is later. |
| `#69` Session-scoped automation connection APIs | `DOMAIN_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 12, 18, 27 | Session automation route visibility is admin parity; productized external contract is later. |
| `#70` API key, audit log, and retention policy controls | `IDENTITY_ACCESS_REQUIREMENTS.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 13, 17, 27 | Identity route can surface current access review now; API keys/audit are enterprise backlog. |
| `#71` Signed human handoff, challenge detection, and private fallback | `IMPLEMENTATION_WORK_ORDER.md`, `DOMAIN_REQUIREMENTS.md` | Item 27 | Deferred product capability. Needs dedicated requirements if promoted. |
| `#72` Enterprise security hardening baseline and threat model | `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 1, 2, 3, 4, 6, 22, 23 | Active security cleanup plus later threat-model documentation. |
| `#73` Backup, restore, and disaster recovery runbooks | `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` | Items 22, 27 | Deferred production runbook work. |
| `#74` High availability and zero-downtime operations | `SECURITY_RUNTIME_ROADMAP.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 6, 20, 23, 27 | Graceful shutdown/readiness are near-term; full HA is later. |
| `#75` Supply chain security and release governance | `REVIEW_FINDINGS_COVERAGE_AUDIT.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 7, 22, 27 | CI/validation ratchet is near-term; release governance is later. |
| `#76` Data residency, encryption, and BYOK controls | `IMPLEMENTATION_WORK_ORDER.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | Item 27 | Deferred enterprise storage/security capability. |
| `#79` Central enterprise policy engine | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 17, 27 | Current project-policy UX is nearer term; central policy engine is later. |
| `#80` DLP and content inspection hooks | `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 17, 27 | Current file policy visibility is nearer term; DLP provider hooks are later. |
| `#124` Admin session template catalog management | `DOMAIN_REQUIREMENTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | Item 15 | Focused admin-new resource slice. Use as canonical issue for template catalog work. |

## Focused Work-Order Issue Matrix

Created on 2026-07-10 from the reverse docs-to-issues audit. These issues
cover concrete topics that were present in `docs/` but did not yet have
dedicated open issue ownership.

| Work item | Issue | Docs source |
| --- | --- | --- |
| 1. Token domain separation and URL credential cleanup | `#145` Add token domain separation and URL credential cleanup | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 2. Shared admin browser auth and web-security hardening | `#146` Harden shared admin auth and browser security | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 3. Webhook SSRF controls | `#147` Add webhook SSRF controls for event delivery | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| 4. Browser context import safety | `#148` Harden browser context import safety | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md` |
| 5. Recording artifact finalization boundary | `#149` Harden recording artifact finalization boundary | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 6. Gateway lifecycle, health, and readiness | `#150` Add gateway lifecycle, health, and readiness endpoints | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` |
| 7. Minimal CI and validation ratchet | `#151` Add minimal CI and validation ratchet | `IMPLEMENTATION_WORK_ORDER.md`, `VALIDATION_MATRIX.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 8. Postgres session-control store contract tests | `#152` Add Postgres session-control store contract tests | `IMPLEMENTATION_WORK_ORDER.md`, `VALIDATION_MATRIX.md`, `SECURITY_RUNTIME_ROADMAP.md` |
| 9. Admin-new pattern, API client, and feedback consolidation | `#153` Consolidate admin-new patterns, API client, and feedback handling | `IMPLEMENTATION_WORK_ORDER.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `ADMIN_INTERACTION_REQUIREMENTS.md` |
| 10. Route-backed workflow run detail | `#154` Add route-backed admin-new workflow run detail | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_REQUIREMENTS.md` |
| 11. Session subareas phase 1 | `#155` Add admin-new session subareas for live, files, recordings, and network | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 12. Session subareas phase 2 | `#156` Add admin-new session automation, policy, and observability subareas | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 13. Identity and access review route | `#157` Add admin-new identity and access review route | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `IDENTITY_ACCESS_REQUIREMENTS.md` |
| 14. API companion, coverage, and docs routes | `#158` Add admin-new API companion, coverage, and docs routes | `IMPLEMENTATION_WORK_ORDER.md`, `NEXT_WORKING_ROADMAP.md`, `ADMIN_NEW_API_COVERAGE.md` |
| 15. Missing resource catalogs except session templates | `#159` Add admin-new catalogs for extensions, credential bindings, and workflow event subscriptions | `IMPLEMENTATION_WORK_ORDER.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`; session templates remain `#124` |
| 16. Browser context clone/import/export UI parity | `#160` Add browser context clone, import, and export admin-new parity | `IMPLEMENTATION_WORK_ORDER.md`, `RESOURCE_LIFECYCLE_REQUIREMENTS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 17. Project governance evidence and cross-resource policy UX | `#161` Add project governance evidence and cross-resource policy UX | `IMPLEMENTATION_WORK_ORDER.md`, `PROJECT_GOVERNANCE_REQUIREMENTS.md`, `ADMIN_NEW_REQUIREMENTS.md` |
| 18. Operator CLI resource parity and local setup docs | `#162` Add operator CLI resource parity and local setup diagnostics docs | `IMPLEMENTATION_WORK_ORDER.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md`, `VALIDATION_MATRIX.md` |
| 19. Admin-new promotion gate | `#163` Define admin-new promotion gate and fallback plan | `IMPLEMENTATION_WORK_ORDER.md`, `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md` |
| 20. Admin and session catalog scalability | `#164` Improve admin and session catalog scalability | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 21. Worker and archive runtime hygiene | `#165` Harden worker logs, polling, and archive runtime behavior | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| 22. Documentation accuracy and production references | `#166` Update documentation accuracy and production references | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 23. Docker runtime launch boundary | `#167` Define Docker runtime launch boundary for production hardening | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `RUNTIME_OPERATOR_REQUIREMENTS.md` |
| 24. Host and client render hot-path work | `#168` Profile and optimize host and client render hot paths | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_COVERAGE_AUDIT.md` |
| 25. Gateway fan-out and transport optimization | `#169` Profile and optimize gateway fan-out and transport behavior | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |
| 26. Structural refactors | `#170` Split session-control structural refactors by domain | `IMPLEMENTATION_WORK_ORDER.md`, `SECURITY_RUNTIME_ROADMAP.md`, `REVIEW_FINDINGS_RECONCILIATION.md` |

## Issue Body Audit

Checked again on 2026-07-10 against the live issue bodies, current
`/admin-new/` routes, and the consolidated docs. The first pass covered the
original 19 open issues; the reverse docs-to-issues pass created focused
issues `#145` through `#170`.

The original 19 open issues remain relevant. None should be closed only
because of the docs consolidation or because `#142` is closed. The focused
issues now own the concrete work-order slices; the original broad issues remain
roadmap and enterprise/product context.

The cross-reference pass also updated the issue bodies in both directions:

- every broad roadmap issue now has a `Docs cross-reference` section with
  source docs, the central issue map, and related focused issues where
  applicable,
- every focused work-order issue from `#145` through `#170` has both `Docs
  source` and `Docs cross-reference` sections,
- every open issue links back to `docs/OPEN_ISSUES_CONTEXT.md`,
- stale wording that told slices to stay on broad issues was removed from the
  issue bodies.

| Issue | Current relevance | GitHub issue-body action | Admin-new reference status |
| --- | --- | --- | --- |
| `#6` | Relevant as umbrella tracker only. | Updated on GitHub 2026-07-10 to remove active-looking references to completed slices and point at this consolidated docs workspace. | Now mentions `#142` as closed lineage and `/admin-new/` as active side-by-side app, but does not own admin implementation PRs. |
| `#20` | Relevant. Session observability remains missing as a coherent API and route-backed inspector model. | Updated on GitHub 2026-07-10 with `/admin-new/` session-inspector alignment and the existing-issue scoping rule. | Admin-new owner is the future session detail subareas for observability, logs, tabs, pages, and diagnostics. |
| `#21` | Relevant. Recording export exists partly; generalized artifacts/browser outputs remain broader work. | Updated on GitHub 2026-07-10 with `/admin-new/recordings` and artifact-route alignment. | `/admin-new/recordings` exists, but session-scoped artifacts/files/download detail routes are still missing. |
| `#28` | Relevant. Workflow webhooks exist; generalized resource events and security export remain. | Updated on GitHub 2026-07-10 with `/admin-new/` event-subscription and delivery-health alignment. | Admin-new should eventually expose event subscriptions and delivery health, but this is not yet implemented. |
| `#30` | Relevant as production/support backlog. | Updated on GitHub 2026-07-10 with `/admin-new/` support-bundle operator-surface alignment. | Support bundle generation/download should eventually be an admin-new operator route. |
| `#31` | Relevant but deferred product capability. | Updated on GitHub 2026-07-10 with `/admin-new/` session create/template/detail device-mode alignment. | Device-mode choices would belong in session create, templates, and session detail. |
| `#47` | Relevant, but the body contained stale route-scoping references to completed issues. | Updated on GitHub 2026-07-10 to replace old `#87`/`#89` routing notes with the current docs/work-order ownership. | Admin-new workflow catalog exists; workflow-run detail and deeper run controls remain missing. |
| `#66` | Relevant. Deployment packaging remains broad and should not be one PR. | Updated on GitHub 2026-07-10 with `/admin-new/` routing, auth, event-stream, and smoke-validation alignment. | Admin-new matters as one deployed web route beside `/admin/`, but this issue should stay deployment-focused. |
| `#69` | Relevant. Automation API productization remains broader than current MCP/workflow implementation. | Updated on GitHub 2026-07-10 with `/admin-new/` session-detail and API-companion alignment. | Session automation should surface in session detail and API companion routes. |
| `#70` | Relevant. Identity/access-review exists; API keys, immutable audit, and retention controls remain backlog. | Updated on GitHub 2026-07-10 with `/admin-new/identity` alignment. | Admin-new identity/access route is the near-term UI owner; API-key/audit/retention screens are later. |
| `#71` | Relevant but deferred product capability. | Updated on GitHub 2026-07-10 with `/admin-new/` session/workflow handoff alignment. | Admin-new handoff controls would belong in session/workflow detail. |
| `#72` | Relevant. Several high-priority security cleanup slices map here. | Updated on GitHub 2026-07-10 with admin-new promotion-blocker alignment and the existing-issue scoping rule. | Admin-new security hardening is promotion-blocking, especially shared auth, event auth, CSP/frame policy, and URL credential cleanup. |
| `#73` | Relevant but deferred production runbook work. | Updated on GitHub 2026-07-10 with `/admin-new/` restore-validation alignment. | Restore validation should include route availability and admin-new smoke checks. |
| `#74` | Relevant. Graceful shutdown/readiness is nearer term; full HA is later. | Updated on GitHub 2026-07-10 with `/admin-new/` event-stream continuity and readiness alignment. | Admin-new should be part of event-stream continuity and route availability validation. |
| `#75` | Relevant. Minimal CI/validation ratchet is nearer term; release governance is later. | Updated on GitHub 2026-07-10 with `bpane-admin-unified` build/test/smoke alignment. | Admin-new build, unit tests, and focused smoke tests must be included in any validation ratchet. |
| `#76` | Relevant but deferred enterprise storage/security capability. | Updated on GitHub 2026-07-10 with `/admin-new/` effective-policy display alignment. | Effective residency/encryption policy belongs in admin-new once implemented. |
| `#79` | Relevant but broad. Current project-policy visibility is nearer term than a central policy engine. | Updated on GitHub 2026-07-10 with `/admin-new/` effective-policy diagnostics alignment. | Admin-new policy diagnostics should be part of the eventual effective-policy surface. |
| `#80` | Relevant but broad. Current file-policy visibility is nearer term than pluggable DLP. | Updated on GitHub 2026-07-10 with `/admin-new/` file/artifact scan-state alignment. | Admin-new file/artifact indicators and filters are the expected operator UI. |
| `#124` | Relevant and focused. This is the cleanest current admin-new implementation issue. | Updated on GitHub 2026-07-10 to name `/admin-new/` explicitly as the target surface. | Admin-new session-template catalog route is missing and should be built when this issue is selected. |

## Docs-To-Issue Context

Open issues are listed as `#N`. The historical closed admin redesign lineage is
shown as `closed #142`.

| Docs file | Primary issue context |
| --- | --- |
| `ADMIN_INTERACTION_REQUIREMENTS.md` | `#20`, `#28`, `#153`, `#156`, closed `#142` |
| `ADMIN_NEW_API_COVERAGE.md` | `#69`, `#70`, `#124`, `#153`, `#158`, closed `#142` |
| `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` | `#153`, `#163`, closed `#142` |
| `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | `#124`, `#154`-`#163`, closed `#142` |
| `ADMIN_NEW_REQUIREMENTS.md` | `#20`, `#21`, `#47`, `#69`, `#124`, `#153`-`#159`, `#161`, `#163`, closed `#142` |
| `ADMIN_NEW_STATUS.md` | `#153`, `#163`, closed `#142` |
| `DOMAIN_REQUIREMENTS.md` | `#20`, `#21`, `#28`, `#31`, `#47`, `#69`, `#124`, `#154`-`#157`, `#159`, `#161`, closed `#142` |
| `IDENTITY_ACCESS_REQUIREMENTS.md` | `#70`, `#157`, closed `#142` |
| `IMPLEMENTATION_WORK_ORDER.md` | all open issues, with focused ownership for items 1-26 in `#145`-`#170` |
| `OPEN_ISSUES_CONTEXT.md` | all open issues, plus closed `#142` lineage |
| `PROJECT_GOVERNANCE_REQUIREMENTS.md` | `#70`, `#79`, `#80`, `#161`, closed `#142` |
| `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | `#21`, `#66`, `#76`, `#80`, `#124`, `#148`, `#159`, `#160`, closed `#142` |
| `RUNTIME_OPERATOR_REQUIREMENTS.md` | `#66`, `#69`, `#72`, `#74`, `#150`, `#162`, `#167` |
| `SECURITY_RUNTIME_ROADMAP.md` | `#28`, `#66`, `#72`, `#74`, `#75`, `#145`-`#150`, `#164`-`#170` |
| `VALIDATION_MATRIX.md` | all implementation issues selected from this matrix, especially `#151`, `#152`, and focused admin-new issues `#153`-`#163` |
| `REVIEW_FINDINGS_RECONCILIATION.md` | `#72`, `#145`-`#150`, `#165`, `#169`, `#170`, plus security/runtime portions of `#28`, `#66`, `#74`, `#75` |
| `REVIEW_FINDINGS_COVERAGE_AUDIT.md` | `#72`, `#75`, `#151`, `#164`, `#166`, `#168`, and deferred product/platform backlog issues |
| `SOURCE_PLAN_INVENTORY.md` | `#6`, closed `#142` |
| `LEGACY_DOC_RETENTION_AUDIT.md` | `#6`, closed `#142` |
| `LEGACY_SECTION_COVERAGE_AUDIT.md` | `#6`, closed `#142` |
| `NEXT_WORKING_ROADMAP.md` | `#145`, `#146`, `#149`, `#154`-`#158`, closed `#142` |
| `README.md` | `#6`, closed `#142` |
| `concept.html` | closed `#142` as design reference only |

## Issue Scoping Before Implementation

The implementation work order deliberately starts with high-priority security
and validation slices. After the reverse docs-to-issues audit, work-order items
1 through 26 have focused open issues. Before coding, select the relevant
focused issue and capture the exact slice boundary in a checked-in
`docs/*_PLAN.md` file:

| Work-order item | Current issue state | Recommended action |
| --- | --- | --- |
| Items 1-9: P0/P1 security, runtime, validation, and admin-new foundation | `#145`-`#153` | Use the matching focused issue and a dedicated plan file. |
| Items 10-19: admin-new parity and promotion work | `#154`-`#163`, with session templates still tracked by `#124` | Use the matching focused issue and keep old admin regression scope explicit. |
| Items 20-26: scalability, runtime hygiene, docs, performance, and refactors | `#164`-`#170` | Use the matching focused issue and require validation evidence before broad refactors. |
| Item 27: deferred product and enterprise backlog | Existing broad issues: `#30`, `#31`, `#66`, `#70`, `#71`, `#73`, `#76`, `#79`, `#80` | Keep as product backlog unless promoted into a concrete implementation slice. |

## Issue Hygiene Notes

- `#142` is closed as completed. It remains the historical admin-new design
  lineage, and its companion-doc section was updated on 2026-07-10 to point at
  the consolidated docs instead of the removed
  `docs/ADMIN_APP_REDESIGN_FOUNDATION.md`.
- There is currently no open broad admin-new parent issue. Use `#124` for
  session-template catalog work, and use `#153`-`#163` for other focused
  admin-new implementation slices.
- `#6` remains useful as the umbrella tracker, but direct implementation PRs
  should reference the matching focused issue from `#145`-`#170` where one
  exists, plus the scoped local plan file.
- `#31`, `#71`, `#76`, `#79`, and `#80` are represented only at product
  backlog level in the docs. That is intentional for the current admin/security
  priority. If any of them becomes active, create a dedicated local plan file
  or expand the relevant domain doc before implementation starts.
- `#124` is the cleanest current issue for a focused admin-new resource slice.
- `#20`, `#21`, `#28`, `#47`, `#66`, `#69`, `#70`, `#72`, `#73`, `#74`, and
  `#75` remain valid but broad. Prefer the matching focused issue from
  `#145`-`#170` for implementation PRs when the slice is covered there.
