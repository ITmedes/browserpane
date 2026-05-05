<script lang="ts">
  import type { SessionListItemViewModel } from './session-view-model';

  type SessionTableProps = {
    readonly sessions: readonly SessionListItemViewModel[];
    readonly selectedSessionId: string | null;
    readonly onSelectSessionId: (sessionId: string) => void;
  };

  let { sessions, selectedSessionId, onSelectSessionId }: SessionTableProps = $props();
</script>

<div class="mt-[22px] grid gap-2" aria-label="Session list">
  <div
    class="grid grid-cols-[minmax(140px,1.5fr)_repeat(4,minmax(96px,1fr))] items-center gap-3 rounded-2xl border border-admin-ink/10 bg-transparent p-3.5 text-left text-xs font-black tracking-[0.08em] text-admin-ink/52 uppercase max-[860px]:hidden"
    aria-hidden="true"
  >
    <span>Session</span>
    <span>Lifecycle</span>
    <span>Runtime</span>
    <span>Presence</span>
    <span>Clients</span>
  </div>
  {#each sessions as session}
    <button
      class={`grid w-full cursor-pointer grid-cols-[minmax(140px,1.5fr)_repeat(4,minmax(96px,1fr))] items-center gap-3 rounded-2xl border p-3.5 text-left text-admin-ink/78 hover:border-admin-leaf/42 hover:bg-admin-field/84 max-[860px]:grid-cols-2 ${
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
      <span class="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-admin-ink" title={session.id}>{session.id}</span>
      <span>{session.lifecycle}</span>
      <span>{session.runtime}</span>
      <span>{session.presence}</span>
      <span>{session.clients}</span>
    </button>
  {/each}
</div>
