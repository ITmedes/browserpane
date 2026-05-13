<script lang="ts">
  import { base } from '$app/paths';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type { FileWorkspaceResource } from '../api/control-types';
  import { FileWorkspaceViewModelBuilder } from '../presentation/file-workspace-view-model';

  type AdminFileWorkspaceListRouteProps = {
    readonly controlClient: ControlClient;
  };

  let { controlClient }: AdminFileWorkspaceListRouteProps = $props();
  let workspaces = $state<readonly FileWorkspaceResource[]>([]);
  let fileCounts = $state<Readonly<Record<string, number | null>>>({});
  let loading = $state(false);
  let creating = $state(false);
  let error = $state<string | null>(null);
  let search = $state('');
  let name = $state('');
  let description = $state('');
  let labels = $state('purpose=admin-input');
  let lastRefreshedAt = $state<string | null>(null);

  const viewModel = $derived(FileWorkspaceViewModelBuilder.list({ workspaces, search }));

  onMount(() => {
    void loadWorkspaces();
  });

  async function loadWorkspaces(): Promise<void> {
    loading = true;
    error = null;
    try {
      const response = await controlClient.listFileWorkspaces();
      workspaces = response.workspaces;
      lastRefreshedAt = new Date().toISOString();
      void loadFileCounts(response.workspaces);
    } catch (loadError) {
      error = errorMessage(loadError, 'Unexpected file workspace list error');
    } finally {
      loading = false;
    }
  }

  async function loadFileCounts(nextWorkspaces: readonly FileWorkspaceResource[]): Promise<void> {
    const entries = await Promise.all(nextWorkspaces.map(async (workspace) => {
      try {
        return [workspace.id, (await controlClient.listFileWorkspaceFiles(workspace.id)).files.length] as const;
      } catch {
        return [workspace.id, null] as const;
      }
    }));
    fileCounts = Object.fromEntries(entries);
  }

  async function createWorkspace(): Promise<void> {
    const trimmedName = name.trim();
    if (!trimmedName) {
      error = 'Workspace name is required.';
      return;
    }
    creating = true;
    error = null;
    try {
      const created = await controlClient.createFileWorkspace({
        name: trimmedName,
        description: description.trim() || null,
        labels: parseLabels(labels),
      });
      workspaces = [created, ...workspaces.filter((workspace) => workspace.id !== created.id)];
      await goto(workspaceHref(created.id));
    } catch (createError) {
      error = errorMessage(createError, 'Unexpected file workspace create error');
    } finally {
      creating = false;
    }
  }

  function workspaceHref(workspaceId: string): string {
    return `${base}/files/workspaces/${encodeURIComponent(workspaceId)}`;
  }

  function formatDate(value: string | null): string {
    return value ? new Date(value).toLocaleString() : 'not refreshed';
  }

  function fileCountLabel(workspaceId: string): string {
    const count = fileCounts[workspaceId];
    if (count === null || count === undefined) {
      return 'files unavailable';
    }
    return count === 1 ? '1 file' : `${count} files`;
  }

  function parseLabels(value: string): Readonly<Record<string, string>> {
    const parsed: Record<string, string> = {};
    for (const rawPart of value.split(/[\n,]/u)) {
      const part = rawPart.trim();
      if (!part) {
        continue;
      }
      const separator = part.indexOf('=');
      if (separator <= 0) {
        throw new Error(`Label "${part}" must use key=value.`);
      }
      const key = part.slice(0, separator).trim();
      const labelValue = part.slice(separator + 1).trim();
      if (!key || !labelValue) {
        throw new Error(`Label "${part}" must use non-empty key and value.`);
      }
      parsed[key] = labelValue;
    }
    return parsed;
  }

  function errorMessage(value: unknown, fallback: string): string {
    return value instanceof Error ? value.message : fallback;
  }
</script>

<section class="grid gap-5" data-testid="file-workspace-list">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">File workspaces</p>
        <h1 class="admin-section-title">Reusable input files</h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <a class="admin-button-ghost" href={`${base}/sessions`}>Sessions</a>
        <a class="admin-button-ghost" href={`${base}/workflows`}>Workflow catalog</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="file-workspace-refresh"
          disabled={loading || creating}
          onclick={() => void loadWorkspaces()}
        >
          Refresh
        </button>
      </div>
    </div>
    <div class="mt-4 grid gap-3 md:grid-cols-[minmax(220px,360px)_1fr] md:items-end">
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Search
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="file-workspace-search"
          placeholder="Workspace name, label, description"
          bind:value={search}
        />
      </label>
      <p class="m-0 text-sm text-admin-ink/62" data-testid="file-workspace-count">
        {viewModel.rows.length} of {viewModel.totalCount} visible workspaces | Last refreshed {formatDate(lastRefreshedAt)}
      </p>
    </div>
  </div>

  <section class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">Create workspace</p>
        <h2 class="admin-section-title">Prepare reusable inputs</h2>
      </div>
    </div>
    <form
      class="mt-4 grid gap-3 lg:grid-cols-[minmax(180px,1fr)_minmax(220px,1.2fr)_minmax(180px,1fr)_auto]"
      onsubmit={(event) => {
        event.preventDefault();
        void createWorkspace();
      }}
    >
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Name
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="file-workspace-create-name"
          placeholder="Admin input files"
          bind:value={name}
        />
      </label>
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Description
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="file-workspace-create-description"
          placeholder="CSV and documents for workflow runs"
          bind:value={description}
        />
      </label>
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Labels
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="file-workspace-create-labels"
          placeholder="key=value"
          bind:value={labels}
        />
      </label>
      <button
        class="admin-button-primary self-end"
        type="submit"
        data-testid="file-workspace-create-submit"
        disabled={creating || loading}
      >
        {creating ? 'Creating...' : 'Create'}
      </button>
    </form>
  </section>

  {#if error}
    <section class="admin-panel mt-0">
      <p class="admin-error mt-0" data-testid="file-workspace-error">{error}</p>
    </section>
  {:else if loading && workspaces.length === 0}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0">Loading file workspaces...</p>
    </section>
  {:else if viewModel.rows.length === 0}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0" data-testid="file-workspace-empty">{viewModel.emptyMessage}</p>
    </section>
  {:else}
    <section class="grid gap-2" aria-label="File workspace table">
      {#each viewModel.rows as row}
        <a
          class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-panel/82 p-4 text-admin-ink no-underline transition hover:border-admin-leaf/42 hover:bg-admin-field/78 lg:grid-cols-[minmax(0,1fr)_minmax(180px,260px)]"
          href={workspaceHref(row.id)}
          data-testid="file-workspace-row"
          data-workspace-id={row.id}
        >
          <span class="grid min-w-0 gap-1">
            <strong class="truncate text-base" data-testid="file-workspace-row-title" title={row.name}>{row.name}</strong>
            <span class="line-clamp-2 text-sm text-admin-ink/66">{row.description}</span>
            <span class="truncate text-xs text-admin-ink/54">{row.labels}</span>
          </span>
          <span class="grid min-w-0 gap-1 text-xs text-admin-ink/62">
            <span><strong class="text-admin-ink/78">Files:</strong> {fileCountLabel(row.id)}</span>
            <span><strong class="text-admin-ink/78">Updated:</strong> {row.updatedAt}</span>
            <span><strong class="text-admin-ink/78">Created:</strong> {row.createdAt}</span>
          </span>
        </a>
      {/each}
    </section>
  {/if}
</section>
