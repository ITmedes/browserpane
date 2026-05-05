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

<section class="admin-panel" aria-label="Selected session detail">
  <div class="admin-header">
    <div>
      <p class="admin-eyebrow">Selected session</p>
      <h2 class="m-0 max-w-[620px] overflow-hidden text-ellipsis whitespace-nowrap font-mono text-base text-admin-night">
        {session?.id ?? 'No session selected'}
      </h2>
    </div>
    <div class="admin-actions">
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-detail-refresh"
        disabled={!session || loading}
        onclick={onRefresh}
      >
        Refresh
      </button>
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-stop"
        disabled={!session || loading || connected || !session.status.stop_eligibility.allowed}
        onclick={onStop}
      >
        Stop
      </button>
      <button
        class="admin-button-primary"
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
    <p class="admin-empty">Select or create a session to inspect lifecycle and runtime state.</p>
  {:else}
    <div class="mt-[22px] grid grid-cols-3 gap-2.5 max-[860px]:grid-cols-1">
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
        state <strong class="mt-1 block text-admin-ink normal-case" data-testid="session-state">{session.state}</strong>
      </span>
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
        owner <strong class="mt-1 block text-admin-ink normal-case">{session.owner_mode}</strong>
      </span>
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
        runtime
        <strong class="mt-1 block text-admin-ink normal-case" data-testid="session-runtime-state">
          {session.status.runtime_state}
        </strong>
      </span>
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
        presence
        <strong class="mt-1 block text-admin-ink normal-case" data-testid="session-presence-state">
          {session.status.presence_state}
        </strong>
      </span>
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
        binding <strong class="mt-1 block text-admin-ink normal-case">{session.runtime.binding}</strong>
      </span>
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-[0.85rem] text-admin-ink/62 uppercase">
        transport <strong class="mt-1 block text-admin-ink normal-case">{session.connect.compatibility_mode}</strong>
      </span>
    </div>
    {#if connected}
      <p class="admin-empty">Disconnect the embedded browser before stopping this session.</p>
    {:else if !session.status.stop_eligibility.allowed}
      <p class="admin-empty">Stop is blocked by {formatBlockers(session)}.</p>
    {/if}
  {/if}

  {#if error}
    <p class="admin-error">{error}</p>
  {/if}
</section>
