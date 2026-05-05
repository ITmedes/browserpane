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

<section class="flex min-h-[calc(100vh-84px)] flex-col rounded-[30px] border border-admin-ink/12 bg-admin-night/92 p-4 shadow-[0_28px_80px_rgb(24_32_24_/_20%)]" aria-label="Live browser session">
  <div class="admin-header">
    <div>
      <p class="admin-eyebrow admin-eyebrow-light">Live browser</p>
      <h2 class="m-0 text-[1.1rem] font-bold text-admin-cream">{viewModel.status}</h2>
      <p class="mt-1 mb-0 text-xs font-bold text-admin-cream/58">
        Session {viewModel.sessionLabel} · {viewModel.connectionLabel}
      </p>
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
    class="relative mt-4 min-h-0 flex-1 overflow-hidden rounded-[24px] border border-admin-cream/14 bg-[#050806] max-[1100px]:min-h-[64vh] max-[760px]:min-h-[420px]"
    data-testid="browser-viewport"
    bind:this={container}
  >
    {#if !isConnected}
      <div class="absolute inset-0 grid place-content-center gap-2 text-center text-admin-cream/72">
        <strong class="text-[1.3rem] text-admin-cream">{session ? 'Ready to connect' : 'No session selected'}</strong>
        <span>
          Live rendering uses the existing <code class="rounded-[7px] bg-admin-cream/12 px-1.5 py-0.5">bpane-client</code>
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
