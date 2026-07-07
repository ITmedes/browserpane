<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
  import type {
    FileWorkspaceActionState,
    FileWorkspaceProjectOptionsLoadState,
  } from '$lib/file-workspaces/file-workspace-detail-state';
  import type { CreateFileWorkspaceRequest, FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
  import AdminMessage from './AdminMessage.svelte';
  import FileWorkspaceEditForm from './FileWorkspaceEditForm.svelte';

  type FileWorkspaceCreateRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToWorkspace?: (workspace: FileWorkspaceResource) => void;
  };

  let {
    authContext,
    navigateToWorkspace = defaultNavigateToWorkspace,
  }: FileWorkspaceCreateRouteProps = $props();
  let actionState = $state<FileWorkspaceActionState>({ status: 'idle' });
  let projectOptionsState = $state<FileWorkspaceProjectOptionsLoadState>({ status: 'idle' });

  const busy = $derived(actionState.status === 'running');

  onMount(() => {
    void loadProjectOptions();
  });

  function client(): FileWorkspaceCatalogClient {
    return new FileWorkspaceCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadProjectOptions(): Promise<void> {
    projectOptionsState = { status: 'loading' };
    try {
      const options = await client().listProjectOptions();
      projectOptionsState = { status: 'ready', projects: options.projects };
    } catch (error) {
      projectOptionsState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project options load failed.',
      };
    }
  }

  async function createWorkspace(request: CreateFileWorkspaceRequest): Promise<void> {
    actionState = { status: 'running', label: 'Creating file workspace...' };
    try {
      const workspace = await client().createFileWorkspace(request);
      actionState = { status: 'success', message: 'File workspace created.' };
      navigateToWorkspace(workspace);
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'File workspace creation failed.',
      };
    }
  }

  function defaultNavigateToWorkspace(workspace: FileWorkspaceResource): void {
    window.location.assign(`/admin-new/files/workspaces/${encodeURIComponent(workspace.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="file-workspace-create-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/files/workspaces"
        data-testid="file-workspace-create-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>File workspaces</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New file workspace</h1>
    </div>
  </header>

  {#if actionState.status === 'success'}
    <AdminMessage tone="success" title="File workspace action completed" message={actionState.message} testId="file-workspace-create-success" />
  {:else if actionState.status === 'error'}
    <AdminMessage tone="error" title="File workspace action failed" message={actionState.message} testId="file-workspace-create-error" />
  {:else if actionState.status === 'running'}
    <AdminMessage tone="loading" title={actionState.label} testId="file-workspace-create-running" />
  {/if}

  <FileWorkspaceEditForm
    disabled={busy}
    {projectOptionsState}
    onSave={createWorkspace}
  />
</div>
