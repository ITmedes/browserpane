<script lang="ts">
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import CollapsibleWorkspacePanel from '../presentation/CollapsibleWorkspacePanel.svelte';
  import FeaturePlaceholderPanel from '../presentation/FeaturePlaceholderPanel.svelte';
  import SessionDetailPanel from '../presentation/SessionDetailPanel.svelte';
  import SessionListPanel from '../presentation/SessionListPanel.svelte';
  import {
    type AdminFeaturePanelId,
    type AdminWorkspaceViewModel,
  } from '../presentation/admin-workspace-view-model';
  import type {
    SessionDetailPanelViewModel,
    SessionListPanelViewModel,
  } from '../presentation/session-view-model';
  import type {
    BrowserSessionConnectPreferences,
    LiveBrowserSessionConnection,
  } from '../session/browser-session-types';
  import BrowserPolicySurface from './BrowserPolicySurface.svelte';
  import DisplayControlsSurface from './DisplayControlsSurface.svelte';
  import LogsSurface from './LogsSurface.svelte';
  import McpDelegationSurface from './McpDelegationSurface.svelte';
  import MetricsSurface from './MetricsSurface.svelte';
  import RecordingSurface from './RecordingSurface.svelte';
  import SessionFilesSurface from './SessionFilesSurface.svelte';
  import WorkflowOperationsSurface from './WorkflowOperationsSurface.svelte';

  type AdminWorkspaceSidebarProps = {
    readonly controlClient: ControlClient;
    readonly selectedSession: SessionResource | null;
    readonly sessions: readonly SessionResource[];
    readonly mcpBridge: McpBridgeConfig | null;
    readonly liveConnection: LiveBrowserSessionConnection | null;
    readonly browserConnected: boolean;
    readonly browserPreferences: BrowserSessionConnectPreferences;
    readonly workspaceViewModel: AdminWorkspaceViewModel;
    readonly sessionListViewModel: SessionListPanelViewModel;
    readonly sessionDetailViewModel: SessionDetailPanelViewModel;
    readonly onRefreshSessions: () => Promise<void>;
    readonly onCreateSession: () => void;
    readonly onSelectSessionId: (sessionId: string) => void;
    readonly onRefreshSelectedSession: () => Promise<void>;
    readonly onStopSession: () => void;
    readonly onKillSession: () => void;
    readonly onFileCountChange: (count: number) => void;
    readonly onBrowserPreferencesChange: (preferences: BrowserSessionConnectPreferences) => void;
  };

  let {
    controlClient,
    selectedSession,
    sessions,
    mcpBridge,
    liveConnection,
    browserConnected,
    browserPreferences,
    workspaceViewModel,
    sessionListViewModel,
    sessionDetailViewModel,
    onRefreshSessions,
    onCreateSession,
    onSelectSessionId,
    onRefreshSelectedSession,
    onStopSession,
    onKillSession,
    onFileCountChange,
    onBrowserPreferencesChange,
  }: AdminWorkspaceSidebarProps = $props();

  let openPanelIds = $state<readonly AdminFeaturePanelId[]>(['sessions', 'lifecycle', 'display', 'files', 'policy']);

  function togglePanel(panelId: AdminFeaturePanelId): void {
    openPanelIds = openPanelIds.includes(panelId)
      ? openPanelIds.filter((entry) => entry !== panelId)
      : [...openPanelIds, panelId];
  }

  function isPanelOpen(panelId: AdminFeaturePanelId): boolean {
    return openPanelIds.includes(panelId);
  }
</script>

<aside class="grid max-h-[calc(100vh-72px)] content-start gap-3 overflow-y-auto pr-1 max-xl:max-h-none max-xl:overflow-visible">
  <div class="rounded-[24px] border border-admin-ink/10 bg-admin-cream/70 p-3">
    <p class="admin-eyebrow mb-2">Admin surface map</p>
    <div class="flex flex-wrap gap-2">
      {#each workspaceViewModel.panels as panel (panel.id)}
        <button
          class={`rounded-full border px-3 py-1 text-xs font-extrabold ${
            isPanelOpen(panel.id)
              ? 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf'
              : 'border-admin-ink/10 bg-admin-panel/70 text-admin-ink/62'
          }`}
          type="button"
          onclick={() => togglePanel(panel.id)}
        >
          {panel.label}
        </button>
      {/each}
    </div>
  </div>

  {#each workspaceViewModel.panels as panel (panel.id)}
    <CollapsibleWorkspacePanel {panel} open={isPanelOpen(panel.id)} onToggle={togglePanel}>
      {#if panel.id === 'sessions'}
        <SessionListPanel
          viewModel={sessionListViewModel}
          onRefresh={() => void onRefreshSessions()}
          onCreateSession={onCreateSession}
          onSelectSessionId={onSelectSessionId}
        />
        <McpDelegationSurface
          {controlClient}
          {selectedSession}
          {sessions}
          {mcpBridge}
          {onRefreshSessions}
          {onRefreshSelectedSession}
        />
      {:else if panel.id === 'lifecycle'}
        <SessionDetailPanel
          viewModel={sessionDetailViewModel}
          onRefresh={() => void onRefreshSelectedSession()}
          onStop={onStopSession}
          onKill={onKillSession}
        />
      {:else if panel.id === 'files'}
        <SessionFilesSurface
          {controlClient}
          session={selectedSession}
          {onFileCountChange}
        />
      {:else if panel.id === 'display'}
        <DisplayControlsSurface
          {liveConnection}
          connected={browserConnected}
          preferences={browserPreferences}
          onPreferencesChange={onBrowserPreferencesChange}
        />
      {:else if panel.id === 'policy'}
        <BrowserPolicySurface
          {selectedSession}
          {onRefreshSelectedSession}
        />
      {:else if panel.id === 'recording'}
        <RecordingSurface {liveConnection} />
      {:else if panel.id === 'metrics'}
        <MetricsSurface {liveConnection} />
      {:else if panel.id === 'logs'}
        <LogsSurface {selectedSession} {browserConnected} sessionCount={sessions.length} />
      {:else if panel.id === 'workflows'}
        <WorkflowOperationsSurface {selectedSession} />
      {:else}
        <FeaturePlaceholderPanel {panel} />
      {/if}
    </CollapsibleWorkspacePanel>
  {/each}
</aside>
