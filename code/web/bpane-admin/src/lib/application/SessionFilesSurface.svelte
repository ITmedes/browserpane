<script lang="ts">
  import { base } from '$app/paths';
  import type { ControlClient } from '../api/control-client';
  import type { SessionFileResource, SessionResource } from '../api/control-types';
  import SessionFileCard from '../presentation/SessionFileCard.svelte';
  import { SessionFileViewModelBuilder } from '../presentation/session-file-view-model';

  type SessionFilesSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly session: SessionResource | null;
    readonly refreshVersion: number;
    readonly onFileCountChange?: (count: number) => void;
  };

  let { controlClient, session, refreshVersion, onFileCountChange }: SessionFilesSurfaceProps = $props();
  let currentSessionId = $state<string | null>(null);
  let lastRefreshVersion = $state(0);
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
    onFileCountChange?.(0);
    error = null;
    if (nextSessionId) {
      void loadFiles(nextSessionId);
    }
  });

  $effect(() => {
    if (refreshVersion === lastRefreshVersion) {
      return;
    }
    lastRefreshVersion = refreshVersion;
    if (currentSessionId) {
      void loadFiles(currentSessionId);
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
        onFileCountChange?.(response.files.length);
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

  async function downloadFile(fileId: string): Promise<void> {
    const file = files.find((entry) => entry.id === fileId);
    if (!file) {
      return;
    }
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

<section class="grid gap-4" aria-label="Session files">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <span class="text-sm font-bold text-admin-ink/68">
      {session ? `${files.length} session file${files.length === 1 ? '' : 's'}` : 'No session selected'}
    </span>
    <div class="flex flex-wrap gap-2">
      <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
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
        <SessionFileCard
          viewModel={SessionFileViewModelBuilder.card(file)}
          {downloadingFileId}
          onDownload={(fileId) => void downloadFile(fileId)}
        />
      {/each}
    </div>
  {/if}
</section>
