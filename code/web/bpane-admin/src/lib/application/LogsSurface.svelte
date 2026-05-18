<script lang="ts">
  import LogsPanel from '../presentation/LogsPanel.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import { AdminLogsViewModelBuilder, type AdminLogEntry } from '../presentation/logs-view-model';
  import { AdminLogEntryFactory } from './admin-log-entries';

  type LogsSurfaceProps = {
    readonly entries: readonly AdminLogEntry[];
    readonly onClear: () => void;
  };

  let { entries, onClear }: LogsSurfaceProps = $props();
  let copied = $state(false);
  let feedback = $state<AdminMessageFeedback | null>(null);
  const viewModel = $derived(AdminLogsViewModelBuilder.build(entries));

  async function copy(): Promise<void> {
    feedback = null;
    try {
      await navigator.clipboard?.writeText(AdminLogEntryFactory.copyText(entries));
      copied = true;
      feedback = { variant: 'success', title: 'Logs copied', message: 'Admin diagnostic log payload copied.', testId: 'admin-log-message' };
    } catch (error) {
      copied = false;
      feedback = {
        variant: 'error',
        title: 'Logs copy failed',
        message: error instanceof Error ? error.message : 'Could not copy admin diagnostic logs.',
        testId: 'admin-log-message',
      };
    }
  }

  function clear(): void {
    onClear();
    copied = false;
    feedback = { variant: 'info', title: 'Logs cleared', message: 'Admin diagnostic logs cleared.', testId: 'admin-log-message' };
  }
</script>

<LogsPanel
  {viewModel}
  {copied}
  {feedback}
  onClear={clear}
  onCopy={() => void copy()}
/>
