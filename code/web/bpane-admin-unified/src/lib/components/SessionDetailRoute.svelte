<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import AdminMessage from '$lib/components/AdminMessage.svelte';
  import SessionInspector from '$lib/components/SessionInspector.svelte';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionDetailLoadState } from '$lib/sessions/session-detail-view-model';
  import type { SessionActionState } from '$lib/sessions/session-overview-view-model';
  import type { SessionResource, SessionStatus } from '$lib/sessions/session-types';

  type SessionDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: SessionDetailRouteProps = $props();
  let sessionState = $state<SessionDetailLoadState>({ status: 'idle' });
  let actionState = $state<SessionActionState>({ status: 'idle' });

  onMount(() => {
    const sessionId = currentSessionId();
    if (!sessionId) {
      sessionState = {
        status: 'error',
        sessionId: 'unknown',
        message: 'Session id is missing from the current route.',
      };
      return;
    }
    void loadSession(sessionId);
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
    try {
      const loaded = await loadSessionPair(sessionId);
      sessionState = {
        status: 'ready',
        session: loaded.session,
        liveStatus: loaded.liveStatus,
      };
    } catch (error) {
      sessionState = {
        status: 'error',
        sessionId,
        message: error instanceof Error ? error.message : 'Unexpected session detail error.',
      };
    }
  }

  async function refreshSession(): Promise<void> {
    const sessionId = activeSessionId();
    if (!sessionId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing session...' };
    try {
      const loaded = await loadSessionPair(sessionId);
      sessionState = {
        status: 'ready',
        session: loaded.session,
        liveStatus: loaded.liveStatus,
      };
      actionState = { status: 'success', message: 'Session refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Session refresh failed.',
      };
    }
  }

  async function cancelQueuedSession(): Promise<void> {
    await mutateSession('Cancelling queued session...', 'Queued session cancelled.', (catalog, sessionId) =>
      catalog.cancelQueuedSession(sessionId),
    );
  }

  async function disconnectAllConnections(): Promise<void> {
    await mutateSession('Disconnecting clients...', 'All session clients were disconnected.', async (catalog, sessionId) => {
      await catalog.disconnectAllSessionConnections(sessionId);
      return await catalog.getSession(sessionId);
    });
  }

  async function releaseSessionRuntime(): Promise<void> {
    await mutateSession('Releasing runtime...', 'Session runtime released.', (catalog, sessionId) =>
      catalog.releaseSessionRuntime(sessionId),
    );
  }

  async function stopSession(): Promise<void> {
    await mutateSession('Stopping session...', 'Session stopped.', (catalog, sessionId) => catalog.stopSession(sessionId));
  }

  async function killSession(): Promise<void> {
    await mutateSession('Killing session...', 'Session killed.', (catalog, sessionId) => catalog.killSession(sessionId));
  }

  async function mutateSession(
    runningLabel: string,
    successMessage: string,
    mutation: (catalog: SessionCatalogClient, sessionId: string) => Promise<SessionResource>,
  ): Promise<void> {
    const sessionId = activeSessionId();
    if (!sessionId) {
      return;
    }
    const catalog = client();
    actionState = { status: 'running', label: runningLabel };
    try {
      const session = await mutation(catalog, sessionId);
      const liveStatus = await loadSessionStatus(catalog, session.id);
      sessionState = {
        status: 'ready',
        session,
        liveStatus,
      };
      actionState = { status: 'success', message: successMessage };
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Session action failed.',
      };
    }
  }

  async function loadSessionPair(sessionId: string): Promise<{
    readonly session: SessionResource;
    readonly liveStatus: SessionStatus | null;
  }> {
    const catalog = client();
    const session = await catalog.getSession(sessionId);
    const liveStatus = await loadSessionStatus(catalog, session.id);
    return { session, liveStatus };
  }

  async function loadSessionStatus(catalog: SessionCatalogClient, sessionId: string): Promise<SessionStatus | null> {
    try {
      return await catalog.getSessionStatus(sessionId);
    } catch {
      return null;
    }
  }

  function currentSessionId(): string | null {
    const match = window.location.pathname.match(/\/sessions\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeSessionId(): string | null {
    if (sessionState.status === 'ready') {
      return sessionState.session.id;
    }
    if (sessionState.status === 'loading' || sessionState.status === 'error') {
      return sessionState.sessionId;
    }
    return null;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/sessions"
        data-testid="session-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session details</h1>
    </div>
  </header>

  {#if sessionState.status === 'error'}
    <div data-testid="session-detail-error">
      <AdminMessage tone="error" title="Session detail unavailable" message={sessionState.message} />
    </div>
  {:else}
    <SessionInspector
      state={sessionState}
      {actionState}
      onRefresh={refreshSession}
      onCancelQueue={cancelQueuedSession}
      onDisconnectAll={disconnectAllConnections}
      onRelease={releaseSessionRuntime}
      onStop={stopSession}
      onKill={killSession}
    />
  {/if}
</div>
