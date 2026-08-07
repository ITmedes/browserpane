<script lang="ts">
  import { ArrowLeft, RefreshCw } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import type { ProjectResource } from '$lib/projects/project-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import { buildSessionPolicyModel } from '$lib/sessions/session-policy-view-model';
  import type { SessionResource, SessionStatus } from '$lib/sessions/session-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

  type SessionPolicyRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly sessionId: string;
  };

  type SessionPolicyRouteState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | {
        readonly status: 'ready';
        readonly session: SessionResource;
        readonly liveStatus: SessionStatus | null;
        readonly project: ProjectResource | null;
        readonly statusError: string | null;
        readonly projectError: string | null;
      };

  let { authContext, sessionId }: SessionPolicyRouteProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let routeState = $state<SessionPolicyRouteState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  const model = $derived(routeState.status === 'ready'
    ? buildSessionPolicyModel(routeState.session, routeState.liveStatus, routeState.project)
    : null);
  const busy = $derived(routeState.status === 'loading' || actionState.status === 'running');

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
      actionState = { status: 'running', label: 'Refreshing session policy...' };
    }
    try {
      const sessionClient = new SessionCatalogClient(clientOptions());
      const session = await sessionClient.getSession(requestSessionId);
      const [statusResult, projectResult] = await Promise.all([
        loadStatus(sessionClient, requestSessionId),
        loadProject(session),
      ]);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      routeState = {
        status: 'ready',
        session,
        liveStatus: statusResult.value,
        project: projectResult.value,
        statusError: statusResult.error,
        projectError: projectResult.error,
      };
      if (showFeedback) {
        actionState = statusResult.error || projectResult.error
          ? { status: 'error', message: 'Effective policy loaded with incomplete supporting evidence.' }
          : { status: 'success', message: 'Session policy refreshed.' };
      }
    } catch (error) {
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      const message = adminErrorMessage(error, 'Unexpected session policy route error.');
      routeState = { status: 'error', message };
      if (showFeedback) {
        actionState = { status: 'error', message };
      }
    }
  }

  async function loadStatus(
    client: SessionCatalogClient,
    requestSessionId: string,
  ): Promise<{ readonly value: SessionStatus | null; readonly error: string | null }> {
    try {
      return { value: await client.getSessionStatus(requestSessionId), error: null };
    } catch (error) {
      return {
        value: null,
        error: adminErrorMessage(error, 'Live session status evidence is unavailable.'),
      };
    }
  }

  async function loadProject(
    session: SessionResource,
  ): Promise<{ readonly value: ProjectResource | null; readonly error: string | null }> {
    if (!session.project_id) {
      return { value: null, error: null };
    }
    try {
      return {
        value: await new ProjectCatalogClient(clientOptions()).getProject(session.project_id),
        error: null,
      };
    } catch (error) {
      return {
        value: null,
        error: adminErrorMessage(error, 'Project policy evidence is unavailable.'),
      };
    }
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-policy-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/sessions" data-testid="session-policy-back">
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session policy</h1>
      <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{sessionId}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void loadRoute()}
      disabled={busy}
      data-testid="session-policy-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh</span>
    </button>
  </header>

  <SessionSubareaNavigation
    {sessionId}
    activeId="policy"
    availableIds={['overview', 'live', 'automation', 'policy', 'files', 'recordings', 'network']}
  />

  <ActionFeedback
    state={actionState}
    successTitle="Session policy refreshed"
    errorTitle="Session policy refresh incomplete"
    successTestId="session-policy-action-success"
    errorTestId="session-policy-action-error"
    runningTestId="session-policy-action-running"
  />

  {#if routeState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-policy-loading">
      Loading effective session policy...
    </section>
  {:else if routeState.status === 'error'}
    <AdminMessage tone="error" title="Session policy unavailable" message={routeState.message} testId="session-policy-error" />
  {:else if model}
    {#if routeState.statusError}
      <AdminMessage tone="warning" title="Live status evidence unavailable" message={routeState.statusError} testId="session-policy-status-warning" />
    {/if}
    {#if routeState.projectError}
      <AdminMessage tone="warning" title="Project policy evidence unavailable" message={routeState.projectError} testId="session-policy-project-warning" />
    {/if}

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-policy-summary">
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3 p-4">
        <div class="min-w-0">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Effective policy evidence</h2>
          <p class="m-0 mt-1 text-sm text-admin-muted">Configured restrictions and runtime-effective session facts are shown separately.</p>
        </div>
        <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.scopeTone)}`} data-testid="session-policy-scope">
          {model.scopeLabel}
        </span>
      </div>
    </section>

    <div class="grid gap-4">
      {#each model.sections as section}
        <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid={section.testId}>
          <div class="border-b border-admin-border p-4">
            <h2 class="m-0 text-base font-semibold text-admin-ink">{section.title}</h2>
            <p class="m-0 mt-1 text-sm text-admin-muted">{section.description}</p>
          </div>
          <dl class="grid gap-px bg-admin-border md:grid-cols-2 xl:grid-cols-3">
            {#each section.facts as fact}
              <div class="min-w-0 bg-admin-panel p-4">
                <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
                <dd class="m-0 mt-2">
                  <span class={`inline-flex max-w-full rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(fact.tone)}`} data-testid={fact.testId}>
                    <span class="min-w-0 break-all">{fact.value}</span>
                  </span>
                  <span class="mt-2 block text-sm leading-5 text-admin-muted" data-testid={`${fact.testId}-description`}>{fact.description}</span>
                </dd>
              </div>
            {/each}
          </dl>
        </section>
      {/each}
    </div>
  {/if}
</div>
