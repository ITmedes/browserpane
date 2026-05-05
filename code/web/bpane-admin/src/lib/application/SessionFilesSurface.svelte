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

<section class="admin-panel" aria-label="Session files">
  <div class="admin-header max-[760px]:flex-col max-[760px]:items-stretch">
    <div>
      <p class="admin-eyebrow admin-eyebrow-warm">Runtime files</p>
      <h2 class="m-0 text-[1.15rem] font-bold text-admin-night">
        {session ? `${files.length} session file${files.length === 1 ? '' : 's'}` : 'No session selected'}
      </h2>
    </div>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="session-files-refresh"
      disabled={!session || loading || Boolean(downloadingFileId)}
      onclick={() => void loadFiles()}
    >
      {loading ? 'Loading...' : 'Refresh files'}
    </button>
  </div>

  {#if error}
    <p class="admin-error" data-testid="session-files-error">{error}</p>
  {:else if !session}
    <p class="admin-empty" data-testid="session-files-empty">Select a session to inspect file artifacts.</p>
  {:else if loading}
    <p class="admin-empty">Loading runtime upload/download artifacts...</p>
  {:else if files.length === 0}
    <p class="admin-empty" data-testid="session-files-empty">No runtime files are recorded for this session yet.</p>
  {:else}
    <div class="mt-[18px] grid gap-3">
      {#each files as file (file.id)}
        <SessionFileCard {file} {downloadingFileId} onDownload={(entry) => void downloadFile(entry)} />
      {/each}
    </div>
  {/if}
</section>
