# Implementation Steps

Each step has a manual checkpoint. Do not move to the next step until the
focused validation for the current step is accepted.

## Step 0: Baseline

1. Stay on `feature/BPANE-00142`.
2. Keep `concept.html` local/untracked unless explicitly requested.
3. Leave generated `dev/certs/` drift unstaged.
4. Run current `/admin/` baseline checks.

## Step 1: New App Beside Current Admin

1. Scaffold `code/web/bpane-admin-unified`.
2. Use static SvelteKit output.
3. Set base path through `BPANE_ADMIN_BASE_PATH=/admin-new`.
4. Preserve current toolchain shape: Svelte 5, SvelteKit, Vite, TypeScript,
   Tailwind 4, `lucide-svelte`, Vitest.
5. Add Docker build stage for the new package.
6. Add nginx `/admin-new/` redirect and SPA fallback.
7. Keep `/admin/`, `/dist/`, `/auth-config.json`, `/cert-hash`,
   `/cert-fingerprint`, and `/api/` unchanged.

## Step 1A: API Coverage Baseline

1. Add an API coverage manifest generated from `openapi/bpane-control-v1.yaml`.
2. Classify every operation.
3. Add route/link helpers and client-wrapper tests.
4. Add compatibility endpoint manifest.
5. Add schema property-level generated parity.

## Step 2: Dashboard

1. Add `/admin-new/`.
2. Use real read-only data.
3. Show active sessions, recent workflow runs, and resource counts.
4. No new mutations except navigation.

## Step 3: Sessions Catalog

1. Add `/admin-new/sessions`.
2. Use a focused sessions list plus selected-session metadata area.
3. Preserve join/reconnect/disconnect behavior.
4. Keep current session switch behavior explicit.

## Step 4: Session Detail Overview

1. Add `/admin-new/sessions/[session_id]`.
2. Add route-backed overview.
3. Preserve lifecycle actions, stop eligibility, queue state, connections,
   disconnect all, and per-connection disconnect.

## Step 5: Live Tab

1. Add `/admin-new/sessions/[session_id]/live`.
2. Reuse the browser SDK from `/dist/`.
3. Use container-based viewport sizing.
4. Preserve attach/detach, upload, microphone, camera, display controls, and
   trust guidance.

## Step 6: Session Files

1. Add `/admin-new/sessions/[session_id]/files`.
2. Preserve session files and session file bindings.
3. Preserve workspace-file binding and download behavior.

## Step 7: Recordings

1. Add `/admin-new/sessions/[session_id]/recordings`.
2. Preserve recording status, retained segments, downloads, playback manifest,
   and playback export.

## Step 8: Network

1. Add `/admin-new/sessions/[session_id]/network`.
2. Preserve network identity, effective egress, diagnostics, and probes.
3. Link to egress profile detail.

## Step 9: Automation

1. Add `/admin-new/sessions/[session_id]/automation`.
2. Preserve MCP delegation, workflow associations, automation owner/delegate
   state, and API companion separation for worker routes.

## Step 9A: Browser Policy

1. Add `/admin-new/sessions/[session_id]/policy`.
2. Preserve local-file and File System Access guardrails.
3. Preserve probe command and CDP endpoint evidence.

## Step 10: Observability

1. Add `/admin-new/sessions/[session_id]/observability`.
2. Preserve logs, metrics summaries, admin event stream state, workflow
   snapshots, recording snapshots, and future observability placeholders.

## Step 11: Create Session Flow

1. Add `/admin-new/sessions/new`.
2. Preserve template selection, project selection, browser context modes,
   network identity, egress profiles, labels, idle timeout, owner mode, and API
   payload preview.

## Step 12: Resource Catalogs

Add route-backed catalogs and detail views for:

- Projects
- Browser contexts
- Egress profiles
- File workspaces
- Workflows and workflow runs
- Session templates
- Extensions
- Credential bindings
- Workflow event subscriptions
- Operation counters and internal automation evidence
- Identity and access review

## Step 13: Command Palette

1. Add command palette to the new shell.
2. Support navigation, session join, and common creation actions.
3. Do not create a second hidden state model.

## Step 14: API Reference And Coverage Companion

1. Add `/admin-new/api`.
2. Link OpenAPI.
3. Show operation classification and copyable examples.
4. Keep compatibility endpoints separated from the frozen API.

## Step 15: Promotion Decision

1. Compare against current app parity.
2. Keep `/admin/` and `/admin-new/` side by side.
3. Promote only after smoke and manual parity pass.
