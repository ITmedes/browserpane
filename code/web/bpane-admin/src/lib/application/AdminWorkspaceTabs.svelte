<script lang="ts">
  import {
    Activity,
    ClipboardList,
    FileArchive,
    FolderOpen,
    Gauge,
    MonitorCog,
    Radio,
    ScrollText,
    Video,
  } from 'lucide-svelte';
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import FeaturePlaceholderPanel from '../presentation/FeaturePlaceholderPanel.svelte';
  import SessionDetailPanel from '../presentation/SessionDetailPanel.svelte';
  import SessionListPanel from '../presentation/SessionListPanel.svelte';
  import {
    type AdminFeaturePanelId,
    type AdminFeaturePanelViewModel,
    type AdminWorkspaceViewModel,
  } from '../presentation/admin-workspace-view-model';
  import type { SessionDetailPanelViewModel, SessionListPanelViewModel } from '../presentation/session-view-model';
  import type { BrowserSessionConnectPreferences, LiveBrowserSessionConnection } from '../session/browser-session-types';
  import BrowserPolicySurface from './BrowserPolicySurface.svelte';
  import DisplayControlsSurface from './DisplayControlsSurface.svelte';
  import LogsSurface from './LogsSurface.svelte';
  import McpDelegationSurface from './McpDelegationSurface.svelte';
  import MetricsSurface from './MetricsSurface.svelte';
  import RecordingSurface from './RecordingSurface.svelte';
  import SessionFilesSurface from './SessionFilesSurface.svelte';
  import WorkflowOperationsSurface from './WorkflowOperationsSurface.svelte';

  type AdminWorkspaceTabsProps = {
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

  let props: AdminWorkspaceTabsProps = $props();
  let activePanelId = $state<AdminFeaturePanelId>('sessions');
  const activePanel = $derived(panelFor(activePanelId, props.workspaceViewModel.panels));

  function panelFor(id: AdminFeaturePanelId, panels: readonly AdminFeaturePanelViewModel[]): AdminFeaturePanelViewModel | null {
    return panels.find((panel) => panel.id === id) ?? panels[0] ?? null;
  }

  const PANEL_ICONS = {
    sessions: ClipboardList,
    lifecycle: Activity,
    display: MonitorCog,
    files: FolderOpen,
    policy: FileArchive,
    workflows: Radio,
    recording: Video,
    metrics: Gauge,
    logs: ScrollText,
  } satisfies Record<AdminFeaturePanelId, typeof Activity>;
</script>

<section class="grid h-full min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden" data-testid="admin-workspace-tabs">
  <div class="border-b border-[#90a6cc]/18 p-3">
    <div class="grid grid-cols-2 gap-2 sm:grid-cols-3" role="tablist" aria-label="Admin panels">
      {#each props.workspaceViewModel.panels as panel (panel.id)}
        {@const Icon = PANEL_ICONS[panel.id]}
        <button
          class={`inline-flex min-w-0 items-center justify-center gap-1.5 truncate rounded-xl border px-3 py-2 text-xs font-bold ${
            activePanelId === panel.id
              ? 'border-admin-leaf/40 bg-admin-leaf/14 text-admin-leaf'
              : 'border-[#90a6cc]/18 bg-[#111e32]/82 text-[#c1d0e8]'
          }`}
          type="button"
          role="tab"
          aria-selected={activePanelId === panel.id}
          data-testid={`workspace-panel-toggle-${panel.id}`}
          onclick={() => { activePanelId = panel.id; }}
        >
          <Icon size={14} aria-hidden="true" />
          <span class="truncate">{panel.label}</span>
        </button>
      {/each}
    </div>
  </div>

  {#if activePanel}
    <div
      class="min-h-0 min-w-0 overflow-y-auto p-3 sm:p-4"
      role="tabpanel"
      tabindex="0"
      aria-label={activePanel.label}
      data-testid={`workspace-panel-${activePanel.id}`}
    >
      <div class="mb-4 grid min-w-0 gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start">
        <div class="min-w-0">
          <p class="admin-eyebrow">{activePanel.label}</p>
          <h3 class="m-0 text-lg font-bold text-admin-ink">{activePanel.title}</h3>
          <p class="mt-1 mb-0 text-sm leading-normal text-[#c1d0e8]">{activePanel.description}</p>
        </div>
        <span class="w-fit rounded-xl border border-admin-leaf/25 bg-admin-leaf/10 px-3 py-1 text-xs font-bold text-admin-leaf">{activePanel.status}</span>
      </div>

      <div class="min-w-0 overflow-x-auto">
        {#if activePanel.id === 'sessions'}
        <SessionListPanel
          viewModel={props.sessionListViewModel}
          onRefresh={() => void props.onRefreshSessions()}
          onCreateSession={props.onCreateSession}
          onSelectSessionId={props.onSelectSessionId}
        />
        <McpDelegationSurface
          controlClient={props.controlClient}
          selectedSession={props.selectedSession}
          sessions={props.sessions}
          mcpBridge={props.mcpBridge}
          onRefreshSessions={props.onRefreshSessions}
          onRefreshSelectedSession={props.onRefreshSelectedSession}
        />
      {:else if activePanel.id === 'lifecycle'}
        <SessionDetailPanel
          viewModel={props.sessionDetailViewModel}
          onRefresh={() => void props.onRefreshSelectedSession()}
          onStop={props.onStopSession}
          onKill={props.onKillSession}
        />
      {:else if activePanel.id === 'files'}
        <SessionFilesSurface controlClient={props.controlClient} session={props.selectedSession} onFileCountChange={props.onFileCountChange} />
      {:else if activePanel.id === 'display'}
        <DisplayControlsSurface
          liveConnection={props.liveConnection}
          connected={props.browserConnected}
          preferences={props.browserPreferences}
          onPreferencesChange={props.onBrowserPreferencesChange}
        />
      {:else if activePanel.id === 'policy'}
        <BrowserPolicySurface selectedSession={props.selectedSession} onRefreshSelectedSession={props.onRefreshSelectedSession} />
      {:else if activePanel.id === 'recording'}
        <RecordingSurface liveConnection={props.liveConnection} />
      {:else if activePanel.id === 'metrics'}
        <MetricsSurface liveConnection={props.liveConnection} />
      {:else if activePanel.id === 'logs'}
        <LogsSurface selectedSession={props.selectedSession} browserConnected={props.browserConnected} sessionCount={props.sessions.length} />
      {:else if activePanel.id === 'workflows'}
        <WorkflowOperationsSurface selectedSession={props.selectedSession} />
      {:else}
        <FeaturePlaceholderPanel panel={activePanel} />
      {/if}
      </div>
    </div>
  {/if}
</section>
