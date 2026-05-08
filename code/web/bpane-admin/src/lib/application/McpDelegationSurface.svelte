<script lang="ts">
  import { McpBridgeClient } from '../api/mcp-bridge-client';
  import type { ControlClient } from '../api/control-client';
  import type { McpBridgeHealth } from '../api/mcp-bridge-client';
  import type { SessionResource } from '../api/control-types';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import McpDelegationPanel from '../presentation/McpDelegationPanel.svelte';
  import { McpDelegationViewModelBuilder } from '../presentation/mcp-delegation-view-model';

  type McpDelegationSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly selectedSession: SessionResource | null;
    readonly sessions: readonly SessionResource[];
    readonly mcpBridge: McpBridgeConfig | null;
    readonly refreshVersion: number;
    readonly onRefreshSessions: () => Promise<void>;
    readonly onRefreshSelectedSession: () => Promise<void>;
  };

  let {
    controlClient,
    selectedSession,
    sessions,
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
    if (bridgeClient) {
      void refreshBridge();
    }
  });

  $effect(() => {
    if (refreshVersion === lastRefreshVersion) {
      return;
    }
    lastRefreshVersion = refreshVersion;
    if (bridgeClient) {
      void refreshBridge();
    }
  });

  async function refreshBridge(): Promise<void> {
    if (!bridgeClient) {
      return;
    }
    loading = true;
    error = null;
    try {
      health = await bridgeClient.getHealth();
    } catch (refreshError) {
      error = errorMessage(refreshError);
    } finally {
      loading = false;
    }
  }

  async function delegateSelectedSession(): Promise<void> {
    if (!selectedSession || !mcpBridge || !bridgeClient) {
      return;
    }
    loading = true;
    error = null;
    try {
      await controlClient.setAutomationDelegate(selectedSession.id, {
        client_id: mcpBridge.clientId,
        issuer: mcpBridge.issuer,
        display_name: mcpBridge.displayName,
      });
      await bridgeClient.setControlSession(selectedSession.id);
      await clearPreviousDelegates(selectedSession.id);
      await onRefreshSessions();
      await onRefreshSelectedSession();
      health = await bridgeClient.getHealth();
    } catch (delegateError) {
      error = errorMessage(delegateError);
    } finally {
      loading = false;
    }
  }

  async function clearBridge(): Promise<void> {
    if (!bridgeClient) {
      return;
    }
    loading = true;
    error = null;
    try {
      await bridgeClient.clearControlSession();
      await clearPreviousDelegates('');
      await onRefreshSessions();
      await onRefreshSelectedSession();
      health = await bridgeClient.getHealth();
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
    try {
      await navigator.clipboard.writeText(viewModel.endpointUrl);
    } catch (copyError) {
      error = errorMessage(copyError);
    }
  }

  async function clearPreviousDelegates(targetSessionId: string): Promise<void> {
    for (const session of sessions) {
      if (session.id !== targetSessionId && isDelegatedToBridge(session)) {
        await controlClient.clearAutomationDelegate(session.id);
      }
    }
  }

  function isDelegatedToBridge(session: SessionResource): boolean {
    const delegate = session.automation_delegate;
    return Boolean(delegate && mcpBridge && delegate.client_id === mcpBridge.clientId);
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected MCP delegation error';
  }
</script>

<McpDelegationPanel
  {viewModel}
  onRefresh={() => void refreshBridge()}
  onDelegate={() => void delegateSelectedSession()}
  onClear={() => void clearBridge()}
  onCopyEndpoint={() => void copyEndpoint()}
/>
