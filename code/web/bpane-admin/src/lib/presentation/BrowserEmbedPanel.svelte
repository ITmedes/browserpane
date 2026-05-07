<script lang="ts">
  import type { SessionResource } from '../api/control-types';
  import type { BrowserStageViewModel } from './admin-workspace-view-model';

  type BrowserEmbedPanelProps = {
    readonly viewModel: BrowserStageViewModel;
    readonly session: SessionResource | null;
    readonly connectedSessionId: string | null;
    readonly connecting: boolean;
    readonly error: string | null;
    readonly onConnect: (container: HTMLElement) => void;
    readonly onDisconnect: () => void;
  };

  let {
    viewModel,
    session,
    connectedSessionId,
    connecting,
    error,
    onConnect,
    onDisconnect,
  }: BrowserEmbedPanelProps = $props();
  let container: HTMLElement;
  const isConnected = $derived(Boolean(session && connectedSessionId === session.id));

  function connect(): void {
    if (container) {
      onConnect(container);
    }
  }
</script>

<section class="flex min-h-[calc(100vh-96px)] flex-col rounded-2xl border border-[#90a6cc]/18 bg-admin-night/92 p-3 shadow-[0_24px_64px_rgb(0_0_0_/_34%)] sm:p-4" aria-label="Live browser session">
  <div class="flex items-center justify-between gap-3">
    <div class="flex min-w-0 items-center gap-3">
      <span class="shrink-0 text-xs font-bold tracking-[0.16em] text-[#9fb1cf] uppercase">Live browser</span>
      <span class="shrink-0 text-sm font-bold text-admin-ink">{viewModel.status}</span>
      <span class="min-w-0 truncate text-xs font-bold text-[#9fb1cf]">
        Session {viewModel.sessionLabel} · {viewModel.connectionLabel}
      </span>
    </div>
    <div class="admin-actions">
      <button
        class="admin-button-light"
        type="button"
        data-testid="browser-connect"
        disabled={!session || connecting || isConnected}
        onclick={connect}
      >
        Connect
      </button>
      <button
        class="admin-button-light"
        type="button"
        data-testid="browser-disconnect"
        disabled={!isConnected || connecting}
        onclick={onDisconnect}
      >
        Disconnect
      </button>
    </div>
  </div>

  <div
    class="relative mt-3 min-h-0 flex-1 overflow-hidden rounded-xl border border-[#c4d5f4]/10 bg-[#050806] max-[1100px]:min-h-[64vh] max-[760px]:min-h-[420px]"
    data-testid="browser-viewport"
    bind:this={container}
  >
    {#if !isConnected}
      <div class="absolute inset-0 grid place-content-center gap-2 bg-[radial-gradient(circle_at_top,rgba(79,209,168,0.08),transparent_34%),linear-gradient(180deg,rgba(14,24,41,0.24),rgba(7,12,21,0.56))] p-6 text-center text-[#9fb1cf]">
        <strong class="text-[1.3rem] text-admin-ink">{session ? 'Ready to connect' : 'No session selected'}</strong>
        <span>
          Live rendering uses the existing <code class="rounded-md bg-admin-ink/10 px-1.5 py-0.5">bpane-client</code>
          bundle.
        </span>
      </div>
    {/if}
  </div>

  {#if error}
    <p class="mt-4 mb-0 text-[#f49a7d] leading-normal" data-testid="browser-error">{error}</p>
  {/if}
  <p class="sr-only" data-testid="browser-status">{viewModel.status}</p>
</section>
