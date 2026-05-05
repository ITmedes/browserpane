<script lang="ts">
  import type { SessionResource } from '../api/control-types';
  import SessionTable from './SessionTable.svelte';

  type SessionListPanelProps = {
    readonly sessions: readonly SessionResource[];
    readonly selectedSessionId: string | null;
    readonly authenticated: boolean;
    readonly loading: boolean;
    readonly error: string | null;
    readonly onRefresh: () => void;
    readonly onCreateSession: () => void;
    readonly onSelectSession: (session: SessionResource) => void;
  };

  let {
    sessions,
    selectedSessionId,
    authenticated,
    loading,
    error,
    onRefresh,
    onCreateSession,
    onSelectSession,
  }: SessionListPanelProps = $props();
</script>

<section class="admin-panel" aria-label="Owner-scoped sessions">
  <div class="admin-header">
    <div>
      <p class="admin-eyebrow">Control plane</p>
      <h2 class="admin-section-title">Owner-scoped sessions</h2>
    </div>
    <div class="admin-actions">
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-new"
        disabled={!authenticated || loading}
        onclick={onCreateSession}
      >
        New session
      </button>
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-refresh"
        disabled={!authenticated || loading}
        onclick={onRefresh}
      >
        Refresh
      </button>
    </div>
  </div>

  {#if !authenticated}
    <p class="admin-empty admin-empty-spacious">
      Sign in to inspect sessions from <code class="admin-code-pill">/api/v1/sessions</code>.
    </p>
  {:else if loading}
    <p class="admin-empty admin-empty-spacious">Loading sessions...</p>
  {:else if error}
    <p class="admin-error admin-empty-spacious">{error}</p>
  {:else if sessions.length === 0}
    <p class="admin-empty admin-empty-spacious">No owner-scoped sessions are visible for this operator.</p>
  {:else}
    <SessionTable {sessions} {selectedSessionId} {onSelectSession} />
  {/if}
</section>
