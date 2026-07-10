# Open GitHub Issues Context

Created: 2026-07-10

This document maps the current `docs/` workspace to the live open GitHub
issues for `ITmedes/browserpane`. It is the bridge between the consolidated
local planning docs and the public issue tracker.

Source check:

- fetched through the GitHub API on 2026-07-10,
- open issues excluding pull requests: 20,
- issue range: `#6` through `#142`.

## Issue Roles

| Role | Issues | How to use |
| --- | --- | --- |
| Umbrella tracker | `#6` | Keep open for high-level roadmap context only. Do not use as the implementation issue for feature PRs. |
| Admin-new parent | `#142` | Parent issue for unified admin redesign and promotion. Follow-up PRs can reference it but should not close it until the promotion gate is complete. |
| Focused current admin resource issue | `#124` | Use for the session-template catalog route when that slice is selected. |
| Product/platform backlog | `#20`, `#21`, `#28`, `#30`, `#31`, `#47`, `#66`, `#69`, `#70`, `#71`, `#72`, `#73`, `#74`, `#75`, `#76`, `#79`, `#80` | Keep mapped to the implementation work order. Create narrower issues before coding if the existing issue is too broad for one PR. |

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
| `#142` Unified admin redesign | `ADMIN_NEW_STATUS.md`, `ADMIN_NEW_REQUIREMENTS.md`, `ADMIN_NEW_API_COVERAGE.md`, `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md`, `ADMIN_NEW_MANUAL_CHECKPOINTS.md`, `IMPLEMENTATION_WORK_ORDER.md` | Items 9-19 | Parent redesign issue. Close only after promotion decision is accepted. |

## Docs-To-Issue Context

| Docs file | Primary issue context |
| --- | --- |
| `ADMIN_INTERACTION_REQUIREMENTS.md` | `#20`, `#28`, `#142` |
| `ADMIN_NEW_API_COVERAGE.md` | `#69`, `#70`, `#124`, `#142` |
| `ADMIN_NEW_IMPLEMENTATION_GUARDRAILS.md` | `#142` |
| `ADMIN_NEW_MANUAL_CHECKPOINTS.md` | `#124`, `#142` |
| `ADMIN_NEW_REQUIREMENTS.md` | `#20`, `#21`, `#47`, `#69`, `#124`, `#142` |
| `ADMIN_NEW_STATUS.md` | `#142` |
| `DOMAIN_REQUIREMENTS.md` | `#20`, `#21`, `#28`, `#31`, `#47`, `#69`, `#124`, `#142` |
| `IDENTITY_ACCESS_REQUIREMENTS.md` | `#70`, `#142` |
| `IMPLEMENTATION_WORK_ORDER.md` | all open issues |
| `PROJECT_GOVERNANCE_REQUIREMENTS.md` | `#70`, `#79`, `#80`, `#142` |
| `RESOURCE_LIFECYCLE_REQUIREMENTS.md` | `#21`, `#66`, `#76`, `#80`, `#124`, `#142` |
| `RUNTIME_OPERATOR_REQUIREMENTS.md` | `#66`, `#69`, `#72`, `#74` |
| `SECURITY_RUNTIME_ROADMAP.md` | `#28`, `#66`, `#72`, `#74`, `#75` |
| `VALIDATION_MATRIX.md` | all implementation issues selected from this matrix |
| `REVIEW_FINDINGS_RECONCILIATION.md` | `#72`, plus security/runtime portions of `#28`, `#66`, `#74`, `#75` |
| `REVIEW_FINDINGS_COVERAGE_AUDIT.md` | `#72`, `#75`, and deferred product/platform backlog issues |
| `SOURCE_PLAN_INVENTORY.md` | `#6`, `#142` |
| `LEGACY_DOC_RETENTION_AUDIT.md` | `#6`, `#142` |
| `LEGACY_SECTION_COVERAGE_AUDIT.md` | `#6`, `#142` |
| `NEXT_WORKING_ROADMAP.md` | `#142`, with security cleanup links to `#72`, `#74`, `#75` where no focused issue exists |
| `README.md` | `#6`, `#142` |
| `concept.html` | `#142` as design reference only |

## Issue Gaps Before Implementation

The implementation work order deliberately starts with high-priority security
and validation slices. Some of those slices do not yet have a focused open
GitHub issue. Before coding, create or confirm a canonical issue for each
selected slice:

| Work-order item | Current issue state | Recommended action |
| --- | --- | --- |
| Item 1: token domain separation and URL credential cleanup | No focused open issue. Broadly related to `#72`. | Create a focused issue before implementation. |
| Item 2: shared admin browser auth and web-security hardening | No focused open issue. Broadly related to `#72` and `#142`. | Create a focused issue before implementation. |
| Item 4: browser context import safety | No focused open issue. Broadly related to `#72`. | Create a focused issue before implementation. |
| Item 5: recording artifact finalization boundary | No focused open issue. Related to `#21`. | Either create a focused issue or explicitly scope it as the next `#21` slice. |
| Item 6: gateway lifecycle, health, and readiness | Broadly related to `#66` and `#74`. | Create a focused issue if the first PR is only graceful shutdown/readiness. |
| Item 7: minimal CI and validation ratchet | Broadly related to `#75`. | Create a focused issue if it becomes the active implementation slice. |
| Item 8: Postgres session-control store contract tests | No focused open issue. | Create a focused issue before implementation. |

## Issue Hygiene Notes

- `#142` is still the correct parent for admin-new. Its companion-doc section
  was updated on 2026-07-10 to point at the consolidated docs instead of the
  removed `docs/ADMIN_APP_REDESIGN_FOUNDATION.md`.
- `#6` remains useful as the umbrella tracker, but direct implementation PRs
  should reference a focused issue or create one when the open issue is too
  broad.
- `#31`, `#71`, `#76`, `#79`, and `#80` are represented only at product
  backlog level in the docs. That is intentional for the current admin/security
  priority. If any of them becomes active, create a dedicated requirements file
  or expand the relevant domain doc before implementation starts.
- `#124` is the cleanest current issue for a focused admin-new resource slice.
- `#20`, `#21`, `#28`, `#47`, `#66`, `#69`, `#70`, `#72`, `#73`, `#74`, and
  `#75` are valid but broad. Use a narrower child issue when the selected PR
  would otherwise mix unrelated API, UI, runtime, and documentation changes.
