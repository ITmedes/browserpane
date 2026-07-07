# Concept Mapping

`concept.html` is a local React prototype with mock data. It defines the target
information architecture and interaction direction, not production code.

## Keep From The Concept

Use the concept for:

- persistent shell
- route-oriented navigation
- dashboard entry
- sessions catalog
- selected session detail
- live attach model
- resource catalogs
- global attach banner
- command palette behavior
- API/coverage companion surfaces
- compact, operational layout

Concept route families to map:

- dashboard
- sessions
- contexts
- egress
- runs
- workspaces
- projects
- workflows
- identity
- api
- memo or internal reference
- coverage or API coverage companion

Concept session detail pattern:

- Overview
- Live
- Files

## Add From Current App And API

These are not literal concept strings, but they are required for parity:

- Recordings
- Network and egress diagnostics
- Automation and MCP delegation
- Browser Policy
- Observability
- Session templates
- Extensions
- Credential bindings
- Workflow event subscriptions and deliveries
- Operation counters
- Automation-task evidence

## Exclude Or Defer

Do not directly ship these concept/prototype-only items:

- fake browser titlebar
- mock URL text
- mock data as production defaults
- prototype component names in route names or UI copy
- `ShareTokenForm`, until a backend share/handoff contract exists
- external provider references from the concept memo

## Route Naming Corrections

Use route names that match the product/API, not prototype shorthand:

- `/admin-new/workflow-runs`, not `/admin-new/runs`
- `/admin-new/browser-contexts`, not `/admin-new/contexts`
- `/admin-new/files/workspaces`
- `/admin-new/sessions/[session_id]/live`
- `/admin-new/sessions/[session_id]/recordings`
- `/admin-new/sessions/[session_id]/network`
- `/admin-new/sessions/[session_id]/automation`
- `/admin-new/sessions/[session_id]/policy`
- `/admin-new/sessions/[session_id]/observability`

## UX Direction

The new app should feel like an operational control plane:

- dense but readable
- persistent resource selection
- clear selected-resource metadata
- route-backed tabs instead of hidden overlay state
- explicit loading/error/empty states
- visible action feedback
- stable browser viewport sizing from container dimensions
- no nested cards
- no landing-page hero patterns
