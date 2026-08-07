<script lang="ts">
  import { Download, Link2, RefreshCw, Trash2 } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
  import type {
    FileWorkspaceFileResource,
    FileWorkspaceResource,
  } from '$lib/file-workspaces/file-workspace-types';
  import type { SessionFileClient } from '$lib/session-files/session-file-client';
  import type {
    SessionFileBindingMode,
    SessionFileBindingResource,
  } from '$lib/session-files/session-file-types';
  import {
    sessionFileBindingRow,
    validateSessionMountPath,
  } from '$lib/session-files/session-file-view-model';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';

  type SessionFileBindingsPanelProps = {
    readonly sessionClient: SessionFileClient;
    readonly workspaceClient: FileWorkspaceCatalogClient;
    readonly sessionId: string;
    readonly projectId?: string | null;
    readonly mutationAllowed: boolean;
    readonly policyMessage?: string | null;
    readonly allowedWorkspaceIds?: readonly string[];
  };

  let {
    sessionClient,
    workspaceClient,
    sessionId,
    projectId = null,
    mutationAllowed,
    policyMessage = null,
    allowedWorkspaceIds = [],
  }: SessionFileBindingsPanelProps = $props();

  let loadedSessionId = $state<string | null>(null);
  let bindings = $state<readonly SessionFileBindingResource[]>([]);
  let workspaces = $state<readonly FileWorkspaceResource[]>([]);
  let workspaceFiles = $state<readonly FileWorkspaceFileResource[]>([]);
  let selectedWorkspaceId = $state('');
  let selectedFileId = $state('');
  let mountPath = $state('');
  let mode = $state<SessionFileBindingMode>('read_only');
  let loading = $state(true);
  let loadingFiles = $state(false);
  let bindingError = $state<string | null>(null);
  let workspaceError = $state<string | null>(null);
  let mutating = $state(false);
  let downloadingId = $state<string | null>(null);
  let actionState = $state<AdminActionState>({ status: 'idle' });

  const rows = $derived(bindings.filter((binding) => binding.state !== 'removed').map(sessionFileBindingRow));
  const eligibleWorkspaces = $derived(workspaces.filter((workspace) => {
    if (workspace.project_id && workspace.project_id !== projectId) {
      return false;
    }
    return allowedWorkspaceIds.length === 0 || allowedWorkspaceIds.includes(workspace.id);
  }));
  const validation = $derived(validateSessionMountPath(mountPath, rows.map((row) => row.mountPath)));
  const selectedFile = $derived(workspaceFiles.find((file) => file.id === selectedFileId) ?? null);
  const formDisabled = $derived(
    !mutationAllowed
      || loading
      || loadingFiles
      || mutating
      || !selectedWorkspaceId
      || !selectedFileId
      || !validation.valid,
  );

  $effect(() => {
    if (sessionId === loadedSessionId) {
      return;
    }
    loadedSessionId = sessionId;
    resetState();
    void loadCatalogs(sessionId, false);
  });

  function resetState(): void {
    bindings = [];
    workspaces = [];
    workspaceFiles = [];
    selectedWorkspaceId = '';
    selectedFileId = '';
    mountPath = '';
    mode = 'read_only';
    loading = true;
    loadingFiles = false;
    bindingError = null;
    workspaceError = null;
    mutating = false;
    downloadingId = null;
    actionState = { status: 'idle' };
  }

  async function loadCatalogs(requestSessionId = sessionId, showFeedback = true): Promise<void> {
    loading = true;
    bindingError = null;
    workspaceError = null;
    if (showFeedback) {
      actionState = { status: 'running', label: 'Refreshing file bindings...' };
    }
    const [bindingResult, workspaceResult] = await Promise.allSettled([
      sessionClient.listSessionFileBindings(requestSessionId),
      workspaceClient.listFileWorkspaces(),
    ]);
    if (loadedSessionId !== requestSessionId) {
      return;
    }
    if (bindingResult.status === 'fulfilled') {
      bindings = bindingResult.value.bindings;
    } else {
      bindings = [];
      bindingError = adminErrorMessage(bindingResult.reason, 'Session file bindings are unavailable.');
    }
    if (workspaceResult.status === 'fulfilled') {
      workspaces = workspaceResult.value.workspaces;
      const firstWorkspaceId = eligibleWorkspaceId(workspaceResult.value.workspaces);
      selectedWorkspaceId = firstWorkspaceId;
      if (firstWorkspaceId) {
        void loadWorkspaceFiles(firstWorkspaceId, requestSessionId);
      }
    } else {
      workspaces = [];
      selectedWorkspaceId = '';
      selectedFileId = '';
      workspaceError = adminErrorMessage(workspaceResult.reason, 'File workspaces are unavailable.');
    }
    loading = false;
    if (showFeedback && !bindingError && !workspaceError) {
      actionState = {
        status: 'success',
        message: `Refreshed ${bindings.length} binding${bindings.length === 1 ? '' : 's'}.`,
      };
    } else if (showFeedback && (bindingError || workspaceError)) {
      actionState = {
        status: 'error',
        message: [bindingError, workspaceError].filter(Boolean).join(' '),
      };
    }
  }

  function eligibleWorkspaceId(candidates: readonly FileWorkspaceResource[]): string {
    return candidates.find((workspace) => {
      const projectMatches = !workspace.project_id || workspace.project_id === projectId;
      const policyMatches = allowedWorkspaceIds.length === 0 || allowedWorkspaceIds.includes(workspace.id);
      return projectMatches && policyMatches;
    })?.id ?? '';
  }

  async function selectWorkspace(workspaceId: string): Promise<void> {
    selectedWorkspaceId = workspaceId;
    selectedFileId = '';
    workspaceFiles = [];
    mountPath = '';
    await loadWorkspaceFiles(workspaceId, sessionId);
  }

  async function loadWorkspaceFiles(workspaceId: string, requestSessionId: string): Promise<void> {
    loadingFiles = true;
    workspaceError = null;
    try {
      const response = await workspaceClient.listFileWorkspaceFiles(workspaceId);
      if (loadedSessionId !== requestSessionId || selectedWorkspaceId !== workspaceId) {
        return;
      }
      workspaceFiles = response.files;
      selectedFileId = response.files[0]?.id ?? '';
      mountPath = response.files[0] ? `inputs/${response.files[0].name}` : '';
    } catch (error) {
      if (loadedSessionId === requestSessionId && selectedWorkspaceId === workspaceId) {
        workspaceFiles = [];
        selectedFileId = '';
        workspaceError = adminErrorMessage(error, 'Workspace files are unavailable.');
      }
    } finally {
      if (loadedSessionId === requestSessionId && selectedWorkspaceId === workspaceId) {
        loadingFiles = false;
      }
    }
  }

  function selectFile(fileId: string): void {
    selectedFileId = fileId;
    const file = workspaceFiles.find((candidate) => candidate.id === fileId);
    if (file && !mountPath.trim()) {
      mountPath = `inputs/${file.name}`;
    }
  }

  async function createBinding(): Promise<void> {
    if (formDisabled || !selectedFile) {
      return;
    }
    const requestSessionId = sessionId;
    mutating = true;
    actionState = { status: 'running', label: `Binding ${selectedFile.name}...` };
    try {
      const created = await sessionClient.createSessionFileBinding(requestSessionId, {
        workspace_id: selectedWorkspaceId,
        file_id: selectedFile.id,
        mount_path: validation.value,
        mode,
        labels: { source: 'admin-new' },
      });
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      bindings = [created, ...bindings.filter((binding) => binding.id !== created.id)];
      mountPath = '';
      actionState = { status: 'success', message: `Bound ${created.file_name} to ${created.mount_path}.` };
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Session file binding create failed.'),
        };
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        mutating = false;
      }
    }
  }

  async function removeBinding(bindingId: string): Promise<void> {
    const requestSessionId = sessionId;
    const binding = bindings.find((candidate) => candidate.id === bindingId);
    if (!binding || !mutationAllowed) {
      return;
    }
    mutating = true;
    actionState = { status: 'running', label: `Removing ${binding.file_name}...` };
    try {
      const removed = await sessionClient.removeSessionFileBinding(requestSessionId, bindingId);
      if (loadedSessionId === requestSessionId) {
        bindings = bindings.filter((candidate) => candidate.id !== bindingId);
        actionState = { status: 'success', message: `Removed binding for ${removed.file_name}.` };
      }
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Session file binding removal failed.'),
        };
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        mutating = false;
      }
    }
  }

  async function downloadBinding(bindingId: string): Promise<void> {
    const requestSessionId = sessionId;
    const binding = bindings.find((candidate) => candidate.id === bindingId);
    if (!binding) {
      return;
    }
    downloadingId = bindingId;
    actionState = { status: 'running', label: `Downloading ${binding.file_name}...` };
    try {
      const blob = await sessionClient.downloadSessionFileBinding(binding);
      triggerDownload(blob, binding.file_name);
      if (loadedSessionId === requestSessionId) {
        actionState = { status: 'success', message: `Download started for ${binding.file_name}.` };
      }
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Bound file download failed.'),
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

<section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-file-bindings-panel">
  <div class="flex flex-col gap-3 border-b border-admin-border p-4 sm:flex-row sm:items-start sm:justify-between">
    <div>
      <h2 class="m-0 text-base font-semibold text-admin-ink">Workspace bindings</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">{rows.length} mounted workspace input{rows.length === 1 ? '' : 's'}</p>
    </div>
    <div class="flex flex-wrap gap-2">
      <a class="inline-flex h-9 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft" href="/admin-new/files/workspaces">
        Workspaces
      </a>
      <button
        class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void loadCatalogs()}
        disabled={loading || mutating || downloadingId !== null}
        data-testid="session-file-bindings-refresh"
      >
        <RefreshCw size={15} strokeWidth={1.8} />
        <span>Refresh</span>
      </button>
    </div>
  </div>

  <div class="grid gap-4 p-4">
    <ActionFeedback
      state={actionState}
      successTitle="Binding action completed"
      errorTitle="Binding action failed"
      successTestId="session-file-bindings-action-success"
      errorTestId="session-file-bindings-action-error"
      runningTestId="session-file-bindings-action-running"
    />

    {#if !mutationAllowed}
      <AdminMessage
        tone={policyMessage ? 'warning' : 'info'}
        density="compact"
        title="Binding changes unavailable"
        message={policyMessage ?? 'Binding policy could not be confirmed. Refresh the session before changing bindings.'}
        testId="session-file-bindings-policy-blocked"
      />
    {/if}

    <form
      class="grid gap-3 rounded-md border border-admin-border bg-admin-soft/50 p-4 lg:grid-cols-2 xl:grid-cols-4"
      data-testid="session-file-binding-form"
      onsubmit={(event) => {
        event.preventDefault();
        void createBinding();
      }}
    >
      <label class="grid min-w-0 gap-1 text-sm font-medium text-admin-ink">
        <span>Workspace</span>
        <select
          class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-panel px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
          value={selectedWorkspaceId}
          onchange={(event) => void selectWorkspace(event.currentTarget.value)}
          disabled={!mutationAllowed || loading || mutating || eligibleWorkspaces.length === 0}
          data-testid="session-file-binding-workspace"
        >
          {#each eligibleWorkspaces as workspace}
            <option value={workspace.id}>{workspace.name}</option>
          {/each}
        </select>
      </label>
      <label class="grid min-w-0 gap-1 text-sm font-medium text-admin-ink">
        <span>Workspace file</span>
        <select
          class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-panel px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
          value={selectedFileId}
          onchange={(event) => selectFile(event.currentTarget.value)}
          disabled={!mutationAllowed || loadingFiles || mutating || workspaceFiles.length === 0}
          data-testid="session-file-binding-file"
        >
          {#each workspaceFiles as file}
            <option value={file.id}>{file.name}</option>
          {/each}
        </select>
      </label>
      <label class="grid min-w-0 gap-1 text-sm font-medium text-admin-ink">
        <span>Mount path</span>
        <input
          class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-panel px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
          bind:value={mountPath}
          disabled={!mutationAllowed || mutating}
          placeholder="inputs/report.pdf"
          aria-describedby="session-file-binding-path-feedback"
          data-testid="session-file-binding-mount-path"
        />
        <span id="session-file-binding-path-feedback" class={`text-xs ${mountPath && !validation.valid ? 'text-red-700' : 'text-admin-muted'}`} data-testid="session-file-binding-path-validation">
          {mountPath ? validation.message : 'Use a relative path without traversal components.'}
        </span>
      </label>
      <label class="grid min-w-0 gap-1 text-sm font-medium text-admin-ink">
        <span>Mode</span>
        <select
          class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-panel px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
          bind:value={mode}
          disabled={!mutationAllowed || mutating}
          data-testid="session-file-binding-mode"
        >
          <option value="read_only">Read only</option>
          <option value="read_write">Read/write</option>
          <option value="scratch_output">Scratch output</option>
        </select>
      </label>
      <div class="flex items-end xl:col-span-4">
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
          type="submit"
          disabled={formDisabled}
          data-testid="session-file-binding-create"
        >
          <Link2 size={15} strokeWidth={1.8} />
          <span>{mutating ? 'Saving...' : 'Bind file'}</span>
        </button>
      </div>
    </form>

    {#if workspaceError}
      <AdminMessage tone="error" density="compact" title="Workspace catalog unavailable" message={workspaceError} testId="session-file-bindings-workspace-error" />
    {:else if !loading && mutationAllowed && eligibleWorkspaces.length === 0}
      <AdminMessage
        tone="info"
        density="compact"
        title="No eligible workspace"
        message="No owner or project-policy-approved file workspace is available for this session."
        testId="session-file-bindings-workspace-empty"
      />
    {:else if !loadingFiles && selectedWorkspaceId && workspaceFiles.length === 0}
      <AdminMessage
        tone="info"
        density="compact"
        title="Workspace is empty"
        message="Upload a file to the selected workspace before creating a binding."
        testId="session-file-bindings-workspace-files-empty"
      />
    {/if}
    {#if loading && bindings.length === 0}
      <AdminMessage tone="loading" density="compact" message="Loading session file bindings..." testId="session-file-bindings-loading" />
    {:else if bindingError}
      <AdminMessage tone="error" density="compact" title="Bindings unavailable" message={bindingError} testId="session-file-bindings-error" />
    {:else if rows.length === 0}
      <div class="rounded-md border border-dashed border-admin-border bg-admin-soft/50 p-5 text-center text-sm text-admin-muted" data-testid="session-file-bindings-empty">
        No workspace files are bound to this session.
      </div>
    {:else}
      <div class="grid gap-2" data-testid="session-file-bindings-list">
        {#each rows as row}
          <article class="grid min-w-0 gap-3 rounded-md border border-admin-border bg-admin-soft/50 p-3 lg:grid-cols-[minmax(0,1fr)_auto]" data-testid="session-file-binding-row">
            <div class="min-w-0">
              <strong class="block break-all text-sm font-semibold text-admin-ink">{row.fileName}</strong>
              <p class="m-0 mt-1 break-all font-mono text-xs text-admin-ink" data-testid="session-file-binding-mount">{row.mountPath}</p>
              <p class="m-0 mt-1 text-xs text-admin-muted">{row.state} | {row.mode} | {row.size} | {row.mediaType}</p>
              <p class="m-0 mt-1 break-all font-mono text-xs text-admin-muted">{row.digest}</p>
              {#if row.error}
                <p class="m-0 mt-1 text-xs text-red-700" data-testid="session-file-binding-row-error">{row.error}</p>
              {/if}
            </div>
            <div class="flex flex-wrap items-start gap-2">
              <a class="inline-flex h-9 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-white" href={`/admin-new/files/workspaces/${encodeURIComponent(row.workspaceId)}`}>
                Workspace
              </a>
              <button
                class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-white disabled:cursor-not-allowed disabled:opacity-60"
                type="button"
                onclick={() => void downloadBinding(row.id)}
                disabled={downloadingId !== null || mutating}
                data-testid="session-file-binding-download"
              >
                <Download size={15} strokeWidth={1.8} />
                <span>{downloadingId === row.id ? 'Downloading...' : 'Download'}</span>
              </button>
              <button
                class="inline-flex h-9 items-center gap-2 rounded-md border border-red-200 bg-red-50 px-3 text-sm font-medium text-red-700 hover:bg-red-100 disabled:cursor-not-allowed disabled:opacity-60"
                type="button"
                onclick={() => void removeBinding(row.id)}
                disabled={!mutationAllowed || downloadingId !== null || mutating}
                data-testid="session-file-binding-remove"
              >
                <Trash2 size={15} strokeWidth={1.8} />
                <span>Remove</span>
              </button>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </div>
</section>
