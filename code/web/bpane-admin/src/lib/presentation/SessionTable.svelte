<script lang="ts">
  import type { SessionListItemViewModel } from './session-view-model';

  type SessionTableProps = {
    readonly sessions: readonly SessionListItemViewModel[];
    readonly selectedSessionId: string | null;
    readonly onSelectSessionId: (sessionId: string) => void;
  };

  let { sessions, selectedSessionId, onSelectSessionId }: SessionTableProps = $props();

  function clientLabel(count: number): string {
    return count === 1 ? '1 client' : `${count} clients`;
  }
</script>

<div class="grid max-h-[min(360px,42vh)] min-w-0 gap-1 overflow-y-auto pr-1" aria-label="Session list">
  {#each sessions as session}
    <button
      class={`grid w-full min-w-0 cursor-pointer grid-cols-[4px_minmax(0,1fr)_auto] items-center gap-3 rounded-xl border p-2 text-left text-admin-ink/78 hover:border-admin-leaf/42 hover:bg-admin-field/84 ${
        session.id === selectedSessionId
          ? 'border-admin-leaf/42 bg-admin-field/84'
          : 'border-admin-ink/10 bg-admin-panel/68'
      }`}
      type="button"
      aria-pressed={session.id === selectedSessionId}
      data-testid="session-row"
      data-session-id={session.id}
      onclick={() => onSelectSessionId(session.id)}
    >
      <span class={`h-full min-h-12 rounded-full ${session.id === selectedSessionId ? 'bg-admin-leaf' : 'bg-admin-ink/12'}`}></span>
      <span class="grid min-w-0 gap-1">
        <span class="flex min-w-0 items-center gap-2">
          <strong class="min-w-0 truncate font-mono text-sm text-admin-ink" title={session.id}>{session.shortId}</strong>
          {#if session.id === selectedSessionId}
            <span class="rounded-full bg-admin-leaf/14 px-2 py-0.5 text-[0.68rem] font-extrabold text-admin-leaf">selected</span>
          {/if}
        </span>
        <span class="min-w-0 truncate text-xs text-admin-ink/52">
          {session.mcpDelegation} | {session.labels} | updated {session.updatedAt}
        </span>
      </span>
      <span class="grid justify-items-end gap-1 text-xs text-[#c1d0e8]">
        <span class="rounded-lg bg-admin-field/72 px-2 py-1">{session.lifecycle}</span>
        <span class="rounded-lg bg-admin-field/72 px-2 py-1">{clientLabel(session.clients)}</span>
      </span>
    </button>
  {/each}
</div>
