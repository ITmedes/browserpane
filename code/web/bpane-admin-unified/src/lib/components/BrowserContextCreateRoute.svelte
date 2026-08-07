<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { BrowserContextCatalogClient } from '$lib/browser-contexts/browser-context-client';
  import type {
    BrowserContextActionState,
    BrowserContextProjectOptionsLoadState,
  } from '$lib/browser-contexts/browser-context-detail-state';
  import type { BrowserContextResource, CreateBrowserContextRequest } from '$lib/browser-contexts/browser-context-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import BrowserContextEditForm from './BrowserContextEditForm.svelte';

  type BrowserContextCreateRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToContext?: (context: BrowserContextResource) => void;
  };

  let {
    authContext,
    navigateToContext = defaultNavigateToContext,
  }: BrowserContextCreateRouteProps = $props();
  let actionState = $state<BrowserContextActionState>({ status: 'idle' });
  let projectOptionsState = $state<BrowserContextProjectOptionsLoadState>({ status: 'idle' });

  const busy = $derived(actionState.status === 'running');

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
        message: error instanceof Error ? error.message : 'Project options load failed.',
      };
    }
  }

  async function createContext(request: CreateBrowserContextRequest): Promise<void> {
    actionState = { status: 'running', label: 'Creating browser context...' };
    try {
      const context = await client().createBrowserContext(request);
      actionState = { status: 'success', message: 'Browser context created.' };
      navigateToContext(context);
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Browser context creation failed.',
      };
    }
  }

  function defaultNavigateToContext(context: BrowserContextResource): void {
    window.location.assign(`/admin-new/browser-contexts/${encodeURIComponent(context.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="browser-context-create-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/browser-contexts"
        data-testid="browser-context-create-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Browser contexts</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New browser context</h1>
    </div>
  </header>

  <ActionFeedback
    state={actionState}
    successTitle="Browser context action completed"
    errorTitle="Browser context action failed"
    successTestId="browser-context-create-success"
    errorTestId="browser-context-create-error"
    runningTestId="browser-context-create-running"
  />

  <BrowserContextEditForm
    disabled={busy}
    {projectOptionsState}
    onSave={createContext}
  />
</div>
