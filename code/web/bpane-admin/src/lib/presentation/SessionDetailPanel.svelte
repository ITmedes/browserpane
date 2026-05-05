<script lang="ts">
  import type { SessionResource } from '../api/control-types';

  type SessionDetailPanelProps = {
    readonly session: SessionResource | null;
    readonly loading: boolean;
    readonly error: string | null;
    readonly connected: boolean;
    readonly onRefresh: () => void;
    readonly onStop: () => void;
    readonly onKill: () => void;
  };

  let {
    session,
    loading,
    error,
    connected,
    onRefresh,
    onStop,
    onKill,
  }: SessionDetailPanelProps = $props();

  function formatBlockers(value: SessionResource): string {
    const blockers = value.status.stop_eligibility.blockers;
    if (blockers.length === 0) {
      return 'the current runtime state';
    }
    return blockers.map((blocker) => `${blocker.count} ${blocker.kind}`).join(', ');
  }
</script>

<section class="detail" aria-label="Selected session detail">
  <div class="header">
    <div>
      <p class="eyebrow">Selected session</p>
      <h2>{session?.id ?? 'No session selected'}</h2>
    </div>
    <div class="actions">
      <button
        type="button"
        data-testid="session-detail-refresh"
        disabled={!session || loading}
        onclick={onRefresh}
      >
        Refresh
      </button>
      <button
        type="button"
        data-testid="session-stop"
        disabled={!session || loading || connected || !session.status.stop_eligibility.allowed}
        onclick={onStop}
      >
        Stop
      </button>
      <button
        type="button"
        data-testid="session-kill"
        disabled={!session || loading || connected}
        onclick={onKill}
      >
        Kill
      </button>
    </div>
  </div>

  {#if !session}
    <p class="empty">Select or create a session to inspect lifecycle and runtime state.</p>
  {:else}
    <div class="facts">
      <span>state <strong>{session.state}</strong></span>
      <span>owner <strong>{session.owner_mode}</strong></span>
      <span>runtime <strong>{session.status.runtime_state}</strong></span>
      <span>presence <strong>{session.status.presence_state}</strong></span>
      <span>binding <strong>{session.runtime.binding}</strong></span>
      <span>transport <strong>{session.connect.compatibility_mode}</strong></span>
    </div>
    {#if connected}
      <p class="hint">Disconnect the embedded browser before stopping this session.</p>
    {:else if !session.status.stop_eligibility.allowed}
      <p class="hint">Stop is blocked by {formatBlockers(session)}.</p>
    {/if}
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .detail {
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
    overflow: hidden;
    max-width: 620px;
    margin: 0;
    color: #243126;
    font-family: "SFMono-Regular", "Cascadia Code", monospace;
    font-size: 1rem;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin-top: 22px;
  }

  .facts span {
    padding: 12px;
    border-radius: 14px;
    background: rgba(65, 116, 99, 0.1);
    color: rgba(24, 32, 24, 0.62);
    font-size: 0.85rem;
    text-transform: uppercase;
  }

  .facts strong {
    display: block;
    margin-top: 4px;
    color: #162119;
    text-transform: none;
  }

  .empty,
  .hint,
  .error {
    margin: 18px 0 0;
    line-height: 1.5;
  }

  .empty,
  .hint {
    color: rgba(24, 32, 24, 0.68);
  }

  .error {
    color: #a33a21;
  }

  @media (max-width: 860px) {
    .header,
    .actions {
      align-items: stretch;
      flex-direction: column;
    }

    .facts {
      grid-template-columns: 1fr;
    }
  }
</style>
