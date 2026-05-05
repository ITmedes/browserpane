<script lang="ts">
  import type { ControlClient } from '../api/control-client';
  import type { SessionFileResource, SessionResource } from '../api/control-types';
  import SessionFileCard from '../presentation/SessionFileCard.svelte';

  type SessionFilesSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly session: SessionResource | null;
  };

  let { controlClient, session }: SessionFilesSurfaceProps = $props();
  let currentSessionId = $state<string | null>(null);
  let files = $state<readonly SessionFileResource[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let downloadingFileId = $state<string | null>(null);

  $effect(() => {
    const nextSessionId = session?.id ?? null;
    if (nextSessionId === currentSessionId) {
      return;
    }
    currentSessionId = nextSessionId;
    files = [];
    error = null;
    if (nextSessionId) {
      void loadFiles(nextSessionId);
    }
  });

  async function loadFiles(sessionId = currentSessionId): Promise<void> {
    if (!sessionId) {
      return;
    }
    loading = true;
    error = null;
    try {
      const response = await controlClient.listSessionFiles(sessionId);
      if (currentSessionId === sessionId) {
        files = response.files;
      }
    } catch (loadError) {
      if (currentSessionId === sessionId) {
        error = errorMessage(loadError);
      }
    } finally {
      if (currentSessionId === sessionId) {
        loading = false;
      }
    }
  }

  async function downloadFile(file: SessionFileResource): Promise<void> {
    downloadingFileId = file.id;
    error = null;
    try {
      const blob = await controlClient.downloadSessionFileContent(file);
      const url = URL.createObjectURL(blob);
      try {
        const link = document.createElement('a');
        link.href = url;
        link.download = file.name;
        document.body.append(link);
        link.click();
        link.remove();
      } finally {
        URL.revokeObjectURL(url);
      }
    } catch (downloadError) {
      error = errorMessage(downloadError);
    } finally {
      downloadingFileId = null;
    }
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected session file error';
  }
</script>

<section class="files" aria-label="Session files">
  <div class="header">
    <div>
      <p class="eyebrow">Runtime files</p>
      <h2>{session ? `${files.length} session file${files.length === 1 ? '' : 's'}` : 'No session selected'}</h2>
    </div>
    <button
      type="button"
      data-testid="session-files-refresh"
      disabled={!session || loading || Boolean(downloadingFileId)}
      onclick={() => void loadFiles()}
    >
      {loading ? 'Loading...' : 'Refresh files'}
    </button>
  </div>

  {#if error}
    <p class="error" data-testid="session-files-error">{error}</p>
  {:else if !session}
    <p class="empty" data-testid="session-files-empty">Select a session to inspect file artifacts.</p>
  {:else if loading}
    <p class="empty">Loading runtime upload/download artifacts...</p>
  {:else if files.length === 0}
    <p class="empty" data-testid="session-files-empty">No runtime files are recorded for this session yet.</p>
  {:else}
    <div class="file-list">
      {#each files as file (file.id)}
        <SessionFileCard {file} {downloadingFileId} onDownload={(entry) => void downloadFile(entry)} />
      {/each}
    </div>
  {/if}
</section>

<style>
  .files {
    margin-top: 22px;
    padding: 24px;
    border: 1px solid rgba(24, 32, 24, 0.12);
    border-radius: 24px;
    background: rgba(255, 255, 248, 0.62);
    box-shadow: 0 18px 48px rgba(24, 32, 24, 0.08);
  }

  .header {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
  }

  .eyebrow {
    margin: 0 0 8px;
    color: #a9522f;
    font-size: 0.74rem;
    font-weight: 800;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    color: #243126;
    font-size: 1.15rem;
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

  .file-list {
    display: grid;
    gap: 12px;
    margin-top: 18px;
  }

  .error {
    margin: 18px 0 0;
    color: #a33a21;
    line-height: 1.5;
  }

  .empty {
    margin: 18px 0 0;
    line-height: 1.5;
  }

  @media (max-width: 760px) {
    .header {
      align-items: stretch;
      flex-direction: column;
    }
  }
</style>
