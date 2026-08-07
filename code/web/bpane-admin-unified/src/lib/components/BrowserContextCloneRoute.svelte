<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import {
    adminErrorMessage,
    type AdminActionState,
  } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { BrowserContextCatalogClient } from '$lib/browser-contexts/browser-context-client';
  import {
    browserContextEditDraftFromResource,
    type BrowserContextEditDraft,
  } from '$lib/browser-contexts/browser-context-edit-view-model';
  import { browserContextLifecycleEligibility } from '$lib/browser-contexts/browser-context-lifecycle-view-model';
  import type {
    BrowserContextProjectResource,
    BrowserContextResource,
    CloneBrowserContextRequest,
    CreateBrowserContextRequest,
  } from '$lib/browser-contexts/browser-context-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import BrowserContextEditForm from './BrowserContextEditForm.svelte';

  type BrowserContextCloneRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToContext?: (context: BrowserContextResource) => void | Promise<void>;
  };

  type CloneLoadState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | {
        readonly status: 'ready';
        readonly context: BrowserContextResource;
        readonly projects: readonly BrowserContextProjectResource[];
        readonly initialDraft: BrowserContextEditDraft;
      };

  let {
    authContext,
    navigateToContext = defaultNavigateToContext,
  }: BrowserContextCloneRouteProps = $props();
  let loadState = $state<CloneLoadState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  const busy = $derived(actionState.status === 'running');
  const eligibility = $derived(loadState.status === 'ready'
    ? browserContextLifecycleEligibility(loadState.context)
    : null);

  onMount(() => {
    void loadCloneSource();
  });

  function client(): BrowserContextCatalogClient {
    return new BrowserContextCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadCloneSource(): Promise<void> {
    const contextId = currentContextId();
    if (!contextId) {
      loadState = { status: 'error', message: 'Browser context id is missing from the current route.' };
      return;
    }
    loadState = { status: 'loading' };
    try {
      const [context, projectOptions] = await Promise.all([
        client().getBrowserContext(contextId),
        client().listProjectOptions(),
      ]);
      loadState = {
        status: 'ready',
        context,
        projects: projectOptions.projects,
        initialDraft: browserContextEditDraftFromResource(context, `${context.name} copy`),
      };
    } catch (error) {
      loadState = {
        status: 'error',
        message: adminErrorMessage(error, 'Browser context clone source failed to load.'),
      };
    }
  }

  async function cloneContext(request: CreateBrowserContextRequest): Promise<void> {
    if (loadState.status !== 'ready' || !eligibility?.canClone) {
      return;
    }
    actionState = { status: 'running', label: 'Cloning browser context...' };
    const cloneRequest: CloneBrowserContextRequest = {
      project_id: request.project_id,
      name: request.name,
      description: request.description,
      labels: request.labels,
      retention_sec: request.retention_sec,
      max_profile_storage_bytes: request.max_profile_storage_bytes,
    };
    try {
      const context = await client().cloneBrowserContext(loadState.context.id, cloneRequest);
      actionState = { status: 'success', message: 'Browser context cloned.' };
      await navigateToContext(context);
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Browser context clone failed.'),
      };
    }
  }

  function currentContextId(): string | null {
    const match = window.location.pathname.match(/\/browser-contexts\/([^/]+)\/clone\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  async function defaultNavigateToContext(context: BrowserContextResource): Promise<void> {
    await goto(`/admin-new/browser-contexts/${encodeURIComponent(context.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="browser-context-clone-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href={loadState.status === 'ready'
          ? `/admin-new/browser-contexts/${encodeURIComponent(loadState.context.id)}`
          : '/admin-new/browser-contexts'}
        data-testid="browser-context-clone-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Browser context details</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Clone browser context</h1>
    </div>
  </header>

  {#if loadState.status === 'loading'}
    <AdminMessage tone="loading" title="Loading clone source" testId="browser-context-clone-loading" />
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Clone source unavailable"
      message={loadState.message}
      testId="browser-context-clone-error"
    />
  {:else if eligibility}
    {#if eligibility.cloneBlockedReason}
      <AdminMessage
        tone="warning"
        title="Browser context cannot be cloned"
        message={eligibility.cloneBlockedReason}
        testId="browser-context-clone-blocked"
      />
      {#if eligibility.activeSessionId}
        <a
          class="inline-flex w-fit text-sm font-semibold text-admin-accent hover:underline"
          href={`/admin-new/sessions/${encodeURIComponent(eligibility.activeSessionId)}`}
          data-testid="browser-context-clone-active-session"
        >Open active session</a>
      {/if}
    {:else}
      {#if eligibility.storageWarning}
        <AdminMessage
          tone="warning"
          title="Profile storage over limit"
          message={eligibility.storageWarning}
          testId="browser-context-clone-storage-warning"
        />
      {/if}
      <ActionFeedback
        state={actionState}
        successTitle="Browser context cloned"
        errorTitle="Browser context clone failed"
        successTestId="browser-context-clone-success"
        errorTestId="browser-context-clone-action-error"
        runningTestId="browser-context-clone-running"
      />
      <BrowserContextEditForm
        disabled={busy}
        initialDraft={loadState.initialDraft}
        title="Clone target settings"
        description="Create a distinct reusable context from the source metadata and persisted profile state."
        submitLabel="Clone browser context"
        persistenceLocked={true}
        requireChanges={false}
        projectOptionsState={{ status: 'ready', projects: loadState.projects }}
        onSave={cloneContext}
      />
    {/if}
  {/if}
</div>
