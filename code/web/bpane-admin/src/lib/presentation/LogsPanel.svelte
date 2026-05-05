<script lang="ts">
  import type { AdminLogEntry, AdminLogsViewModel } from './logs-view-model';

  type LogsPanelProps = {
    readonly viewModel: AdminLogsViewModel;
    readonly copied: boolean;
    readonly onClear: () => void;
    readonly onCopy: () => void;
  };

  let { viewModel, copied, onClear, onCopy }: LogsPanelProps = $props();

  function levelClass(entry: AdminLogEntry): string {
    return entry.level === 'warn' ? 'text-admin-warm' : 'text-admin-leaf';
  }
</script>

<section class="grid gap-4" aria-label="Admin event logs">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <p class="m-0 text-sm leading-normal text-admin-ink/68">Local admin timeline for session selection and connection changes.</p>
    <span class="rounded-full bg-admin-leaf/10 px-3 py-1 text-xs font-extrabold text-admin-leaf" data-testid="admin-log-count">
      {viewModel.countLabel}
    </span>
  </div>

  <div class="flex flex-wrap gap-2">
    <button class="admin-button-primary" type="button" data-testid="admin-log-copy" disabled={!viewModel.canCopy} onclick={onCopy}>
      {copied ? 'Copied logs' : 'Copy logs'}
    </button>
    <button class="admin-button-primary" type="button" data-testid="admin-log-clear" disabled={!viewModel.canClear} onclick={onClear}>
      Clear logs
    </button>
  </div>

  {#if viewModel.entries.length === 0}
    <p class="admin-empty mt-0">{viewModel.emptyLabel}</p>
  {:else}
    <ol class="grid max-h-64 gap-2 overflow-auto p-0">
      {#each viewModel.entries as entry}
        <li class="list-none rounded-[14px] bg-admin-cream/72 p-3 text-sm text-admin-ink/72">
          <span class={`font-extrabold ${levelClass(entry)}`}>{entry.timestamp}</span>
          <span class="ml-2">{entry.message}</span>
        </li>
      {/each}
    </ol>
  {/if}
</section>
