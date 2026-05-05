<script lang="ts">
  import type { SessionResource } from '../api/control-types';

  type SessionTableProps = {
    readonly sessions: readonly SessionResource[];
    readonly selectedSessionId: string | null;
    readonly onSelectSession: (session: SessionResource) => void;
  };

  let { sessions, selectedSessionId, onSelectSession }: SessionTableProps = $props();
</script>

<div class="table" aria-label="Session list">
  <div class="row heading" aria-hidden="true">
    <span>Session</span>
    <span>Lifecycle</span>
    <span>Runtime</span>
    <span>Presence</span>
    <span>Clients</span>
  </div>
  {#each sessions as session}
    <button
      class="row"
      class:active={session.id === selectedSessionId}
      type="button"
      aria-pressed={session.id === selectedSessionId}
      onclick={() => onSelectSession(session)}
    >
      <span title={session.id}>{session.id}</span>
      <span>{session.state}</span>
      <span>{session.status.runtime_state}</span>
      <span>{session.status.presence_state}</span>
      <span>{session.status.connection_counts.total_clients}</span>
    </button>
  {/each}
</div>

<style>
  .table {
    display: grid;
    gap: 8px;
    margin-top: 22px;
  }

  .row {
    display: grid;
    grid-template-columns: minmax(140px, 1.5fr) repeat(4, minmax(96px, 1fr));
    gap: 12px;
    align-items: center;
    width: 100%;
    padding: 14px;
    border: 1px solid rgba(24, 32, 24, 0.1);
    border-radius: 16px;
    background: rgba(255, 255, 248, 0.68);
    color: rgba(24, 32, 24, 0.78);
    font: inherit;
    text-align: left;
  }

  button.row {
    cursor: pointer;
  }

  button.row:hover,
  button.row.active {
    border-color: rgba(65, 116, 99, 0.42);
    background: rgba(232, 245, 223, 0.84);
  }

  .heading {
    background: transparent;
    color: rgba(24, 32, 24, 0.52);
    font-size: 0.78rem;
    font-weight: 900;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .row span:first-child {
    overflow: hidden;
    color: #162119;
    font-family: "SFMono-Regular", "Cascadia Code", monospace;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 860px) {
    .row {
      grid-template-columns: 1fr 1fr;
    }

    .heading {
      display: none;
    }
  }
</style>
