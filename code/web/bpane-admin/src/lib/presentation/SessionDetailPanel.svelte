<script lang="ts">
  import AdminMessage from './AdminMessage.svelte';
  import type { AdminMessageFeedback } from './admin-message-types';
  import type { SessionDetailPanelViewModel } from './session-view-model';

  type SessionDetailPanelProps = {
    readonly viewModel: SessionDetailPanelViewModel;
    readonly onRefresh: () => void;
    readonly onStop: () => void;
    readonly onKill: () => void;
    readonly onDisconnectConnection: (connectionId: number) => void;
    readonly onDisconnectAll: () => void;
    readonly feedback?: AdminMessageFeedback | null;
  };

  let {
    viewModel,
    onRefresh,
    onStop,
    onKill,
    onDisconnectConnection,
    onDisconnectAll,
    feedback = null,
  }: SessionDetailPanelProps = $props();
</script>

<div class="grid gap-4" aria-label="Selected session detail">
  <div>
    <p class="admin-eyebrow">Selected session</p>
    <h2 class="m-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-base text-admin-ink">
      {viewModel.title}
    </h2>
  </div>
  <div class="flex flex-wrap gap-2">
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-detail-refresh"
      disabled={!viewModel.canRefresh}
      onclick={onRefresh}
    >
      Refresh
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-stop"
      disabled={!viewModel.canStop}
      onclick={onStop}
    >
      Stop
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-kill"
      disabled={!viewModel.canKill}
      onclick={onKill}
    >
      Kill
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-disconnect-all"
      disabled={!viewModel.canDisconnectAll}
      onclick={onDisconnectAll}
    >
      Disconnect all
    </button>
  </div>

  {#if feedback}
    <AdminMessage
      variant={feedback.variant}
      title={feedback.title}
      message={feedback.message}
      testId={feedback.testId}
      compact={true}
    />
  {/if}

  {#if viewModel.facts.length === 0}
    <AdminMessage variant="empty" message={viewModel.hint} compact={true} />
  {:else}
    <div class="grid grid-cols-2 gap-2.5 max-[860px]:grid-cols-1">
      {#each viewModel.facts as fact}
        <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
          {fact.label}
          <strong class="mt-1 block text-admin-ink normal-case" data-testid={fact.testId}>
            {fact.value}
          </strong>
        </span>
      {/each}
    </div>
    {#if viewModel.hint}
      <AdminMessage variant="info" role="note" message={viewModel.hint} compact={true} />
    {/if}
  {/if}

  <div class="grid gap-2" aria-label="Live session connections">
    <div class="flex items-center justify-between gap-3">
      <p class="admin-eyebrow m-0">Connections</p>
      {#if viewModel.loading}
        <span class="text-xs font-bold text-admin-leaf">Loading</span>
      {/if}
    </div>
    {#if viewModel.connections.length === 0}
      <AdminMessage
        variant="empty"
        message={viewModel.statusHint ?? 'No live connections reported.'}
        testId="session-connection-empty"
        compact={true}
      />
    {:else}
      <div class="grid gap-2">
        {#each viewModel.connections as connection}
          <div
            class="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-xl border border-admin-ink/10 bg-admin-panel/68 p-3"
            data-testid="session-connection-row"
            data-connection-id={connection.id}
          >
            <span class="min-w-0">
              <strong class="block font-mono text-sm text-admin-ink">{connection.label}</strong>
              <span class="text-xs uppercase text-admin-ink/62" data-testid="session-connection-role">
                {connection.role}
              </span>
            </span>
            <button
              class="admin-button-primary"
              type="button"
              data-testid="session-connection-disconnect"
              disabled={!connection.canDisconnect}
              onclick={() => onDisconnectConnection(connection.id)}
            >
              Disconnect
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if viewModel.error}
    <AdminMessage variant="error" message={viewModel.error} compact={true} />
  {/if}
</div>
