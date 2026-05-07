<script lang="ts">
  import LogsPanel from '../presentation/LogsPanel.svelte';
  import { AdminLogsViewModelBuilder, type AdminLogEntry } from '../presentation/logs-view-model';
  import { AdminLogEntryFactory } from './admin-log-entries';

  type LogsSurfaceProps = {
    readonly entries: readonly AdminLogEntry[];
    readonly onClear: () => void;
  };

  let { entries, onClear }: LogsSurfaceProps = $props();
  let copied = $state(false);
  const viewModel = $derived(AdminLogsViewModelBuilder.build(entries));

  async function copy(): Promise<void> {
    await navigator.clipboard?.writeText(AdminLogEntryFactory.copyText(entries));
    copied = true;
  }

  function clear(): void {
    onClear();
    copied = false;
  }
</script>

<LogsPanel
  {viewModel}
  {copied}
  onClear={clear}
  onCopy={() => void copy()}
/>
