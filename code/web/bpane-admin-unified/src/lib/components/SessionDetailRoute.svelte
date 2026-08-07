<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import AdminMessage from '$lib/components/AdminMessage.svelte';
  import { McpBridgeClient } from '$lib/mcp/mcp-bridge-client';
  import type { McpBridgeHealth } from '$lib/mcp/mcp-bridge-client';
  import {
    buildMcpDelegationViewModel,
    isDelegatedToBridge,
    sessionEndpointUrl,
  } from '$lib/mcp/mcp-delegation-view-model';
  import SessionInspector from '$lib/components/SessionInspector.svelte';
  import SessionSubareaNavigation from '$lib/components/SessionSubareaNavigation.svelte';
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
  let mcpHealth = $state<McpBridgeHealth | null>(null);
  let mcpActionState = $state<SessionActionState>({ status: 'idle' });
  const mcpBridge = $derived(authContext.authConfig?.mcpBridge ?? null);
  const selectedSession = $derived(sessionState.status === 'ready' ? sessionState.session : null);
  const routeSessionId = $derived(activeSessionId());
  const mcpViewModel = $derived(buildMcpDelegationViewModel({
    bridge: mcpBridge,
    session: selectedSession,
    health: mcpHealth,
    busy: mcpActionState.status === 'running',
  }));

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
      void refreshMcpBridge(false);
    } catch (error) {
      sessionState = {
        status: 'error',
        sessionId,
        message: adminErrorMessage(error, 'Unexpected session detail error.'),
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
      void refreshMcpBridge(false);
      actionState = { status: 'success', message: 'Session refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Session refresh failed.'),
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

  async function enableSessionRecording(): Promise<void> {
    const sessionId = activeSessionId();
    if (!sessionId) {
      return;
    }
    const catalog = client();
    actionState = { status: 'running', label: 'Enabling always-on recording...' };
    try {
      const session = await catalog.updateSessionRecordingPolicy(sessionId, {
        mode: 'always',
        format: 'webm',
      });
      const liveStatus = await loadSessionStatus(catalog, session.id);
      sessionState = {
        status: 'ready',
        session,
        liveStatus,
      };
      actionState = {
        status: 'success',
        message: recordingEnableMessage(session, liveStatus),
      };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Session recording enable failed.'),
      };
    }
  }

  async function disableSessionRecording(): Promise<void> {
    await mutateSession('Disabling always-on recording...', 'Always-on recording disabled.', (catalog, sessionId) =>
      catalog.updateSessionRecordingPolicy(sessionId, {
        mode: 'disabled',
        format: 'webm',
      }),
    );
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

  async function refreshMcpBridge(showFeedback = true): Promise<void> {
    const bridgeClient = mcpClient();
    if (!bridgeClient) {
      mcpHealth = null;
      return;
    }
    if (showFeedback) {
      mcpActionState = { status: 'running', label: 'Refreshing MCP bridge...' };
    }
    try {
      mcpHealth = await bridgeClient.getHealth();
      if (showFeedback) {
        mcpActionState = { status: 'success', message: 'MCP bridge status refreshed.' };
      }
    } catch (error) {
      mcpActionState = {
        status: 'error',
        message: adminErrorMessage(error, 'MCP bridge refresh failed.'),
      };
    }
  }

  async function authorizeMcp(): Promise<void> {
    await mutateMcp('Authorizing MCP bridge...', 'MCP bridge authorized for this session.', async (catalog, sessionId, bridge) => {
      await catalog.setAutomationDelegate(sessionId, {
        client_id: bridge.clientId,
        issuer: bridge.issuer,
        display_name: bridge.displayName,
      });
    });
  }

  async function revokeMcp(): Promise<void> {
    const sessionId = activeSessionId();
    if (sessionId && mcpHealth?.control_session_id === sessionId) {
      mcpActionState = {
        status: 'error',
        message: 'Clear the default MCP session before revoking this authorization.',
      };
      return;
    }
    await mutateMcp('Revoking MCP bridge...', 'MCP bridge authorization revoked for this session.', async (catalog, sessionId) => {
      await catalog.clearAutomationDelegate(sessionId);
    });
  }

  async function setDefaultMcpSession(): Promise<void> {
    await mutateMcp('Setting default MCP session...', 'This session is now the default MCP session.', async (
      catalog,
      sessionId,
      bridge,
      bridgeClient,
    ) => {
      if (!isDelegatedToBridge(selectedSession, bridge)) {
        await catalog.setAutomationDelegate(sessionId, {
          client_id: bridge.clientId,
          issuer: bridge.issuer,
          display_name: bridge.displayName,
        });
      }
      await bridgeClient.setControlSession(sessionId);
    });
  }

  async function clearDefaultMcpSession(): Promise<void> {
    await mutateMcp('Clearing default MCP session...', 'Default MCP session cleared.', async (
      _catalog,
      _sessionId,
      _bridge,
      bridgeClient,
    ) => {
      await bridgeClient.clearControlSession();
    });
  }

  async function copyMcpEndpoint(): Promise<void> {
    const endpoint = sessionEndpointUrl(mcpBridge, activeSessionId());
    if (!endpoint) {
      return;
    }
    try {
      await navigator.clipboard?.writeText(endpoint);
      mcpActionState = { status: 'success', message: 'Session MCP endpoint copied.' };
    } catch (error) {
      mcpActionState = {
        status: 'error',
        message: adminErrorMessage(error, 'MCP endpoint copy failed.'),
      };
    }
  }

  function openPreviewWindow(sessionId: string): void {
    const url = `/admin-new/sessions/${encodeURIComponent(sessionId)}/preview`;
    const popup = window.open(
      url,
      `bpane-session-preview-${sessionId}`,
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    if (popup) {
      popup.focus();
      return;
    }
    actionState = {
      status: 'error',
      message: 'The browser preview popup was blocked. Allow popups for this admin origin and try again.',
    };
  }

  function recordingEnableMessage(session: SessionResource, liveStatus: SessionStatus | null): string {
    const activeRecordingId = liveStatus?.recording?.active_recording_id;
    if (activeRecordingId) {
      return `Always-on recording enabled. Active segment ${activeRecordingId} is recording.`;
    }
    if (session.state === 'stopped' || session.state === 'released' || session.state === 'queued') {
      return 'Always-on recording enabled. The recorder will start when this session runtime starts.';
    }
    return 'Always-on recording enabled. Refresh the session status if the active segment is not visible yet.';
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
        message: adminErrorMessage(error, 'Session action failed.'),
      };
    }
  }

  async function mutateMcp(
    runningLabel: string,
    successMessage: string,
    mutation: (
      catalog: SessionCatalogClient,
      sessionId: string,
      bridge: NonNullable<typeof mcpBridge>,
      bridgeClient: McpBridgeClient,
    ) => Promise<void>,
  ): Promise<void> {
    const sessionId = activeSessionId();
    const bridge = mcpBridge;
    const bridgeClient = mcpClient();
    if (!sessionId || !bridge || !bridgeClient) {
      mcpActionState = {
        status: 'error',
        message: 'MCP bridge delegation is not configured for this admin deployment.',
      };
      return;
    }
    const catalog = client();
    mcpActionState = { status: 'running', label: runningLabel };
    try {
      await mutation(catalog, sessionId, bridge, bridgeClient);
      const loaded = await loadSessionPair(sessionId);
      sessionState = {
        status: 'ready',
        session: loaded.session,
        liveStatus: loaded.liveStatus,
      };
      mcpHealth = await bridgeClient.getHealth();
      mcpActionState = { status: 'success', message: successMessage };
    } catch (error) {
      mcpActionState = {
        status: 'error',
        message: adminErrorMessage(error, 'MCP action failed.'),
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

  function mcpClient(): McpBridgeClient | null {
    const bridge = mcpBridge;
    return bridge
      ? new McpBridgeClient({
          controlUrl: bridge.controlUrl,
          accessTokenProvider: authContext.accessTokenProvider,
          onAuthenticationFailure: authContext.onAuthenticationFailure,
        })
      : null;
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

  {#if routeSessionId}
    <SessionSubareaNavigation sessionId={routeSessionId} activeId="overview" availableIds={['overview', 'live', 'files', 'recordings']} />
  {/if}

  {#if sessionState.status === 'error'}
    <div data-testid="session-detail-error">
      <AdminMessage tone="error" title="Session detail unavailable" message={sessionState.message} />
    </div>
  {:else}
    <SessionInspector
      state={sessionState}
      {actionState}
      onRefresh={refreshSession}
      onConnectPreview={openPreviewWindow}
      onCancelQueue={cancelQueuedSession}
      onDisconnectAll={disconnectAllConnections}
      onEnableRecording={enableSessionRecording}
      onDisableRecording={disableSessionRecording}
      onRelease={releaseSessionRuntime}
      onStop={stopSession}
      onKill={killSession}
      {mcpViewModel}
      {mcpActionState}
      onMcpRefresh={refreshMcpBridge}
      onMcpAuthorize={authorizeMcp}
      onMcpRevoke={revokeMcp}
      onMcpSetDefault={setDefaultMcpSession}
      onMcpClearDefault={clearDefaultMcpSession}
      onMcpCopyEndpoint={copyMcpEndpoint}
    />
  {/if}
</div>
