<script lang="ts">
  import type { SessionResource } from '../api/control-types';
  import LogsPanel from '../presentation/LogsPanel.svelte';
  import { AdminLogsViewModelBuilder, type AdminLogEntry } from '../presentation/logs-view-model';

  type LogsSurfaceProps = {
    readonly selectedSession: SessionResource | null;
    readonly browserConnected: boolean;
    readonly sessionCount: number;
  };

  let { selectedSession, browserConnected, sessionCount }: LogsSurfaceProps = $props();
  let entries = $state<readonly AdminLogEntry[]>([]);
  let copied = $state(false);
  let lastSignature = $state('');
  const viewModel = $derived(AdminLogsViewModelBuilder.build(entries));

  $effect(() => {
    const signature = `${selectedSession?.id ?? 'none'}:${selectedSession?.state ?? 'none'}:${browserConnected}:${sessionCount}`;
    if (signature !== lastSignature) {
      lastSignature = signature;
      appendLog(describeState());
    }
  });

  async function copy(): Promise<void> {
    await navigator.clipboard?.writeText(entries.map((entry) => `${entry.timestamp} ${entry.message}`).join('\n'));
    copied = true;
  }

  function appendLog(message: string): void {
    entries = [{ id: crypto.randomUUID(), timestamp: new Date().toLocaleTimeString(), level: 'info', message }, ...entries].slice(0, 80);
    copied = false;
  }

  function describeState(): string {
    if (!selectedSession) {
      return `No session selected, ${sessionCount} visible sessions.`;
    }
    const connection = browserConnected ? 'browser connected' : 'browser disconnected';
    return `Selected ${selectedSession.id} is ${selectedSession.state}, ${connection}.`;
  }
</script>

<LogsPanel
  {viewModel}
  {copied}
  onClear={() => { entries = []; copied = false; }}
  onCopy={() => void copy()}
/>
