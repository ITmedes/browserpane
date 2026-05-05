<script lang="ts">
  import type { SessionResource } from '../api/control-types';

  type SessionListPanelProps = {
    readonly sessions: readonly SessionResource[];
    readonly authenticated: boolean;
    readonly loading: boolean;
    readonly error: string | null;
    readonly onRefresh: () => void;
    readonly onCreateSession: () => void;
  };

  let {
    sessions,
    authenticated,
    loading,
    error,
    onRefresh,
    onCreateSession,
  }: SessionListPanelProps = $props();
</script>

<section class="sessions" aria-label="Owner-scoped sessions">
  <div class="header">
    <div>
      <p class="eyebrow">Control plane</p>
      <h2>Owner-scoped sessions</h2>
    </div>
    <div class="actions">
      <button type="button" disabled={!authenticated || loading} onclick={onCreateSession}>
        New session
      </button>
      <button type="button" disabled={!authenticated || loading} onclick={onRefresh}>
        Refresh
      </button>
    </div>
  </div>

  {#if !authenticated}
    <p class="empty">Sign in to inspect sessions from <code>/api/v1/sessions</code>.</p>
  {:else if loading}
    <p class="empty">Loading sessions...</p>
  {:else if error}
    <p class="error">{error}</p>
  {:else if sessions.length === 0}
    <p class="empty">No owner-scoped sessions are visible for this operator.</p>
  {:else}
    <div class="table" role="table" aria-label="Session list">
      <div class="row heading" role="row">
        <span role="columnheader">Session</span>
        <span role="columnheader">Lifecycle</span>
        <span role="columnheader">Runtime</span>
        <span role="columnheader">Presence</span>
        <span role="columnheader">Clients</span>
      </div>
      {#each sessions as session}
        <div class="row" role="row">
          <span role="cell" title={session.id}>{session.id}</span>
          <span role="cell">{session.state}</span>
          <span role="cell">{session.status.runtime_state}</span>
          <span role="cell">{session.status.presence_state}</span>
          <span role="cell">{session.status.connection_counts.total_clients}</span>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .sessions {
    margin-top: 22px;
    padding: 24px;
    border: 1px solid rgba(24, 32, 24, 0.12);
    border-radius: 24px;
    background: rgba(255, 255, 248, 0.62);
    box-shadow: 0 18px 48px rgba(24, 32, 24, 0.08);
  }

  .header,
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .header {
    justify-content: space-between;
  }

  .eyebrow {
    margin: 0 0 8px;
    color: #417463;
    font-size: 0.74rem;
    font-weight: 800;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    color: #243126;
    font-size: 1.25rem;
  }

  button {
    min-height: 40px;
    padding: 0 14px;
    border: 1px solid rgba(24, 32, 24, 0.18);
    border-radius: 999px;
    background: #243126;
    color: #fffdf3;
    font: inherit;
    font-weight: 800;
    cursor: pointer;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .empty,
  .error {
    margin: 22px 0 0;
    line-height: 1.5;
  }

  .empty {
    color: rgba(24, 32, 24, 0.68);
  }

  .error {
    color: #a33a21;
  }

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
    padding: 14px;
    border: 1px solid rgba(24, 32, 24, 0.1);
    border-radius: 16px;
    background: rgba(255, 255, 248, 0.68);
    color: rgba(24, 32, 24, 0.78);
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

  code {
    padding: 2px 6px;
    border-radius: 7px;
    background: rgba(65, 116, 99, 0.12);
  }

  @media (max-width: 860px) {
    .header,
    .actions {
      align-items: stretch;
      flex-direction: column;
    }

    .row {
      grid-template-columns: 1fr 1fr;
    }

    .heading {
      display: none;
    }
  }
</style>
