<script lang="ts">
  import { base } from '$app/paths';
  import type { ControlClient } from '../api/control-client';
  import type {
    FileWorkspaceFileResource,
    FileWorkspaceResource,
    SessionFileBindingMode,
    SessionFileBindingResource,
  } from '../api/control-types';
  import {
    FileWorkspaceViewModelBuilder,
    validateSessionMountPath,
    type SessionFileBindingViewModel,
  } from '../presentation/file-workspace-view-model';
  import AdminMessage from '../presentation/AdminMessage.svelte';

  type SessionFileBindingsSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly sessionId: string;
    readonly refreshVersion?: number;
    readonly onBindingCountChange?: (count: number) => void;
  };

  let {
    controlClient,
    sessionId,
    refreshVersion = 0,
    onBindingCountChange,
  }: SessionFileBindingsSurfaceProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let lastRefreshVersion = $state(0);
  let selectedWorkspaceId = $state('');
  let loadedWorkspaceId = $state<string | null>(null);
  let selectedFileId = $state('');
  let mountPath = $state('');
  let mode = $state<SessionFileBindingMode>('read_only');
  let workspaces = $state<readonly FileWorkspaceResource[]>([]);
  let workspaceFiles = $state<readonly FileWorkspaceFileResource[]>([]);
  let bindings = $state<readonly SessionFileBindingResource[]>([]);
  let loading = $state(false);
  let loadingFiles = $state(false);
  let mutating = $state(false);
  let downloadingBindingId = $state<string | null>(null);
  let error = $state<string | null>(null);
  let actionMessage = $state<string | null>(null);

  const bindingRows = $derived(bindings.map((binding) => FileWorkspaceViewModelBuilder.sessionBinding(binding)));
  const validation = $derived(validateSessionMountPath(
    mountPath,
    bindings
      .filter((binding) => binding.state !== 'removed')
      .map((binding) => binding.mount_path),
  ));
  const selectedWorkspace = $derived(workspaces.find((workspace) => workspace.id === selectedWorkspaceId) ?? null);
  const selectedFile = $derived(workspaceFiles.find((file) => file.id === selectedFileId) ?? null);

  $effect(() => {
    if (sessionId === loadedSessionId) {
      return;
    }
    loadedSessionId = sessionId;
    loadedWorkspaceId = null;
    bindings = [];
    workspaceFiles = [];
    selectedFileId = '';
    mountPath = '';
    loading = false;
    loadingFiles = false;
    mutating = false;
    downloadingBindingId = null;
    error = null;
    actionMessage = null;
    onBindingCountChange?.(0);
    void loadBindingsAndWorkspaces(false);
  });

  $effect(() => {
    if (refreshVersion === lastRefreshVersion) {
      return;
    }
    lastRefreshVersion = refreshVersion;
    void loadBindingsAndWorkspaces(false);
  });

  $effect(() => {
    if (!selectedWorkspaceId || selectedWorkspaceId === loadedWorkspaceId) {
      return;
    }
    loadedWorkspaceId = selectedWorkspaceId;
    void loadWorkspaceFiles(selectedWorkspaceId);
  });

  async function loadBindingsAndWorkspaces(showFeedback = true): Promise<void> {
    const requestSessionId = sessionId;
    const previousBindingCount = bindings.length;
    const previousWorkspaceCount = workspaces.length;
    loading = true;
    error = null;
    actionMessage = null;
    try {
      const [bindingResult, workspaceResult] = await Promise.allSettled([
        controlClient.listSessionFileBindings(requestSessionId),
        controlClient.listFileWorkspaces(),
      ]);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      if (bindingResult.status === 'fulfilled') {
        bindings = bindingResult.value.bindings;
        onBindingCountChange?.(bindingResult.value.bindings.length);
      } else {
        bindings = [];
        onBindingCountChange?.(0);
        error = errorMessage(bindingResult.reason, 'Session file bindings are unavailable.');
      }
      if (workspaceResult.status === 'fulfilled') {
        workspaces = workspaceResult.value.workspaces;
        if (!selectedWorkspaceId || !workspaceResult.value.workspaces.some((workspace) => workspace.id === selectedWorkspaceId)) {
          selectedWorkspaceId = workspaceResult.value.workspaces[0]?.id ?? '';
          loadedWorkspaceId = null;
        }
      } else {
        workspaces = [];
        selectedWorkspaceId = '';
        selectedFileId = '';
        error = [error, errorMessage(workspaceResult.reason, 'File workspaces are unavailable.')]
          .filter(Boolean)
          .join(' | ');
      }
      if (showFeedback && !error) {
        actionMessage = refreshMessage(
          previousBindingCount,
          bindings.length,
          previousWorkspaceCount,
          workspaces.length,
        );
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        loading = false;
      }
    }
  }

  async function loadWorkspaceFiles(workspaceId: string): Promise<void> {
    loadingFiles = true;
    error = null;
    try {
      const response = await controlClient.listFileWorkspaceFiles(workspaceId);
      if (loadedWorkspaceId !== workspaceId) {
        return;
      }
      workspaceFiles = response.files;
      selectedFileId = response.files[0]?.id ?? '';
      if (selectedFileId && !mountPath.trim()) {
        mountPath = `uploads/${response.files[0]?.name ?? 'input-file'}`;
      }
    } catch (loadError) {
      if (loadedWorkspaceId !== workspaceId) {
        return;
      }
      workspaceFiles = [];
      selectedFileId = '';
      error = errorMessage(loadError, 'Workspace files are unavailable.');
    } finally {
      if (loadedWorkspaceId === workspaceId) {
        loadingFiles = false;
      }
    }
  }

  async function createBinding(): Promise<void> {
    const requestSessionId = sessionId;
    actionMessage = null;
    if (!selectedWorkspaceId || !selectedFileId) {
      error = 'Choose a workspace file before creating a binding.';
      return;
    }
    if (!validation.valid) {
      error = validation.message;
      return;
    }
    mutating = true;
    error = null;
    try {
      const created = await controlClient.createSessionFileBinding(requestSessionId, {
        workspace_id: selectedWorkspaceId,
        file_id: selectedFileId,
        mount_path: validation.value,
        mode,
        labels: { source: 'admin' },
      });
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      bindings = [created, ...bindings.filter((binding) => binding.id !== created.id)];
      onBindingCountChange?.(bindings.length);
      actionMessage = `Bound ${created.file_name} to ${created.mount_path}.`;
      mountPath = '';
    } catch (createError) {
      if (loadedSessionId === requestSessionId) {
        error = errorMessage(createError, 'Unexpected session file binding create error');
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        mutating = false;
      }
    }
  }

  async function removeBinding(bindingId: string): Promise<void> {
    const requestSessionId = sessionId;
    mutating = true;
    error = null;
    actionMessage = null;
    try {
      const removed = await controlClient.removeSessionFileBinding(requestSessionId, bindingId);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      bindings = bindings.filter((binding) => binding.id !== bindingId);
      onBindingCountChange?.(bindings.length);
      actionMessage = `Removed binding for ${removed.file_name}.`;
    } catch (removeError) {
      if (loadedSessionId === requestSessionId) {
        error = errorMessage(removeError, 'Unexpected session file binding remove error');
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        mutating = false;
      }
    }
  }

  async function downloadBinding(bindingId: string): Promise<void> {
    const requestSessionId = sessionId;
    const binding = bindings.find((entry) => entry.id === bindingId);
    if (!binding) {
      return;
    }
    downloadingBindingId = bindingId;
    error = null;
    actionMessage = null;
    try {
      const blob = await controlClient.downloadSessionFileBindingContent(binding);
      triggerDownload(blob, binding.file_name);
      if (loadedSessionId === requestSessionId) {
        actionMessage = `Download started for ${binding.file_name}.`;
      }
    } catch (downloadError) {
      if (loadedSessionId === requestSessionId) {
        error = errorMessage(downloadError, 'Unexpected session file binding download error');
      }
    } finally {
      if (loadedSessionId === requestSessionId) {
        downloadingBindingId = null;
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

  function workspaceHref(workspaceId: string): string {
    return `${base}/files/workspaces/${encodeURIComponent(workspaceId)}`;
  }

  function errorMessage(value: unknown, fallback: string): string {
    return value instanceof Error ? value.message : fallback;
  }

  function refreshMessage(
    previousBindingCount: number,
    nextBindingCount: number,
    previousWorkspaceCount: number,
    nextWorkspaceCount: number,
  ): string {
    if (previousBindingCount !== nextBindingCount) {
      return `Binding count changed from ${previousBindingCount} to ${nextBindingCount}.`;
    }
    if (previousWorkspaceCount !== nextWorkspaceCount) {
      return `Workspace count changed from ${previousWorkspaceCount} to ${nextWorkspaceCount}.`;
    }
    return `Refreshed ${nextBindingCount} binding${nextBindingCount === 1 ? '' : 's'} and ${nextWorkspaceCount} workspace${nextWorkspaceCount === 1 ? '' : 's'}.`;
  }
</script>

<section class="admin-panel mt-0 grid gap-4" data-testid="session-file-bindings">
  <div class="admin-header">
    <div>
      <p class="admin-eyebrow">Session file bindings</p>
      <h2 class="admin-section-title">Mounted workspace inputs</h2>
    </div>
    <div class="admin-actions">
      <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-file-bindings-refresh"
        disabled={loading || mutating}
        onclick={() => void loadBindingsAndWorkspaces()}
      >
        Refresh bindings
      </button>
    </div>
  </div>

  <form
    class="grid gap-3 xl:grid-cols-[minmax(180px,1fr)_minmax(180px,1fr)_minmax(180px,1fr)_minmax(130px,160px)_auto]"
    onsubmit={(event) => {
      event.preventDefault();
      void createBinding();
    }}
  >
    <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
      Workspace
      <select
        class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="session-file-binding-workspace"
        bind:value={selectedWorkspaceId}
        disabled={loading || mutating || workspaces.length === 0}
      >
        {#each workspaces as workspace}
          <option value={workspace.id}>{workspace.name}</option>
        {/each}
      </select>
    </label>
    <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
      Workspace file
      <select
        class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="session-file-binding-file"
        bind:value={selectedFileId}
        disabled={loadingFiles || mutating || workspaceFiles.length === 0}
      >
        {#each workspaceFiles as file}
          <option value={file.id}>{file.name}</option>
        {/each}
      </select>
    </label>
    <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
      Mount path
      <input
        class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="session-file-binding-mount-path"
        placeholder="uploads/customer-sample.csv"
        bind:value={mountPath}
      />
    </label>
    <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
      Mode
      <select
        class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="session-file-binding-mode"
        bind:value={mode}
        disabled={mutating}
      >
        <option value="read_only">read only</option>
        <option value="read_write">read write</option>
        <option value="scratch_output">scratch output</option>
      </select>
    </label>
    <button
      class="admin-button-primary self-end"
      type="submit"
      data-testid="session-file-binding-create"
      disabled={loading || loadingFiles || mutating || !selectedWorkspaceId || !selectedFileId || !validation.valid}
    >
      {mutating ? 'Saving...' : 'Bind file'}
    </button>
  </form>

  <div class="grid gap-2 text-xs text-admin-ink/58 md:grid-cols-2">
    <span data-testid="session-file-binding-path-validation">{mountPath ? validation.message : 'Mount path must be a relative file path.'}</span>
    <span>
      Selected:
      {#if selectedWorkspace && selectedFile}
        <a class="text-admin-leaf" href={workspaceHref(selectedWorkspace.id)}>{selectedWorkspace.name}</a>
        / {selectedFile.name}
      {:else}
        No workspace file selected
      {/if}
    </span>
  </div>

  {#if error}
    <AdminMessage variant="error" message={error} testId="session-file-bindings-error" compact={true} />
  {/if}
  {#if actionMessage}
    <AdminMessage variant="success" message={actionMessage} testId="session-file-bindings-message" compact={true} />
  {/if}

  {#if loading && bindings.length === 0}
    <AdminMessage variant="loading" message="Loading session file bindings..." compact={true} />
  {:else if workspaces.length === 0}
    <AdminMessage
      variant="empty"
      message="No file workspaces are available. Create a workspace before binding inputs into this session."
      testId="session-file-bindings-empty"
      compact={true}
    />
  {:else if bindingRows.length === 0}
    <AdminMessage
      variant="empty"
      message="No workspace files are bound to this session yet."
      testId="session-file-bindings-empty"
      compact={true}
    />
  {:else}
    <div class="grid gap-3">
      {#each bindingRows as binding}
        {@render BindingRow(binding)}
      {/each}
    </div>
  {/if}
</section>

{#snippet BindingRow(binding: SessionFileBindingViewModel)}
  <article
    class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-field p-4 lg:grid-cols-[minmax(0,1fr)_auto]"
    data-testid="session-file-binding-row"
    data-binding-id={binding.id}
  >
    <div class="grid min-w-0 gap-1">
      <strong class="[overflow-wrap:anywhere]" data-testid="session-file-binding-file-name">{binding.fileName}</strong>
      <span class="font-mono text-xs text-admin-ink/70" data-testid="session-file-binding-mount">{binding.mountPath}</span>
      <span class="text-xs text-admin-ink/62">{binding.size} | {binding.mediaType} | {binding.mode} | {binding.state}</span>
      <span class="font-mono text-xs text-admin-ink/58">{binding.digest}</span>
      <span class="text-xs text-admin-ink/58">{binding.provenance}</span>
      {#if binding.error !== 'No materialization error.'}
        <span class="text-xs text-admin-danger">{binding.error}</span>
      {/if}
    </div>
    <div class="flex flex-wrap items-start gap-2">
      <a class="admin-button-ghost" href={workspaceHref(binding.workspaceId)}>Workspace</a>
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-file-binding-download"
        disabled={Boolean(downloadingBindingId) || mutating}
        onclick={() => void downloadBinding(binding.id)}
      >
        {downloadingBindingId === binding.id ? 'Downloading...' : 'Download'}
      </button>
      <button
        class="admin-button-ghost"
        type="button"
        data-testid="session-file-binding-remove"
        disabled={Boolean(downloadingBindingId) || mutating}
        onclick={() => void removeBinding(binding.id)}
      >
        Remove
      </button>
    </div>
  </article>
{/snippet}
