<script lang="ts">
  import { ArrowLeft, ExternalLink, RefreshCw } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { McpBridgeClient, type McpBridgeHealth } from '$lib/mcp/mcp-bridge-client';
  import {
    buildMcpDelegationViewModel,
    isDelegatedToBridge,
    sessionEndpointUrl,
  } from '$lib/mcp/mcp-delegation-view-model';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { buildSessionAutomationModel } from '$lib/sessions/session-automation-view-model';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionResource } from '$lib/sessions/session-types';
  import { WorkflowRunCatalogClient } from '$lib/workflow-runs/workflow-run-client';
  import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionMcpDelegationCard from './SessionMcpDelegationCard.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

  type SessionAutomationRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly sessionId: string;
  };

  type SessionAutomationRouteState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | {
        readonly status: 'ready';
        readonly session: SessionResource;
        readonly runs: readonly WorkflowRunResource[];
      };

  let { authContext, sessionId }: SessionAutomationRouteProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let routeState = $state<SessionAutomationRouteState>({ status: 'loading' });
  let routeActionState = $state<AdminActionState>({ status: 'idle' });
  let mcpHealth = $state<McpBridgeHealth | null>(null);
  let mcpActionState = $state<AdminActionState>({ status: 'idle' });

  const mcpBridge = $derived(authContext.authConfig?.mcpBridge ?? null);
  const selectedSession = $derived(routeState.status === 'ready' ? routeState.session : null);
  const model = $derived(routeState.status === 'ready'
    ? buildSessionAutomationModel(routeState.session.id, routeState.runs)
    : null);
  const mcpViewModel = $derived(buildMcpDelegationViewModel({
    bridge: mcpBridge,
    session: selectedSession,
    health: mcpHealth,
    busy: mcpActionState.status === 'running',
  }));
  const busy = $derived(routeState.status === 'loading' || routeActionState.status === 'running');

  $effect(() => {
    if (sessionId === loadedSessionId) {
      return;
    }
    loadedSessionId = sessionId;
    routeState = { status: 'loading' };
    routeActionState = { status: 'idle' };
    mcpActionState = { status: 'idle' };
    mcpHealth = null;
    void loadRoute(sessionId, false);
  });

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  function sessionClient(): SessionCatalogClient {
    return new SessionCatalogClient(clientOptions());
  }

  function workflowRunClient(): WorkflowRunCatalogClient {
    return new WorkflowRunCatalogClient(clientOptions());
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

  async function loadRoute(requestSessionId = sessionId, showFeedback = true): Promise<void> {
    if (showFeedback) {
      routeActionState = { status: 'running', label: 'Refreshing session automation...' };
    }
    try {
      const [session, response] = await Promise.all([
        sessionClient().getSession(requestSessionId),
        workflowRunClient().listRuns(),
      ]);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      routeState = { status: 'ready', session, runs: response.runs };
      if (showFeedback) {
        routeActionState = { status: 'success', message: 'Session automation refreshed.' };
      }
      void refreshMcpBridge(false);
    } catch (error) {
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      const message = adminErrorMessage(error, 'Unexpected session automation route error.');
      routeState = { status: 'error', message };
      if (showFeedback) {
        routeActionState = { status: 'error', message };
      }
    }
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
    await mutateMcp('Authorizing MCP bridge...', 'MCP bridge authorized for this session.', async (
      catalog,
      requestSessionId,
      bridge,
    ) => {
      await catalog.setAutomationDelegate(requestSessionId, {
        client_id: bridge.clientId,
        issuer: bridge.issuer,
        display_name: bridge.displayName,
      });
    });
  }

  async function revokeMcp(): Promise<void> {
    if (mcpHealth?.control_session_id === sessionId) {
      mcpActionState = {
        status: 'error',
        message: 'Clear the default MCP session before revoking this authorization.',
      };
      return;
    }
    await mutateMcp('Revoking MCP bridge...', 'MCP bridge authorization revoked for this session.', async (
      catalog,
      requestSessionId,
    ) => {
      await catalog.clearAutomationDelegate(requestSessionId);
    });
  }

  async function setDefaultMcpSession(): Promise<void> {
    await mutateMcp('Setting default MCP session...', 'This session is now the default MCP session.', async (
      catalog,
      requestSessionId,
      bridge,
      bridgeClient,
    ) => {
      if (!isDelegatedToBridge(selectedSession, bridge)) {
        await catalog.setAutomationDelegate(requestSessionId, {
          client_id: bridge.clientId,
          issuer: bridge.issuer,
          display_name: bridge.displayName,
        });
      }
      await bridgeClient.setControlSession(requestSessionId);
    });
  }

  async function clearDefaultMcpSession(): Promise<void> {
    await mutateMcp('Clearing default MCP session...', 'Default MCP session cleared.', async (
      _catalog,
      _requestSessionId,
      _bridge,
      bridgeClient,
    ) => {
      await bridgeClient.clearControlSession();
    });
  }

  async function copyMcpEndpoint(): Promise<void> {
    const endpoint = sessionEndpointUrl(mcpBridge, sessionId);
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

  async function mutateMcp(
    runningLabel: string,
    successMessage: string,
    mutation: (
      catalog: SessionCatalogClient,
      requestSessionId: string,
      bridge: NonNullable<typeof mcpBridge>,
      bridgeClient: McpBridgeClient,
    ) => Promise<void>,
  ): Promise<void> {
    const requestSessionId = sessionId;
    const bridge = mcpBridge;
    const bridgeClient = mcpClient();
    if (!bridge || !bridgeClient) {
      mcpActionState = {
        status: 'error',
        message: 'MCP bridge delegation is not configured for this admin deployment.',
      };
      return;
    }
    const catalog = sessionClient();
    mcpActionState = { status: 'running', label: runningLabel };
    try {
      await mutation(catalog, requestSessionId, bridge, bridgeClient);
      const session = await catalog.getSession(requestSessionId);
      if (loadedSessionId !== requestSessionId || routeState.status !== 'ready') {
        return;
      }
      routeState = { ...routeState, session };
      mcpHealth = await bridgeClient.getHealth();
      mcpActionState = { status: 'success', message: successMessage };
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        mcpActionState = {
          status: 'error',
          message: adminErrorMessage(error, 'MCP action failed.'),
        };
      }
    }
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-automation-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/sessions" data-testid="session-automation-back">
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session automation</h1>
      <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{sessionId}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void loadRoute()}
      disabled={busy}
      data-testid="session-automation-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh</span>
    </button>
  </header>

  <SessionSubareaNavigation
    {sessionId}
    activeId="automation"
    availableIds={['overview', 'live', 'automation', 'policy', 'files', 'recordings', 'network', 'observability']}
  />

  <ActionFeedback
    state={routeActionState}
    successTitle="Session automation refreshed"
    errorTitle="Session automation refresh failed"
    successTestId="session-automation-action-success"
    errorTestId="session-automation-action-error"
    runningTestId="session-automation-action-running"
  />

  {#if routeState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-automation-loading">
      Loading session automation...
    </section>
  {:else if routeState.status === 'error'}
    <AdminMessage tone="error" title="Session automation unavailable" message={routeState.message} testId="session-automation-error" />
  {:else if model}
    <SessionMcpDelegationCard
      viewModel={mcpViewModel}
      actionState={mcpActionState}
      onRefresh={refreshMcpBridge}
      onAuthorize={authorizeMcp}
      onRevoke={revokeMcp}
      onSetDefault={setDefaultMcpSession}
      onClearDefault={clearDefaultMcpSession}
      onCopyEndpoint={copyMcpEndpoint}
    />

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-workflow-associations">
      <div class="border-b border-admin-border p-4">
        <h2 class="m-0 text-base font-semibold text-admin-ink">Workflow associations</h2>
        <p class="m-0 mt-1 text-sm text-admin-muted">Owner-visible workflow runs bound to this session.</p>
      </div>

      <dl class="grid gap-px border-b border-admin-border bg-admin-border sm:grid-cols-3">
        {#each model.metrics as metric}
          <div class="bg-admin-panel p-4">
            <dt class="text-xs font-semibold uppercase text-admin-muted">{metric.label}</dt>
            <dd class="m-0 mt-1 text-xl font-semibold text-admin-ink" data-testid={metric.testId}>{metric.value}</dd>
          </div>
        {/each}
      </dl>

      {#if model.workflowRuns.length === 0}
        <p class="m-4 rounded-md border border-dashed border-admin-border bg-admin-soft/50 p-4 text-sm text-admin-muted" data-testid="session-workflow-associations-empty">
          No workflow runs are associated with this session.
        </p>
      {:else}
        <div class="divide-y divide-admin-border" data-testid="session-workflow-association-list">
          {#each model.workflowRuns as run}
            <article class="flex min-w-0 flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between" data-testid={`session-workflow-run-${run.id}`}>
              <div class="min-w-0">
                <div class="flex min-w-0 flex-wrap items-center gap-2">
                  <span class="font-mono text-sm font-semibold text-admin-ink">{run.shortId}</span>
                  <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(run.stateTone)}`}>{run.state}</span>
                  <span class="text-xs text-admin-muted">{run.workflowId} · {run.workflowVersion}</span>
                </div>
                <p class="m-0 mt-1 text-sm text-admin-muted">{run.terminalAt}</p>
                {#if run.error !== 'No error'}
                  <p class="m-0 mt-1 break-words text-sm text-red-700">{run.error}</p>
                {/if}
              </div>
              <a
                class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
                href={`/admin-new/workflow-runs/${encodeURIComponent(run.id)}`}
                data-testid={`session-workflow-run-open-${run.id}`}
              >
                <span>Open run</span>
                <ExternalLink size={14} strokeWidth={1.8} />
              </a>
            </article>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>
