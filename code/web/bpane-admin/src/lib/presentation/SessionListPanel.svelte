<script lang="ts">
  import { base } from '$app/paths';
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

  function detailHref(sessionId: string): string {
    return `${base}/sessions/${encodeURIComponent(sessionId)}`;
  }
</script>

<div class="grid gap-3" aria-label="Owner-scoped sessions">
  <section class="grid gap-3 rounded-[16px] border border-admin-leaf/25 bg-admin-leaf/10 p-3" aria-label="Selected session">
    <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Selected session</p>
        <h3 class="m-0 truncate font-mono text-sm font-extrabold text-admin-ink" title={viewModel.selectedSession?.id ?? ''}>
          {viewModel.selectedSession?.id ?? 'No session selected'}
        </h3>
      </div>
      <span class="rounded-full border border-admin-leaf/30 bg-admin-leaf/12 px-3 py-1 text-xs font-extrabold text-admin-leaf">
        {viewModel.selectedSession?.presence ?? 'select'}
      </span>
    </div>

    {#if viewModel.selectedSession}
      <div class="grid min-w-0 grid-cols-2 gap-2 text-xs text-admin-ink/70 sm:grid-cols-4">
        {@render Fact('State', viewModel.selectedSession.lifecycle, 'session-selected-state')}
        {@render Fact('Runtime', viewModel.selectedSession.runtime, 'session-selected-runtime')}
        {@render Fact('Clients', String(viewModel.selectedSession.clients), 'session-selected-clients')}
        {@render Fact('MCP', viewModel.selectedSession.mcpDelegation, 'session-selected-mcp')}
      </div>
      <p class="m-0 truncate text-xs text-admin-ink/58">
        {viewModel.selectedSession.ownerMode} | {viewModel.selectedSession.runtimeBinding} | updated {viewModel.selectedSession.updatedAt}
      </p>
    {:else}
      <p class="m-0 text-sm leading-normal text-admin-ink/68">
        Select an existing session or create a new one before joining, delegating MCP, or inspecting runtime state.
      </p>
    {/if}

    <div class="flex flex-wrap gap-2">
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-join"
        disabled={!viewModel.authenticated || viewModel.loading || !viewModel.selectedSessionId}
        onclick={onJoinSession}
      >
        Join / reconnect
      </button>
      {#if viewModel.selectedSessionId}
        <a class="admin-button-ghost" data-testid="session-detail-link" href={detailHref(viewModel.selectedSessionId)}>
          Inspect details
        </a>
      {/if}
    </div>
  </section>

  <section class="grid gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Session switcher">
    <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
      <div>
        <p class="admin-eyebrow mb-1">Session switcher</p>
        <p class="m-0 text-sm font-bold text-admin-ink/72">{viewModel.sessions.length} visible sessions</p>
      </div>
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
          data-testid="session-refresh"
          disabled={!viewModel.authenticated || viewModel.loading}
          onclick={onRefresh}
        >
          Refresh
        </button>
      </div>
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
  </section>
</div>

{#snippet Fact(label: string, value: string, testId: string)}
  <span class="min-w-0 rounded-xl bg-admin-field/72 p-2 font-bold uppercase">
    {label}
    <strong class="mt-1 block truncate font-mono text-admin-ink normal-case" data-testid={testId}>{value}</strong>
  </span>
{/snippet}
