<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type {
    FileWorkspaceFileResource,
    FileWorkspaceResource,
  } from '../api/control-types';
  import AdminMessage from '../presentation/AdminMessage.svelte';
  import {
    FileWorkspaceViewModelBuilder,
    type FileWorkspaceFileViewModel,
  } from '../presentation/file-workspace-view-model';

  type AdminFileWorkspaceDetailRouteProps = {
    readonly controlClient: ControlClient;
    readonly workspaceId: string;
  };

  let { controlClient, workspaceId }: AdminFileWorkspaceDetailRouteProps = $props();
  let workspace = $state<FileWorkspaceResource | null>(null);
  let files = $state<readonly FileWorkspaceFileResource[]>([]);
  let loading = $state(false);
  let uploading = $state(false);
  let deletingFileId = $state<string | null>(null);
  let downloadingFileId = $state<string | null>(null);
  let error = $state<string | null>(null);
  let actionMessage = $state<string | null>(null);
  let lastRefreshedAt = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);

  const fileRows = $derived(files.map((file) => FileWorkspaceViewModelBuilder.workspaceFile(file)));

  onMount(() => {
    void loadDetail(false);
  });

  async function loadDetail(showFeedback = true): Promise<void> {
    loading = true;
    error = null;
    actionMessage = null;
    try {
      const [nextWorkspace, nextFiles] = await Promise.all([
        controlClient.getFileWorkspace(workspaceId),
        controlClient.listFileWorkspaceFiles(workspaceId),
      ]);
      workspace = nextWorkspace;
      files = nextFiles.files;
      lastRefreshedAt = new Date().toISOString();
      if (showFeedback) {
        actionMessage = `Refreshed ${nextFiles.files.length} workspace file${nextFiles.files.length === 1 ? '' : 's'}.`;
      }
    } catch (loadError) {
      error = errorMessage(loadError, 'Unexpected file workspace detail error');
      actionMessage = null;
    } finally {
      loading = false;
    }
  }

  async function uploadFile(): Promise<void> {
    const selectedFile = fileInput?.files?.[0];
    actionMessage = null;
    if (!selectedFile) {
      error = 'Choose a file before uploading.';
      return;
    }
    uploading = true;
    error = null;
    try {
      const uploaded = await controlClient.uploadFileWorkspaceFile(workspaceId, {
        fileName: selectedFile.name,
        mediaType: selectedFile.type || 'application/octet-stream',
        content: selectedFile,
        provenance: {
          source: 'admin-upload',
          uploaded_at: new Date().toISOString(),
        },
      });
      files = [uploaded, ...files.filter((file) => file.id !== uploaded.id)];
      if (fileInput) {
        fileInput.value = '';
      }
      actionMessage = `Uploaded ${uploaded.name}.`;
    } catch (uploadError) {
      error = errorMessage(uploadError, 'Unexpected workspace file upload error');
    } finally {
      uploading = false;
    }
  }

  async function downloadFile(fileId: string): Promise<void> {
    const file = files.find((entry) => entry.id === fileId);
    if (!file) {
      return;
    }
    downloadingFileId = fileId;
    error = null;
    actionMessage = null;
    try {
      const blob = await controlClient.downloadFileWorkspaceFileContent(file);
      triggerDownload(blob, file.name);
      actionMessage = `Download started for ${file.name}.`;
    } catch (downloadError) {
      error = errorMessage(downloadError, 'Unexpected workspace file download error');
    } finally {
      downloadingFileId = null;
    }
  }

  async function deleteFile(fileId: string): Promise<void> {
    deletingFileId = fileId;
    error = null;
    actionMessage = null;
    try {
      const deleted = await controlClient.deleteFileWorkspaceFile(workspaceId, fileId);
      files = files.filter((file) => file.id !== fileId);
      actionMessage = `Deleted ${deleted.name}.`;
    } catch (deleteError) {
      error = errorMessage(deleteError, 'Unexpected workspace file delete error');
    } finally {
      deletingFileId = null;
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

  function formatDate(value: string | null): string {
    return value ? new Date(value).toLocaleString() : 'not refreshed';
  }

  function errorMessage(value: unknown, fallback: string): string {
    return value instanceof Error ? value.message : fallback;
  }
</script>

<section class="grid gap-5" data-testid="file-workspace-detail">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div class="min-w-0">
        <p class="admin-eyebrow">File workspace detail</p>
        <h1
          class="m-0 max-w-full text-xl font-bold text-admin-ink [overflow-wrap:anywhere]"
          data-testid="file-workspace-detail-title"
        >
          {workspace?.name ?? workspaceId}
        </h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
        <a class="admin-button-ghost" href={`${base}/sessions`}>Sessions</a>
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="file-workspace-detail-refresh"
          disabled={loading || uploading || Boolean(deletingFileId)}
          onclick={() => void loadDetail()}
        >
          Refresh
        </button>
      </div>
    </div>
    <p class="m-0 mt-3 text-sm text-admin-ink/62" data-testid="file-workspace-detail-last-refresh">
      Last refreshed {formatDate(lastRefreshedAt)}
    </p>
  </div>

  {#if loading && !workspace}
    <section class="admin-panel mt-0">
      <AdminMessage variant="loading" message="Loading file workspace..." compact={true} />
    </section>
  {:else if error && !workspace}
    <section class="admin-panel mt-0">
      <AdminMessage variant="error" message={error} testId="file-workspace-detail-error" compact={true} />
    </section>
  {:else}
    {#if workspace}
      <section class="admin-panel mt-0 grid gap-3">
        <p class="admin-eyebrow">Workspace</p>
        <div class="grid gap-2 text-sm text-admin-ink/72 md:grid-cols-2">
          <span class="[overflow-wrap:anywhere]"><strong>Workspace id:</strong> <code class="admin-code-pill">{workspace.id}</code></span>
          <span><strong>Files:</strong> {files.length}</span>
          <span><strong>Description:</strong> {workspace.description ?? 'No description available.'}</span>
          <span><strong>Updated:</strong> {formatDate(workspace.updated_at)}</span>
        </div>
      </section>
    {/if}

    <section class="admin-panel mt-0">
      <div class="admin-header">
        <div>
          <p class="admin-eyebrow">Upload</p>
          <h2 class="admin-section-title">Add workspace file</h2>
        </div>
      </div>
      <form
        class="mt-4 grid gap-3 md:grid-cols-[minmax(220px,1fr)_auto]"
        onsubmit={(event) => {
          event.preventDefault();
          void uploadFile();
        }}
      >
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-admin-ink outline-none file:mr-3 file:rounded-lg file:border-0 file:bg-admin-leaf/15 file:px-3 file:py-1.5 file:text-admin-leaf"
          type="file"
          data-testid="file-workspace-upload-input"
          bind:this={fileInput}
        />
        <button
          class="admin-button-primary"
          type="submit"
          data-testid="file-workspace-upload-submit"
          disabled={uploading || loading}
        >
          {uploading ? 'Uploading...' : 'Upload file'}
        </button>
      </form>
      <p class="m-0 mt-3 text-sm text-admin-ink/58">
        Files are downloaded as attachments; active content is not previewed in the admin UI.
      </p>
    </section>

    {#if error}
      <AdminMessage variant="error" message={error} testId="file-workspace-action-error" compact={true} />
    {/if}
    {#if actionMessage}
      <AdminMessage variant="success" message={actionMessage} testId="file-workspace-action-message" compact={true} />
    {/if}

    <section class="admin-panel mt-0 grid gap-3">
      <div class="admin-header">
        <div>
          <p class="admin-eyebrow">Workspace files</p>
          <h2 class="admin-section-title">Reusable inputs</h2>
        </div>
      </div>
      {#if fileRows.length === 0}
        <AdminMessage
          variant="empty"
          message="No files have been uploaded to this workspace yet."
          testId="file-workspace-files-empty"
          compact={true}
        />
      {:else}
        <div class="grid gap-3">
          {#each fileRows as file}
            {@render FileRow(file)}
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</section>

{#snippet FileRow(file: FileWorkspaceFileViewModel)}
  <article
    class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-field p-4 lg:grid-cols-[minmax(0,1fr)_auto]"
    data-testid="file-workspace-file-row"
    data-file-id={file.id}
  >
    <div class="grid min-w-0 gap-1">
      <strong class="[overflow-wrap:anywhere]" data-testid="file-workspace-file-name">{file.name}</strong>
      <span class="text-xs text-admin-ink/62">{file.size} | {file.mediaType} | {file.createdAt}</span>
      <span class="font-mono text-xs text-admin-ink/58">{file.digest}</span>
      <span class="text-xs text-admin-ink/58">{file.provenance}</span>
    </div>
    <div class="flex flex-wrap items-start gap-2">
      <button
        class="admin-button-primary"
        type="button"
        data-testid="file-workspace-file-download"
        disabled={Boolean(downloadingFileId) || Boolean(deletingFileId)}
        onclick={() => void downloadFile(file.id)}
      >
        {downloadingFileId === file.id ? 'Downloading...' : 'Download'}
      </button>
      <button
        class="admin-button-ghost"
        type="button"
        data-testid="file-workspace-file-delete"
        disabled={Boolean(downloadingFileId) || Boolean(deletingFileId)}
        onclick={() => void deleteFile(file.id)}
      >
        {deletingFileId === file.id ? 'Deleting...' : 'Delete'}
      </button>
    </div>
  </article>
{/snippet}
