<script lang="ts">
  import SessionTable from './SessionTable.svelte';
  import type { SessionListPanelViewModel } from './session-view-model';

  type SessionListPanelProps = {
    readonly viewModel: SessionListPanelViewModel;
    readonly onRefresh: () => void;
    readonly onCreateSession: () => void;
    readonly onJoinSession: () => void;
    readonly onSelectSessionId: (sessionId: string) => void;
  };

  let {
    viewModel,
    onRefresh,
    onCreateSession,
    onJoinSession,
    onSelectSessionId,
  }: SessionListPanelProps = $props();
</script>

<div class="grid gap-4" aria-label="Owner-scoped sessions">
  <div class="flex flex-wrap gap-2">
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-new"
      disabled={!viewModel.authenticated || viewModel.loading}
      onclick={onCreateSession}
    >
      New session
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-join"
      disabled={!viewModel.authenticated || viewModel.loading || !viewModel.selectedSessionId}
      onclick={onJoinSession}
    >
      Join / reconnect
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-refresh"
      disabled={!viewModel.authenticated || viewModel.loading}
      onclick={onRefresh}
    >
      Refresh
    </button>
  </div>

  {#if !viewModel.authenticated}
    <p class="admin-empty mt-0">
      Sign in to inspect sessions from <code class="admin-code-pill">/api/v1/sessions</code>.
    </p>
  {:else if viewModel.loading}
    <p class="admin-empty mt-0">Loading sessions...</p>
  {:else if viewModel.error}
    <p class="admin-error mt-0">{viewModel.error}</p>
  {:else if viewModel.sessions.length === 0}
    <p class="admin-empty mt-0">No owner-scoped sessions are visible for this operator.</p>
  {:else}
    <SessionTable
      sessions={viewModel.sessions}
      selectedSessionId={viewModel.selectedSessionId}
      {onSelectSessionId}
    />
  {/if}
</div>
