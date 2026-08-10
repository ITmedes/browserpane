<script lang="ts">
  import { Download, RefreshCw } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { SessionFileClient } from '$lib/session-files/session-file-client';
  import type { SessionFileResource } from '$lib/session-files/session-file-types';
  import { sessionFileRow } from '$lib/session-files/session-file-view-model';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';

  type SessionTransferFilesPanelProps = {
    readonly client: SessionFileClient;
    readonly sessionId: string;
    readonly transferPolicyMessage: string | null;
  };

  let { client, sessionId, transferPolicyMessage }: SessionTransferFilesPanelProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let files = $state<readonly SessionFileResource[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let downloadingId = $state<string | null>(null);
  let actionState = $state<AdminActionState>({ status: 'idle' });
  const rows = $derived(files.map(sessionFileRow));

  $effect(() => {
    if (sessionId === loadedSessionId) {
      return;
    }
    loadedSessionId = sessionId;
    files = [];
    loading = true;
    loadError = null;
    downloadingId = null;
    actionState = { status: 'idle' };
    void loadFiles(sessionId, false);
  });

  async function loadFiles(requestSessionId = sessionId, showFeedback = true): Promise<void> {
    loading = true;
    loadError = null;
    if (showFeedback) {
      actionState = { status: 'running', label: 'Refreshing captured files...' };
    }
    try {
      const response = await client.listSessionFiles(requestSessionId);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      files = response.files;
      if (showFeedback) {
        actionState = {
          status: 'success',
          message: `Refreshed ${files.length} captured file${files.length === 1 ? '' : 's'}.`,
        };
      }
    } catch (error) {
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      loadError = adminErrorMessage(error, 'Unexpected session file error.');
      actionState = showFeedback ? { status: 'error', message: loadError } : { status: 'idle' };
    } finally {
      if (loadedSessionId === requestSessionId) {
        loading = false;
      }
    }
  }

  async function downloadFile(fileId: string): Promise<void> {
    const requestSessionId = sessionId;
    const file = files.find((candidate) => candidate.id === fileId);
    if (!file) {
      return;
    }
    downloadingId = file.id;
    actionState = { status: 'running', label: `Downloading ${file.name}...` };
    try {
      const blob = await client.downloadSessionFile(file);
      triggerDownload(blob, file.name);
      if (loadedSessionId === requestSessionId) {
        actionState = { status: 'success', message: `Download started for ${file.name}.` };
      }
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Session file download failed.'),
        };
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        downloadingId = null;
      }
    }
  }

  function triggerDownload(blob: Blob, fileName: string): void {
    const url = URL.createObjectURL(blob);
    try {
      const link = document.createElement('a');
      link.href = url;
      link.download = fileName;
      document.body.append(link);
      link.click();
      link.remove();
    } finally {
      URL.revokeObjectURL(url);
    }
  }
</script>

<section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-transfer-files-panel">
  <div class="flex flex-col gap-3 border-b border-admin-border p-4 sm:flex-row sm:items-start sm:justify-between">
    <div>
      <h2 class="m-0 text-base font-semibold text-admin-ink">Captured transfers</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">{rows.length} retained upload/download artifact{rows.length === 1 ? '' : 's'}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void loadFiles()}
      disabled={loading || downloadingId !== null}
      data-testid="session-files-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh</span>
    </button>
  </div>

  <div class="grid gap-3 p-4">
    <ActionFeedback
      state={actionState}
      successTitle="File action completed"
      errorTitle="File action failed"
      successTestId="session-files-action-success"
      errorTestId="session-files-action-error"
      runningTestId="session-files-action-running"
    />

    {#if transferPolicyMessage}
      <AdminMessage
        tone="warning"
        density="compact"
        title="Live file transfer blocked"
        message={transferPolicyMessage}
        testId="session-files-policy-blocked"
      />
    {/if}

    {#if loading && files.length === 0}
      <AdminMessage tone="loading" density="compact" message="Loading captured session files..." testId="session-files-loading" />
    {:else if loadError}
      <AdminMessage tone="error" density="compact" title="Captured files unavailable" message={loadError} testId="session-files-error" />
    {:else if rows.length === 0}
      <div class="rounded-md border border-dashed border-admin-border bg-admin-soft/50 p-5 text-center text-sm text-admin-muted" data-testid="session-files-empty">
        No captured session files.
      </div>
    {:else}
      <div class="grid gap-2" data-testid="session-files-list">
        {#each rows as row}
          <article class="grid min-w-0 gap-3 rounded-md border border-admin-border bg-admin-soft/50 p-3 lg:grid-cols-[minmax(0,1fr)_auto]" data-testid="session-file-row">
            <div class="min-w-0">
              <strong class="block break-all text-sm font-semibold text-admin-ink" data-testid="session-file-name">{row.name}</strong>
              <p class="m-0 mt-1 text-xs text-admin-muted">{row.source} | {row.size} | {row.mediaType}</p>
              <p class="m-0 mt-1 break-all font-mono text-xs text-admin-muted">{row.digest}</p>
              <p class="m-0 mt-1 text-xs text-admin-muted">{row.createdAt}</p>
            </div>
            <button
              class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-white disabled:cursor-not-allowed disabled:opacity-60"
              type="button"
              onclick={() => void downloadFile(row.id)}
              disabled={downloadingId !== null}
              data-testid="session-file-download"
            >
              <Download size={15} strokeWidth={1.8} />
              <span>{downloadingId === row.id ? 'Downloading...' : 'Download'}</span>
            </button>
          </article>
        {/each}
      </div>
    {/if}
  </div>
</section>
