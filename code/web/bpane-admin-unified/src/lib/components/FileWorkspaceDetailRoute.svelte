<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
  import type {
    FileWorkspaceActionState,
    FileWorkspaceDetailLoadState,
  } from '$lib/file-workspaces/file-workspace-detail-state';
  import type { FileWorkspaceFileResource } from '$lib/file-workspaces/file-workspace-types';
  import AdminMessage from './AdminMessage.svelte';
  import FileWorkspaceInspector from './FileWorkspaceInspector.svelte';

  type FileWorkspaceDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: FileWorkspaceDetailRouteProps = $props();
  let workspaceState = $state<FileWorkspaceDetailLoadState>({ status: 'idle' });
  let actionState = $state<FileWorkspaceActionState>({ status: 'idle' });

  onMount(() => {
    const workspaceId = currentWorkspaceId();
    if (!workspaceId) {
      workspaceState = {
        status: 'error',
        workspaceId: 'unknown',
        message: 'File workspace id is missing from the current route.',
      };
      return;
    }
    void loadWorkspace(workspaceId);
  });

  function client(): FileWorkspaceCatalogClient {
    return new FileWorkspaceCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadWorkspace(workspaceId: string): Promise<void> {
    workspaceState = { status: 'loading', workspaceId };
    actionState = { status: 'idle' };
    try {
      const workspaceClient = client();
      const [workspace, files] = await Promise.all([
        workspaceClient.getFileWorkspace(workspaceId),
        workspaceClient.listFileWorkspaceFiles(workspaceId),
      ]);
      workspaceState = { status: 'ready', workspace, files: files.files };
    } catch (error) {
      workspaceState = {
        status: 'error',
        workspaceId,
        message: adminErrorMessage(error, 'Unexpected file workspace detail error.'),
      };
    }
  }

  async function refreshWorkspace(): Promise<void> {
    const workspaceId = activeWorkspaceId();
    if (!workspaceId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing file workspace...' };
    try {
      const workspaceClient = client();
      const [workspace, files] = await Promise.all([
        workspaceClient.getFileWorkspace(workspaceId),
        workspaceClient.listFileWorkspaceFiles(workspaceId),
      ]);
      workspaceState = { status: 'ready', workspace, files: files.files };
      actionState = { status: 'success', message: 'File workspace refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'File workspace refresh failed.'),
      };
    }
  }

  async function uploadFile(file: File): Promise<void> {
    if (workspaceState.status !== 'ready') {
      return;
    }
    const workspaceId = workspaceState.workspace.id;
    actionState = { status: 'running', label: `Uploading ${file.name}...` };
    try {
      const uploaded = await client().uploadFileWorkspaceFile(workspaceId, {
        fileName: file.name,
        mediaType: file.type || 'application/octet-stream',
        content: file,
        provenance: {
          source: 'unified-admin-upload',
          uploaded_at: new Date().toISOString(),
        },
      });
      workspaceState = {
        ...workspaceState,
        files: [uploaded, ...workspaceState.files.filter((item) => item.id !== uploaded.id)],
      };
      actionState = { status: 'success', message: `Uploaded ${uploaded.name}.` };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Workspace file upload failed.'),
      };
    }
  }

  async function downloadFile(file: FileWorkspaceFileResource): Promise<void> {
    actionState = { status: 'running', label: `Downloading ${file.name}...` };
    try {
      const blob = await client().downloadFileWorkspaceFileContent(file);
      triggerDownload(blob, file.name);
      actionState = { status: 'success', message: `Download started for ${file.name}.` };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Workspace file download failed.'),
      };
    }
  }

  async function deleteFile(file: FileWorkspaceFileResource): Promise<void> {
    if (workspaceState.status !== 'ready') {
      return;
    }
    const workspaceId = workspaceState.workspace.id;
    actionState = { status: 'running', label: `Deleting ${file.name}...` };
    try {
      const deleted = await client().deleteFileWorkspaceFile(workspaceId, file.id);
      workspaceState = {
        ...workspaceState,
        files: workspaceState.files.filter((item) => item.id !== file.id),
      };
      actionState = { status: 'success', message: `Deleted ${deleted.name}.` };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Workspace file delete failed.'),
      };
    }
  }

  function currentWorkspaceId(): string | null {
    const match = window.location.pathname.match(/\/files\/workspaces\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeWorkspaceId(): string | null {
    if (workspaceState.status === 'ready') {
      return workspaceState.workspace.id;
    }
    if (workspaceState.status === 'loading' || workspaceState.status === 'error') {
      return workspaceState.workspaceId;
    }
    return null;
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

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="file-workspace-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/files/workspaces"
        data-testid="file-workspace-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>File workspaces</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">File workspace details</h1>
    </div>
  </header>

  {#if workspaceState.status === 'error'}
    <div data-testid="file-workspace-detail-error">
      <AdminMessage tone="error" title="File workspace detail unavailable" message={workspaceState.message} />
    </div>
  {:else}
    <FileWorkspaceInspector
      state={workspaceState}
      {actionState}
      onRefreshWorkspace={refreshWorkspace}
      onUploadFile={uploadFile}
      onDownloadFile={downloadFile}
      onDeleteFile={deleteFile}
    />
  {/if}
</div>
