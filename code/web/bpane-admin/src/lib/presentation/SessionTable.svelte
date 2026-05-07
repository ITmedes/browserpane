<script lang="ts">
  import type { SessionListItemViewModel } from './session-view-model';

  type SessionTableProps = {
    readonly sessions: readonly SessionListItemViewModel[];
    readonly selectedSessionId: string | null;
    readonly onSelectSessionId: (sessionId: string) => void;
  };

  let { sessions, selectedSessionId, onSelectSessionId }: SessionTableProps = $props();
</script>

<div class="mt-4 grid min-w-0 gap-2" aria-label="Session list">
  {#each sessions as session}
    <button
      class={`grid w-full min-w-0 cursor-pointer gap-3 rounded-xl border p-3 text-left text-admin-ink/78 hover:border-admin-leaf/42 hover:bg-admin-field/84 ${
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
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-sm text-admin-ink" title={session.id}>
        {session.id}
      </span>
      <span class="grid min-w-0 grid-cols-2 gap-2 text-xs text-[#c1d0e8] sm:grid-cols-4">
        <span class="min-w-0 truncate rounded-lg bg-admin-field/72 px-2 py-1">State: {session.lifecycle}</span>
        <span class="min-w-0 truncate rounded-lg bg-admin-field/72 px-2 py-1">Runtime: {session.runtime}</span>
        <span class="min-w-0 truncate rounded-lg bg-admin-field/72 px-2 py-1">Presence: {session.presence}</span>
        <span class="min-w-0 truncate rounded-lg bg-admin-field/72 px-2 py-1">Clients: {session.clients}</span>
      </span>
    </button>
  {/each}
</div>
