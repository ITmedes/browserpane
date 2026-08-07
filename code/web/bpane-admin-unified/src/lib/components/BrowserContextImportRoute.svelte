<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft, FileArchive, Upload } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { BrowserContextCatalogClient } from '$lib/browser-contexts/browser-context-client';
  import type { BrowserContextProjectOptionsLoadState } from '$lib/browser-contexts/browser-context-detail-state';
  import type {
    BrowserContextResource,
    CreateBrowserContextRequest,
    ImportBrowserContextRequest,
  } from '$lib/browser-contexts/browser-context-types';
  import { formatBytes } from '$lib/projects/project-formatters';
  import ActionFeedback from './ActionFeedback.svelte';
  import BrowserContextEditForm from './BrowserContextEditForm.svelte';

  type BrowserContextImportRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToContext?: (context: BrowserContextResource) => void | Promise<void>;
  };

  let {
    authContext,
    navigateToContext = defaultNavigateToContext,
  }: BrowserContextImportRouteProps = $props();
  let archive = $state<File | null>(null);
  let actionState = $state<AdminActionState>({ status: 'idle' });
  let projectOptionsState = $state<BrowserContextProjectOptionsLoadState>({ status: 'idle' });

  const busy = $derived(actionState.status === 'running');
  const archiveSize = $derived(archive ? formatBytes(archive.size) ?? `${archive.size} B` : null);

  onMount(() => {
    void loadProjectOptions();
  });

  function client(): BrowserContextCatalogClient {
    return new BrowserContextCatalogClient({
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
        message: adminErrorMessage(error, 'Project options load failed.'),
      };
    }
  }

  function selectArchive(event: Event): void {
    archive = (event.currentTarget as HTMLInputElement).files?.[0] ?? null;
    actionState = { status: 'idle' };
  }

  async function importContext(request: CreateBrowserContextRequest): Promise<void> {
    if (!archive) {
      return;
    }
    actionState = { status: 'running', label: 'Importing browser context archive...' };
    const importRequest: ImportBrowserContextRequest = {
      project_id: request.project_id,
      name: request.name,
      description: request.description,
      labels: request.labels,
      retention_sec: request.retention_sec,
      max_profile_storage_bytes: request.max_profile_storage_bytes,
      archive,
    };
    try {
      const context = await client().importBrowserContext(importRequest);
      actionState = { status: 'success', message: 'Browser context imported.' };
      await navigateToContext(context);
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Browser context import failed.'),
      };
    }
  }

  async function defaultNavigateToContext(context: BrowserContextResource): Promise<void> {
    await goto(`/admin-new/browser-contexts/${encodeURIComponent(context.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="browser-context-import-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/browser-contexts"
        data-testid="browser-context-import-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Browser contexts</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Import browser context</h1>
    </div>
  </header>

  <ActionFeedback
    state={actionState}
    successTitle="Browser context imported"
    errorTitle="Browser context import failed"
    successTestId="browser-context-import-success"
    errorTestId="browser-context-import-error"
    runningTestId="browser-context-import-running"
  />

  <section class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5" data-testid="browser-context-import-archive">
    <div class="flex flex-col gap-3 border-b border-admin-border pb-4 sm:flex-row sm:items-start sm:justify-between">
      <div class="min-w-0">
        <h2 class="m-0 text-base font-semibold text-admin-ink">BrowserPane export archive</h2>
        <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
          Choose an archive produced by BrowserPane. The control plane validates its manifest and profile contents during import.
        </p>
      </div>
      <FileArchive class="shrink-0 text-admin-muted" size={22} strokeWidth={1.7} />
    </div>

    <div class="mt-4 flex flex-col gap-3 sm:flex-row sm:items-center">
      <label class="inline-flex h-10 w-fit cursor-pointer items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft focus-within:ring-2 focus-within:ring-admin-accent/25">
        <Upload size={16} strokeWidth={1.9} />
        <span>Choose archive</span>
        <input
          class="sr-only"
          type="file"
          accept=".zip,application/zip"
          disabled={busy}
          onchange={selectArchive}
          data-testid="browser-context-import-file"
        />
      </label>
      {#if archive}
        <div class="min-w-0" data-testid="browser-context-import-file-summary">
          <p class="m-0 truncate text-sm font-medium text-admin-ink">{archive.name}</p>
          <p class="m-0 mt-0.5 text-xs text-admin-muted">{archiveSize}</p>
        </div>
      {:else}
        <p class="m-0 text-sm text-admin-muted" data-testid="browser-context-import-file-empty">
          No archive selected.
        </p>
      {/if}
    </div>
  </section>

  <BrowserContextEditForm
    disabled={busy}
    title="Imported context settings"
    description="Define the owner or project scope and the limits applied to the restored reusable profile."
    submitLabel="Import browser context"
    persistenceLocked={true}
    submitBlocked={!archive}
    submitBlockedHint={archive ? null : 'Choose a BrowserPane export archive before importing.'}
    {projectOptionsState}
    onSave={importContext}
  />
</div>
