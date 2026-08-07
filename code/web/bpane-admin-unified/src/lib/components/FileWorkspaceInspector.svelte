<script lang="ts">
  import { Copy, Download, RefreshCw, Trash2, Upload } from '@lucide/svelte';
  import type {
    FileWorkspaceActionState,
    FileWorkspaceDetailLoadState,
  } from '$lib/file-workspaces/file-workspace-detail-state';
  import {
    fileWorkspaceFileRow,
    labelSummary,
  } from '$lib/file-workspaces/file-workspace-overview-view-model';
  import type { FileWorkspaceFileResource } from '$lib/file-workspaces/file-workspace-types';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import ActionFeedback from './ActionFeedback.svelte';

  type FileWorkspaceInspectorProps = {
    readonly state: FileWorkspaceDetailLoadState;
    readonly actionState?: FileWorkspaceActionState;
    readonly onRefreshWorkspace?: () => void | Promise<void>;
    readonly onUploadFile?: (file: File) => void | Promise<void>;
    readonly onDownloadFile?: (file: FileWorkspaceFileResource) => void | Promise<void>;
    readonly onDeleteFile?: (file: FileWorkspaceFileResource) => void | Promise<void>;
  };

  let {
    state: loadState,
    actionState = { status: 'idle' },
    onRefreshWorkspace,
    onUploadFile,
    onDownloadFile,
    onDeleteFile,
  }: FileWorkspaceInspectorProps = $props();
  let fileInput = $state<HTMLInputElement | null>(null);
  let uploadError = $state<string | null>(null);

  const busy = $derived(actionState.status === 'running');
  const fileRows = $derived(loadState.status === 'ready'
    ? loadState.files.map((file) => ({ resource: file, row: fileWorkspaceFileRow(file) }))
    : []);

  function refreshWorkspace(): void {
    void onRefreshWorkspace?.();
  }

  function uploadFile(): void {
    const file = fileInput?.files?.[0] ?? null;
    if (!file) {
      uploadError = 'Choose a file before uploading.';
      return;
    }
    uploadError = null;
    if (fileInput) {
      fileInput.value = '';
    }
    void onUploadFile?.(file);
  }

  function downloadFile(file: FileWorkspaceFileResource): void {
    void onDownloadFile?.(file);
  }

  function deleteFile(file: FileWorkspaceFileResource): void {
    void onDeleteFile?.(file);
  }

  async function copyWorkspaceId(workspaceId: string): Promise<void> {
    await navigator.clipboard?.writeText(workspaceId);
  }

  function scopeLabel(): string {
    if (loadState.status !== 'ready') {
      return '';
    }
    return loadState.workspace.project?.name ?? loadState.workspace.project_id ?? 'Owner scoped';
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="file-workspace-inspector">
  {#if loadState.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-center text-sm text-admin-muted" data-testid="file-workspace-inspector-idle">
      Select a file workspace to inspect.
    </div>
  {:else if loadState.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="file-workspace-inspector-loading">
      Loading file workspace...
    </div>
  {:else if loadState.status === 'error'}
    <div class="p-4">
      <AdminMessage
        tone="error"
        title="File workspace unavailable"
        message={loadState.message}
        testId="file-workspace-inspector-error"
      />
    </div>
  {:else}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 min-w-0 max-w-full truncate text-xl font-semibold text-admin-ink" data-testid="file-workspace-detail-name">{loadState.workspace.name}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(loadState.workspace.project_id ? 'warning' : 'neutral')}`}>
              {scopeLabel()}
            </span>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(loadState.files.length > 0 ? 'success' : 'neutral')}`} data-testid="file-workspace-detail-file-count">
              {loadState.files.length === 1 ? '1 file' : `${loadState.files.length} files`}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{loadState.workspace.description ?? 'No description available.'}</p>
          <p class="m-0 mt-2 min-w-0 truncate font-mono text-xs text-admin-muted">{loadState.workspace.id}</p>
        </div>

        <div class="flex shrink-0 flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            type="button"
            onclick={() => void copyWorkspaceId(loadState.workspace.id)}
            data-testid="file-workspace-copy-id"
          >
            <Copy size={15} strokeWidth={1.8} />
            <span>Copy ID</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={refreshWorkspace}
            disabled={busy}
            data-testid="file-workspace-refresh-detail"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Refresh</span>
          </button>
        </div>
      </div>
    </div>

    <div class="border-b border-admin-border p-4">
      <ActionFeedback
        state={actionState}
        successTitle="File workspace action completed"
        errorTitle="File workspace action failed"
        successTestId="file-workspace-action-success"
        errorTestId="file-workspace-action-error"
        runningTestId="file-workspace-action-running"
      />
    </div>

    <div class="grid gap-4 p-4">
      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="file-workspace-detail-configuration">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Workspace</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
            Existing workspace metadata is read-only in the current control API. Files can be managed below.
          </p>
        </div>

        <dl class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Scope</dt>
            <dd class="m-0 mt-1 truncate text-sm font-medium text-admin-ink">{scopeLabel()}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Files path</dt>
            <dd class="m-0 mt-1 truncate font-mono text-xs text-admin-ink">{loadState.workspace.files_path}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Updated</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">{formatDateTime(loadState.workspace.updated_at)}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3 md:col-span-2 xl:col-span-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Labels</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="file-workspace-detail-labels">{labelSummary(loadState.workspace.labels)}</dd>
          </div>
        </dl>
      </section>

      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="file-workspace-upload-panel">
        <div class="flex flex-col gap-3 border-b border-admin-border pb-3 sm:flex-row sm:items-start sm:justify-between">
          <div class="min-w-0">
            <h3 class="m-0 text-sm font-semibold text-admin-ink">Upload file</h3>
            <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
              Files are stored as workspace artifacts and downloaded as attachments.
            </p>
          </div>
        </div>

        <form
          class="mt-4 grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]"
          onsubmit={(event) => {
            event.preventDefault();
            uploadFile();
          }}
        >
          <input
            class="min-h-10 min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 text-sm text-admin-ink outline-none file:mr-3 file:rounded-md file:border-0 file:bg-admin-soft file:px-3 file:py-1.5 file:text-admin-ink focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="file"
            bind:this={fileInput}
            disabled={busy}
            data-testid="file-workspace-upload-input"
          />
          <button
            class="inline-flex h-10 w-full items-center justify-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60 md:w-auto"
            type="submit"
            disabled={busy}
            data-testid="file-workspace-upload-submit"
          >
            <Upload size={16} strokeWidth={1.8} />
            <span>Upload</span>
          </button>
        </form>
        {#if uploadError}
          <div class="mt-3">
            <AdminMessage tone="error" title="Upload unavailable" message={uploadError} density="compact" testId="file-workspace-upload-error" />
          </div>
        {/if}
      </section>

      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="file-workspace-files-panel">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Workspace files</h3>
        </div>

        {#if fileRows.length === 0}
          <p class="m-0 mt-4 rounded-md border border-dashed border-admin-border bg-admin-panel p-4 text-sm text-admin-muted" data-testid="file-workspace-files-empty">
            No files have been uploaded to this workspace yet.
          </p>
        {:else}
          <div class="mt-4 grid gap-3">
            {#each fileRows as item}
              <article
                class="grid min-w-0 gap-3 rounded-md border border-admin-border bg-admin-panel p-3 lg:grid-cols-[minmax(0,1fr)_auto]"
                data-testid="file-workspace-file-row"
                data-file-id={item.row.id}
              >
                <div class="grid min-w-0 gap-1">
                  <strong class="break-words text-sm text-admin-ink" data-testid="file-workspace-file-name">{item.row.name}</strong>
                  <span class="text-xs text-admin-muted">{item.row.size} · {item.row.mediaType} · {item.row.createdAt}</span>
                  <span class="truncate font-mono text-xs text-admin-muted">{item.row.digest}</span>
                  <span class="line-clamp-2 text-xs text-admin-muted">{item.row.provenance}</span>
                </div>
                <div class="flex flex-wrap items-start gap-2">
                  <button
                    class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
                    type="button"
                    onclick={() => downloadFile(item.resource)}
                    disabled={busy}
                    data-testid="file-workspace-file-download"
                  >
                    <Download size={15} strokeWidth={1.8} />
                    <span>Download</span>
                  </button>
                  <button
                    class="inline-flex h-9 items-center gap-2 rounded-md border border-rose-200 bg-rose-50 px-3 text-sm font-medium text-rose-800 hover:bg-rose-100 disabled:cursor-not-allowed disabled:opacity-60"
                    type="button"
                    onclick={() => deleteFile(item.resource)}
                    disabled={busy}
                    data-testid="file-workspace-file-delete"
                  >
                    <Trash2 size={15} strokeWidth={1.8} />
                    <span>Delete</span>
                  </button>
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {/if}
</aside>
