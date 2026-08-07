<script lang="ts">
  import { ArrowLeft, ExternalLink, RefreshCw } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import {
    buildSessionDetailModel,
    type SessionDetailLoadState,
  } from '$lib/sessions/session-detail-view-model';
  import { resolveSessionSubareaRoute, sessionSubareaHref } from '$lib/sessions/session-subarea';
  import type { SessionActionState } from '$lib/sessions/session-overview-view-model';
  import type { SessionStatus } from '$lib/sessions/session-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

  type SessionLiveRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: SessionLiveRouteProps = $props();
  let sessionState = $state<SessionDetailLoadState>({ status: 'idle' });
  let actionState = $state<SessionActionState>({ status: 'idle' });
  const model = $derived(sessionState.status === 'ready'
    ? buildSessionDetailModel(sessionState.session, sessionState.liveStatus)
    : null);
  const routeSessionId = $derived(activeSessionId());
  const busy = $derived(sessionState.status === 'loading' || actionState.status === 'running');

  onMount(() => {
    const route = resolveSessionSubareaRoute(window.location.pathname);
    if (!route || route.activeId !== 'live') {
      sessionState = {
        status: 'error',
        sessionId: 'unknown',
        message: 'Session id is missing from the current live route.',
      };
      return;
    }
    void loadSession(route.sessionId);
  });

  function client(): SessionCatalogClient {
    return new SessionCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadSession(sessionId: string): Promise<void> {
    sessionState = { status: 'loading', sessionId };
    actionState = { status: 'idle' };
    const catalog = client();
    try {
      const session = await catalog.getSession(sessionId);
      sessionState = {
        status: 'ready',
        session,
        liveStatus: await loadSessionStatus(catalog, session.id),
      };
    } catch (error) {
      sessionState = {
        status: 'error',
        sessionId,
        message: adminErrorMessage(error, 'Unexpected live session error.'),
      };
    }
  }

  async function refreshSession(): Promise<void> {
    const sessionId = activeSessionId();
    if (!sessionId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing live session...' };
    const catalog = client();
    try {
      const session = await catalog.getSession(sessionId);
      sessionState = {
        status: 'ready',
        session,
        liveStatus: await loadSessionStatus(catalog, session.id),
      };
      actionState = { status: 'success', message: 'Live session status refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Live session refresh failed.'),
      };
    }
  }

  function openPreviewWindow(): void {
    if (!model?.actions.canConnectPreview) {
      return;
    }
    const sessionId = model.id;
    const popup = window.open(
      `/admin-new/sessions/${encodeURIComponent(sessionId)}/preview`,
      `bpane-session-preview-${sessionId}`,
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    if (popup) {
      popup.focus();
      actionState = { status: 'success', message: 'Browser preview opened.' };
      return;
    }
    actionState = {
      status: 'error',
      message: 'The browser preview popup was blocked. Allow popups for this admin origin and try again.',
    };
  }

  async function loadSessionStatus(catalog: SessionCatalogClient, sessionId: string): Promise<SessionStatus | null> {
    try {
      return await catalog.getSessionStatus(sessionId);
    } catch {
      return null;
    }
  }

  function activeSessionId(): string | null {
    if (sessionState.status === 'ready') {
      return sessionState.session.id;
    }
    if (sessionState.status === 'loading' || sessionState.status === 'error') {
      return sessionState.sessionId === 'unknown' ? null : sessionState.sessionId;
    }
    return null;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-live-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/sessions"
        data-testid="session-live-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Live session</h1>
      {#if routeSessionId}
        <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{routeSessionId}</p>
      {/if}
    </div>
  </header>

  {#if routeSessionId}
    <SessionSubareaNavigation sessionId={routeSessionId} activeId="live" availableIds={['overview', 'live', 'files', 'recordings', 'network']} />
  {/if}

  {#if sessionState.status === 'idle' || sessionState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-live-loading">
      Loading live session status...
    </section>
  {:else if sessionState.status === 'error'}
    <div data-testid="session-live-error">
      <AdminMessage tone="error" title="Live session unavailable" message={sessionState.message} />
    </div>
  {:else if model}
    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-live-panel">
      <div class="flex flex-col gap-4 border-b border-admin-border p-4 md:flex-row md:items-start md:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 font-mono text-xl font-semibold text-admin-ink">{model.shortId}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.stateTone)}`} data-testid="session-live-state">
              {model.state}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{model.subtitle}</p>
        </div>
        <div class="flex shrink-0 flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={() => void refreshSession()}
            disabled={busy}
            data-testid="session-live-refresh"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Refresh</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={openPreviewWindow}
            disabled={busy || !model.actions.canConnectPreview}
            title={model.actions.connectPreviewDescription}
            data-testid="session-live-connect"
          >
            <ExternalLink size={15} strokeWidth={1.8} />
            <span>{model.actions.connectPreviewLabel}</span>
          </button>
        </div>
      </div>

      <div class="p-4">
        <ActionFeedback
          state={actionState}
          successTitle="Live session action completed"
          errorTitle="Live session action failed"
          successTestId="session-live-action-success"
          errorTestId="session-live-action-error"
          runningTestId="session-live-action-running"
        />

        <dl class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" data-testid="session-live-facts">
          <div class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Runtime</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="session-live-runtime">
              {sessionState.liveStatus?.runtime_state ?? sessionState.session.status.runtime_state}
            </dd>
          </div>
          <div class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Clients</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="session-live-clients">
              {sessionState.liveStatus?.connection_counts.total_clients ?? sessionState.session.status.connection_counts.total_clients}
            </dd>
          </div>
          <div class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Interactive</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="session-live-interactive">
              {sessionState.liveStatus?.connection_counts.interactive_clients ?? sessionState.session.status.connection_counts.interactive_clients}
            </dd>
          </div>
          <div class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Resolution</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="session-live-resolution">
              {sessionState.liveStatus ? `${sessionState.liveStatus.resolution[0]} x ${sessionState.liveStatus.resolution[1]}` : 'not reported'}
            </dd>
          </div>
        </dl>

        <section class="mt-4 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-live-connections">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Connections</h3>
          {#if model.connections.length === 0}
            <p class="m-0 mt-3 text-sm text-admin-muted" data-testid="session-live-connections-empty">No live connections reported.</p>
          {:else}
            <div class="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {#each model.connections as connection}
                <div class="rounded-md border border-admin-border bg-admin-panel p-3" data-testid="session-live-connection">
                  <strong class="block font-mono text-sm text-admin-ink">#{connection.id}</strong>
                  <span class="text-xs uppercase text-admin-muted">{connection.role}</span>
                </div>
              {/each}
            </div>
          {/if}
        </section>
      </div>
    </section>
  {/if}
</div>
