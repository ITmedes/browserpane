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
    loading = false;
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
    const client = bridgeClient;
    const requestKey = currentKey;
    if (!client) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      const nextHealth = await client.getHealth();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      health = nextHealth;
      if (showFeedback) {
        feedback = successFeedback('MCP bridge status refreshed.');
      }
    } catch (refreshError) {
      if (isCurrentRequest(requestKey)) {
        error = errorMessage(refreshError);
        feedback = null;
      }
    } finally {
      if (isCurrentRequest(requestKey)) {
        loading = false;
      }
    }
  }

  async function authorizeSelectedSession(): Promise<void> {
    const session = selectedSession;
    const bridge = mcpBridge;
    const client = bridgeClient;
    const requestKey = currentKey;
    if (!session || !bridge) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      await authorizeSession(session.id, bridge);
      await onRefreshSessions();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      await onRefreshSelectedSession();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      health = await client?.getHealth() ?? health;
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      feedback = successFeedback('MCP authorized for the selected session.');
    } catch (delegateError) {
      if (isCurrentRequest(requestKey)) {
        error = errorMessage(delegateError);
      }
    } finally {
      if (isCurrentRequest(requestKey)) {
        loading = false;
      }
    }
  }

  async function revokeSelectedSession(): Promise<void> {
    const session = selectedSession;
    const client = bridgeClient;
    const requestKey = currentKey;
    if (!session) {
      return;
    }
    if (health?.control_session_id === session.id) {
      error = 'Clear the default MCP session before revoking this authorization.';
      feedback = null;
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      await controlClient.clearAutomationDelegate(session.id);
      await onRefreshSessions();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      await onRefreshSelectedSession();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      health = await client?.getHealth() ?? health;
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      feedback = successFeedback('MCP authorization was revoked for the selected session.');
    } catch (clearError) {
      if (isCurrentRequest(requestKey)) {
        error = errorMessage(clearError);
      }
    } finally {
      if (isCurrentRequest(requestKey)) {
        loading = false;
      }
    }
  }

  async function setDefaultSession(): Promise<void> {
    const session = selectedSession;
    const bridge = mcpBridge;
    const client = bridgeClient;
    const requestKey = currentKey;
    if (!session || !bridge || !client) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      if (!isDelegatedToBridge(session, bridge)) {
        await authorizeSession(session.id, bridge);
      }
      await client.setControlSession(session.id);
      await onRefreshSessions();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      await onRefreshSelectedSession();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      health = await client.getHealth();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      feedback = successFeedback('Selected session is now the default MCP session.');
    } catch (clearError) {
      if (isCurrentRequest(requestKey)) {
        error = errorMessage(clearError);
      }
    } finally {
      if (isCurrentRequest(requestKey)) {
        loading = false;
      }
    }
  }

  async function clearDefaultSession(): Promise<void> {
    const client = bridgeClient;
    const requestKey = currentKey;
    if (!client) {
      return;
    }
    loading = true;
    error = null;
    feedback = null;
    try {
      await client.clearControlSession();
      await onRefreshSessions();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      await onRefreshSelectedSession();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      health = await client.getHealth();
      if (!isCurrentRequest(requestKey)) {
        return;
      }
      feedback = successFeedback('Default MCP session was cleared.');
    } catch (clearError) {
      if (isCurrentRequest(requestKey)) {
        error = errorMessage(clearError);
      }
    } finally {
      if (isCurrentRequest(requestKey)) {
        loading = false;
      }
    }
  }

  async function copyEndpoint(): Promise<void> {
    const endpointUrl = viewModel.endpointUrl;
    const requestKey = currentKey;
    if (!endpointUrl) {
      return;
    }
    error = null;
    feedback = null;
    try {
      await navigator.clipboard.writeText(endpointUrl);
      if (isCurrentRequest(requestKey)) {
        feedback = successFeedback('Session MCP endpoint copied.');
      }
    } catch (copyError) {
      if (isCurrentRequest(requestKey)) {
        error = errorMessage(copyError);
      }
    }
  }

  async function authorizeSession(sessionId: string, bridge: McpBridgeConfig): Promise<void> {
    await controlClient.setAutomationDelegate(sessionId, {
      client_id: bridge.clientId,
      issuer: bridge.issuer,
      display_name: bridge.displayName,
    });
  }

  function isDelegatedToBridge(session: SessionResource, bridge = mcpBridge): boolean {
    const delegate = session.automation_delegate;
    return Boolean(delegate && bridge && delegate.client_id === bridge.clientId && (!bridge.issuer || delegate.issuer === bridge.issuer));
  }

  function isCurrentRequest(requestKey: string): boolean {
    return requestKey === currentKey;
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
