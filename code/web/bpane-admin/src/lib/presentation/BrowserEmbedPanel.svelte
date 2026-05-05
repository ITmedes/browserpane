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

<section class="browser" aria-label="Live browser session">
  <div class="header">
    <div>
      <p class="eyebrow">Live browser</p>
      <h2>{status}</h2>
    </div>
    <div class="actions">
      <button type="button" disabled={!session || connecting || isConnected} onclick={connect}>
        Connect
      </button>
      <button type="button" disabled={!isConnected || connecting} onclick={onDisconnect}>
        Disconnect
      </button>
    </div>
  </div>

  <div class="viewport" bind:this={container}>
    {#if !isConnected}
      <div class="placeholder">
        <strong>{session ? 'Ready to connect' : 'No session selected'}</strong>
        <span>Live rendering uses the existing <code>bpane-client</code> bundle.</span>
      </div>
    {/if}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .browser {
    margin-top: 22px;
    padding: 24px;
    border: 1px solid rgba(24, 32, 24, 0.12);
    border-radius: 24px;
    background: rgba(24, 32, 24, 0.88);
    box-shadow: 0 18px 48px rgba(24, 32, 24, 0.16);
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
    color: #a7c7a1;
    font-size: 0.74rem;
    font-weight: 800;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    color: #fffdf3;
    font-size: 1.1rem;
  }

  button {
    min-height: 40px;
    padding: 0 14px;
    border: 1px solid rgba(255, 253, 243, 0.2);
    border-radius: 999px;
    background: #fffdf3;
    color: #162119;
    font: inherit;
    font-weight: 800;
    cursor: pointer;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .viewport {
    position: relative;
    overflow: hidden;
    min-height: min(62vh, 620px);
    margin-top: 22px;
    border: 1px solid rgba(255, 253, 243, 0.14);
    border-radius: 20px;
    background: #050806;
  }

  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    gap: 8px;
    color: rgba(255, 253, 243, 0.72);
    text-align: center;
  }

  .placeholder strong {
    color: #fffdf3;
    font-size: 1.3rem;
  }

  code {
    padding: 2px 6px;
    border-radius: 7px;
    background: rgba(255, 253, 243, 0.12);
  }

  .error {
    margin: 16px 0 0;
    color: #f49a7d;
    line-height: 1.5;
  }

  @media (max-width: 860px) {
    .header,
    .actions {
      align-items: stretch;
      flex-direction: column;
    }

    .viewport {
      min-height: 420px;
    }
  }
</style>
