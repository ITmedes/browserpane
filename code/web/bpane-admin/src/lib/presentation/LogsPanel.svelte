<script lang="ts">
  import AdminMessage from './AdminMessage.svelte';
  import type { AdminMessageFeedback } from './admin-message-types';
  import type { AdminLogEntry, AdminLogsViewModel } from './logs-view-model';

  type LogsPanelProps = {
    readonly viewModel: AdminLogsViewModel;
    readonly copied: boolean;
    readonly feedback?: AdminMessageFeedback | null;
    readonly onClear: () => void;
    readonly onCopy: () => void;
  };

  let { viewModel, copied, feedback = null, onClear, onCopy }: LogsPanelProps = $props();

  function levelClass(entry: AdminLogEntry): string {
    return entry.level === 'warn' ? 'text-admin-warm' : 'text-admin-leaf';
  }

  function sourceClass(entry: AdminLogEntry): string {
    return entry.source === 'gateway'
      ? 'bg-admin-leaf/10 text-admin-leaf'
      : 'bg-admin-cream/12 text-[#c1d0e8]';
  }
</script>

<section class="grid gap-4" aria-label="Admin event logs">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <p class="m-0 text-sm leading-normal text-admin-ink/68">Gateway event stream with local UI diagnostics.</p>
    <div class="flex flex-wrap gap-2">
      <span class="rounded-full bg-admin-leaf/10 px-3 py-1 text-xs font-extrabold text-admin-leaf" data-testid="admin-log-count">{viewModel.countLabel}</span>
      <span class="rounded-full bg-[#111e32]/80 px-3 py-1 text-xs font-extrabold text-[#c1d0e8]" data-testid="admin-log-source-count">{viewModel.sourceLabel}</span>
    </div>
  </div>

  <div class="flex flex-wrap gap-2">
    <button class="admin-button-primary" type="button" data-testid="admin-log-copy" disabled={!viewModel.canCopy} onclick={onCopy}>
      {copied ? 'Copied logs' : 'Copy logs'}
    </button>
    <button class="admin-button-primary" type="button" data-testid="admin-log-clear" disabled={!viewModel.canClear} onclick={onClear}>
      Clear logs
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

  {#if viewModel.entries.length === 0}
    <AdminMessage variant="empty" message={viewModel.emptyLabel} compact={true} />
  {:else}
    <ol class="grid max-h-64 gap-2 overflow-auto p-0">
      {#each viewModel.entries as entry}
        <li
          class="list-none rounded-[14px] bg-admin-cream/72 p-3 text-sm text-admin-ink/72"
          data-testid="admin-log-entry"
          data-log-source={entry.source}
        >
          <span class={`font-extrabold ${levelClass(entry)}`}>{entry.timestamp}</span>
          <span class={`ml-2 rounded-full px-2 py-0.5 text-[11px] font-extrabold uppercase tracking-[0.12em] ${sourceClass(entry)}`}>{entry.source}</span>
          <span class="ml-2">{entry.message}</span>
        </li>
      {/each}
    </ol>
  {/if}
</section>
