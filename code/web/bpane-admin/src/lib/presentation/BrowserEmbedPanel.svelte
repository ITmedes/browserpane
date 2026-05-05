<script lang="ts">
  import type { SessionResource } from '../api/control-types';

  type BrowserEmbedPanelProps = {
    readonly session: SessionResource | null;
    readonly connectedSessionId: string | null;
    readonly connecting: boolean;
    readonly status: string;
    readonly error: string | null;
    readonly onConnect: (container: HTMLElement) => void;
    readonly onDisconnect: () => void;
  };

  let {
    session,
    connectedSessionId,
    connecting,
    status,
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

<section class="mt-[22px] rounded-[24px] border border-admin-ink/12 bg-admin-night/88 p-6 shadow-[0_18px_48px_rgb(24_32_24_/_16%)]" aria-label="Live browser session">
  <div class="admin-header">
    <div>
      <p class="admin-eyebrow admin-eyebrow-light">Live browser</p>
      <h2 class="m-0 text-[1.1rem] font-bold text-admin-cream">{status}</h2>
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
    class="relative mt-[22px] min-h-[min(62vh,620px)] overflow-hidden rounded-[20px] border border-admin-cream/14 bg-[#050806] max-[860px]:min-h-[420px]"
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
  <p class="sr-only" data-testid="browser-status">{status}</p>
</section>
