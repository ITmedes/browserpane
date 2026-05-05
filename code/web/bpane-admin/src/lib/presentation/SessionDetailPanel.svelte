<script lang="ts">
  import type { SessionDetailPanelViewModel } from './session-view-model';

  type SessionDetailPanelProps = {
    readonly viewModel: SessionDetailPanelViewModel;
    readonly onRefresh: () => void;
    readonly onStop: () => void;
    readonly onKill: () => void;
  };

  let {
    viewModel,
    onRefresh,
    onStop,
    onKill,
  }: SessionDetailPanelProps = $props();
</script>

<div class="grid gap-4" aria-label="Selected session detail">
  <div>
    <p class="admin-eyebrow">Selected session</p>
    <h2 class="m-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-base text-admin-night">
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
  </div>

  {#if viewModel.facts.length === 0}
    <p class="admin-empty">{viewModel.hint}</p>
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
      <p class="admin-empty">{viewModel.hint}</p>
    {/if}
  {/if}

  {#if viewModel.error}
    <p class="admin-error">{viewModel.error}</p>
  {/if}
</div>
