<script lang="ts">
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
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
  import SessionFilesSurface from './SessionFilesSurface.svelte';

  type AdminWorkspaceSidebarProps = {
    readonly controlClient: ControlClient;
    readonly selectedSession: SessionResource | null;
    readonly workspaceViewModel: AdminWorkspaceViewModel;
    readonly sessionListViewModel: SessionListPanelViewModel;
    readonly sessionDetailViewModel: SessionDetailPanelViewModel;
    readonly onRefreshSessions: () => void;
    readonly onCreateSession: () => void;
    readonly onSelectSessionId: (sessionId: string) => void;
    readonly onRefreshSelectedSession: () => void;
    readonly onStopSession: () => void;
    readonly onKillSession: () => void;
    readonly onFileCountChange: (count: number) => void;
  };

  let {
    controlClient,
    selectedSession,
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
  }: AdminWorkspaceSidebarProps = $props();

  let openPanelIds = $state<readonly AdminFeaturePanelId[]>(['sessions', 'lifecycle', 'files']);

  function togglePanel(panelId: AdminFeaturePanelId): void {
    openPanelIds = openPanelIds.includes(panelId)
      ? openPanelIds.filter((entry) => entry !== panelId)
      : [...openPanelIds, panelId];
  }

  function isPanelOpen(panelId: AdminFeaturePanelId): boolean {
    return openPanelIds.includes(panelId);
  }
</script>

<aside class="grid max-h-[calc(100vh-96px)] content-start gap-3 overflow-y-auto pr-1 max-xl:max-h-none max-xl:overflow-visible">
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
          onRefresh={onRefreshSessions}
          onCreateSession={onCreateSession}
          onSelectSessionId={onSelectSessionId}
        />
      {:else if panel.id === 'lifecycle'}
        <SessionDetailPanel
          viewModel={sessionDetailViewModel}
          onRefresh={onRefreshSelectedSession}
          onStop={onStopSession}
          onKill={onKillSession}
        />
      {:else if panel.id === 'files'}
        <SessionFilesSurface
          {controlClient}
          session={selectedSession}
          {onFileCountChange}
        />
      {:else}
        <FeaturePlaceholderPanel {panel} />
      {/if}
    </CollapsibleWorkspacePanel>
  {/each}
</aside>
