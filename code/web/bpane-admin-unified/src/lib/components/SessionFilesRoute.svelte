<script lang="ts">
  import { ArrowLeft, RefreshCw } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import { SessionFileClient } from '$lib/session-files/session-file-client';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionResource } from '$lib/sessions/session-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionFileBindingsPanel from './SessionFileBindingsPanel.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';
  import SessionTransferFilesPanel from './SessionTransferFilesPanel.svelte';

  type SessionFilesRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly sessionId: string;
  };

  type SessionFilesRouteState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | {
        readonly status: 'ready';
        readonly session: SessionResource;
        readonly bindingMutationAllowed: boolean;
        readonly bindingPolicyMessage: string | null;
        readonly allowedWorkspaceIds: readonly string[];
      };

  let { authContext, sessionId }: SessionFilesRouteProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let routeState = $state<SessionFilesRouteState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  const sessionClient = $derived(new SessionCatalogClient(clientOptions()));
  const sessionFileClient = $derived(new SessionFileClient(clientOptions()));
  const workspaceClient = $derived(new FileWorkspaceCatalogClient(clientOptions()));
  const projectClient = $derived(new ProjectCatalogClient(clientOptions()));

  $effect(() => {
    if (sessionId === loadedSessionId) {
      return;
    }
    loadedSessionId = sessionId;
    routeState = { status: 'loading' };
    actionState = { status: 'idle' };
    void loadRoute(sessionId, false);
  });

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  async function loadRoute(requestSessionId = sessionId, showFeedback = true): Promise<void> {
    if (showFeedback) {
      actionState = { status: 'running', label: 'Refreshing session file policy...' };
    }
    try {
      const session = await sessionClient.getSession(requestSessionId);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      const policy = await loadBindingPolicy(session);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      routeState = {
        status: 'ready',
        session,
        bindingMutationAllowed: policy.allowed,
        bindingPolicyMessage: policy.message,
        allowedWorkspaceIds: policy.allowedWorkspaceIds,
      };
      if (showFeedback) {
        actionState = { status: 'success', message: 'Session file policy refreshed.' };
      }
    } catch (error) {
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      const message = adminErrorMessage(error, 'Unexpected session files route error.');
      routeState = { status: 'error', message };
      if (showFeedback) {
        actionState = { status: 'error', message };
      }
    }
  }

  async function loadBindingPolicy(session: SessionResource): Promise<{
    readonly allowed: boolean;
    readonly message: string | null;
    readonly allowedWorkspaceIds: readonly string[];
  }> {
    if (!session.project_id) {
      return { allowed: true, message: null, allowedWorkspaceIds: [] };
    }
    try {
      const project = await projectClient.getProject(session.project_id);
      if (!project.policy.allow_session_file_bindings) {
        return {
          allowed: false,
          message: `Project ${project.name} blocks session file bindings.`,
          allowedWorkspaceIds: project.policy.allowed_file_workspace_ids,
        };
      }
      return {
        allowed: true,
        message: null,
        allowedWorkspaceIds: project.policy.allowed_file_workspace_ids,
      };
    } catch (error) {
      return {
        allowed: false,
        message: adminErrorMessage(error, 'Project file-binding policy could not be loaded.'),
        allowedWorkspaceIds: [],
      };
    }
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-files-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/sessions" data-testid="session-files-back">
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session files</h1>
      <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{sessionId}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void loadRoute()}
      disabled={routeState.status === 'loading' || actionState.status === 'running'}
      data-testid="session-files-policy-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh policy</span>
    </button>
  </header>

  <SessionSubareaNavigation sessionId={sessionId} activeId="files" availableIds={['overview', 'live', 'files', 'recordings']} />

  <ActionFeedback
    state={actionState}
    successTitle="Session files refreshed"
    errorTitle="Session files refresh failed"
    successTestId="session-files-route-action-success"
    errorTestId="session-files-route-action-error"
    runningTestId="session-files-route-action-running"
  />

  {#if routeState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-files-route-loading">
      Loading session file policy...
    </section>
  {:else if routeState.status === 'error'}
    <AdminMessage tone="error" title="Session files unavailable" message={routeState.message} testId="session-files-route-error" />
  {:else}
    <SessionTransferFilesPanel
      client={sessionFileClient}
      sessionId={routeState.session.id}
      transferBlocked={!routeState.session.capabilities.file_transfer}
    />
    <SessionFileBindingsPanel
      sessionClient={sessionFileClient}
      {workspaceClient}
      sessionId={routeState.session.id}
      projectId={routeState.session.project_id}
      mutationAllowed={routeState.bindingMutationAllowed}
      policyMessage={routeState.bindingPolicyMessage}
      allowedWorkspaceIds={routeState.allowedWorkspaceIds}
    />
  {/if}
</div>
