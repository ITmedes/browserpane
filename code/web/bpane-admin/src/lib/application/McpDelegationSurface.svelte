<script lang="ts">
  import { McpBridgeClient } from '../api/mcp-bridge-client';
  import type { ControlClient } from '../api/control-client';
  import type { McpBridgeHealth } from '../api/mcp-bridge-client';
  import type { SessionResource } from '../api/control-types';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import McpDelegationPanel from '../presentation/McpDelegationPanel.svelte';
  import { McpDelegationViewModelBuilder } from '../presentation/mcp-delegation-view-model';

  type McpDelegationSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly selectedSession: SessionResource | null;
    readonly mcpBridge: McpBridgeConfig | null;
    readonly refreshVersion: number;
    readonly onRefreshSessions: () => Promise<void>;
    readonly onRefreshSelectedSession: () => Promise<void>;
  };

  let {
    controlClient,
    selectedSession,
    mcpBridge,
    refreshVersion,
    onRefreshSessions,
    onRefreshSelectedSession,
  }: McpDelegationSurfaceProps = $props();
  let currentKey = $state('');
  let lastRefreshVersion = $state(0);
  let health = $state<McpBridgeHealth | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let feedback = $state<AdminMessageFeedback | null>(null);
  const bridgeClient = $derived(mcpBridge ? new McpBridgeClient({ controlUrl: mcpBridge.controlUrl }) : null);
  const viewModel = $derived(McpDelegationViewModelBuilder.build({
    bridge: mcpBridge,
    session: selectedSession,
    health,
    busy: loading,
    error,
  }));

  $effect(() => {
    const nextKey = `${mcpBridge?.controlUrl ?? 'none'}:${selectedSession?.id ?? 'none'}`;
    if (nextKey === currentKey) {
      return;
    }
    currentKey = nextKey;
    health = null;
    error = null;
    feedback = null;
    if (bridgeClient) {
      void refreshBridge(false);
    }
  });

  $effect(() => {
    if (refreshVersion === lastRefreshVersion) {
      return;
    }
    lastRefreshVersion = refreshVersion;
    if (bridgeClient) {
      void refreshBridge(false);
    }
  });

  async function refreshBridge(showFeedback = true): Promise<void> {
    if (!bridgeClient) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      health = await bridgeClient.getHealth();
      if (showFeedback) {
        feedback = successFeedback('MCP bridge status refreshed.');
      }
    } catch (refreshError) {
      error = errorMessage(refreshError);
      feedback = null;
    } finally {
      loading = false;
    }
  }

  async function authorizeSelectedSession(): Promise<void> {
    if (!selectedSession || !mcpBridge) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      await authorizeSession(selectedSession.id);
      await onRefreshSessions();
      await onRefreshSelectedSession();
      health = await bridgeClient?.getHealth() ?? health;
      feedback = successFeedback('MCP authorized for the selected session.');
    } catch (delegateError) {
      error = errorMessage(delegateError);
    } finally {
      loading = false;
    }
  }

  async function revokeSelectedSession(): Promise<void> {
    if (!selectedSession) {
      return;
    }
    if (health?.control_session_id === selectedSession.id) {
      error = 'Clear the default MCP session before revoking this authorization.';
      feedback = null;
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      await controlClient.clearAutomationDelegate(selectedSession.id);
      await onRefreshSessions();
      await onRefreshSelectedSession();
      health = await bridgeClient?.getHealth() ?? health;
      feedback = successFeedback('MCP authorization was revoked for the selected session.');
    } catch (clearError) {
      error = errorMessage(clearError);
    } finally {
      loading = false;
    }
  }

  async function setDefaultSession(): Promise<void> {
    if (!selectedSession || !mcpBridge || !bridgeClient) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      if (!isDelegatedToBridge(selectedSession)) {
        await authorizeSession(selectedSession.id);
      }
      await bridgeClient.setControlSession(selectedSession.id);
      await onRefreshSessions();
      await onRefreshSelectedSession();
      health = await bridgeClient.getHealth();
      feedback = successFeedback('Selected session is now the default MCP session.');
    } catch (clearError) {
      error = errorMessage(clearError);
    } finally {
      loading = false;
    }
  }

  async function clearDefaultSession(): Promise<void> {
    if (!bridgeClient) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      await bridgeClient.clearControlSession();
      await onRefreshSessions();
      await onRefreshSelectedSession();
      health = await bridgeClient.getHealth();
      feedback = successFeedback('Default MCP session was cleared.');
    } catch (clearError) {
      error = errorMessage(clearError);
    } finally {
      loading = false;
    }
  }

  async function copyEndpoint(): Promise<void> {
    if (!viewModel.endpointUrl) {
      return;
    }
    error = null;
    feedback = null;
    try {
      await navigator.clipboard.writeText(viewModel.endpointUrl);
      feedback = successFeedback('Session MCP endpoint copied.');
    } catch (copyError) {
      error = errorMessage(copyError);
    }
  }

  async function authorizeSession(sessionId: string): Promise<void> {
    if (!mcpBridge) return;
    await controlClient.setAutomationDelegate(sessionId, {
      client_id: mcpBridge.clientId,
      issuer: mcpBridge.issuer,
      display_name: mcpBridge.displayName,
    });
  }

  function isDelegatedToBridge(session: SessionResource): boolean {
    const delegate = session.automation_delegate;
    return Boolean(delegate && mcpBridge && delegate.client_id === mcpBridge.clientId && (!mcpBridge.issuer || delegate.issuer === mcpBridge.issuer));
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected MCP delegation error';
  }

  function successFeedback(message: string): AdminMessageFeedback {
    return { variant: 'success', title: 'MCP updated', message, testId: 'mcp-message' };
  }
</script>

<McpDelegationPanel
  {viewModel}
  {feedback}
  onRefresh={() => void refreshBridge()}
  onAuthorize={() => void authorizeSelectedSession()}
  onRevoke={() => void revokeSelectedSession()}
  onSetDefault={() => void setDefaultSession()}
  onClearDefault={() => void clearDefaultSession()}
  onCopyEndpoint={() => void copyEndpoint()}
/>
