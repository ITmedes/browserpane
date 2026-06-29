<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { BrowserContextCatalogClient } from '$lib/browser-contexts/browser-context-client';
  import type {
    BrowserContextActionState,
    BrowserContextDetailLoadState,
  } from '$lib/browser-contexts/browser-context-detail-state';
  import AdminMessage from './AdminMessage.svelte';
  import BrowserContextInspector from './BrowserContextInspector.svelte';

  type BrowserContextDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: BrowserContextDetailRouteProps = $props();
  let contextState = $state<BrowserContextDetailLoadState>({ status: 'idle' });
  let actionState = $state<BrowserContextActionState>({ status: 'idle' });

  onMount(() => {
    const contextId = currentContextId();
    if (!contextId) {
      contextState = {
        status: 'error',
        contextId: 'unknown',
        message: 'Browser context id is missing from the current route.',
      };
      return;
    }
    void loadContext(contextId);
  });

  function client(): BrowserContextCatalogClient {
    return new BrowserContextCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadContext(contextId: string): Promise<void> {
    contextState = { status: 'loading', contextId };
    actionState = { status: 'idle' };
    try {
      const context = await client().getBrowserContext(contextId);
      contextState = { status: 'ready', context };
    } catch (error) {
      contextState = {
        status: 'error',
        contextId,
        message: error instanceof Error ? error.message : 'Unexpected browser context detail error.',
      };
    }
  }

  async function refreshContext(): Promise<void> {
    const contextId = activeContextId();
    if (!contextId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing browser context...' };
    try {
      const context = await client().getBrowserContext(contextId);
      contextState = { status: 'ready', context };
      actionState = { status: 'success', message: 'Browser context refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Browser context refresh failed.',
      };
    }
  }

  async function deleteContext(): Promise<void> {
    if (contextState.status !== 'ready') {
      return;
    }
    const contextId = contextState.context.id;
    actionState = { status: 'running', label: 'Deleting browser context...' };
    try {
      const context = await client().deleteBrowserContext(contextId);
      contextState = { status: 'ready', context };
      actionState = { status: 'success', message: 'Browser context deleted.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Browser context delete failed.',
      };
    }
  }

  function currentContextId(): string | null {
    const match = window.location.pathname.match(/\/browser-contexts\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeContextId(): string | null {
    if (contextState.status === 'ready') {
      return contextState.context.id;
    }
    if (contextState.status === 'loading' || contextState.status === 'error') {
      return contextState.contextId;
    }
    return null;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="browser-context-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/browser-contexts"
        data-testid="browser-context-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Browser contexts</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Browser context details</h1>
    </div>
  </header>

  {#if contextState.status === 'error'}
    <div data-testid="browser-context-detail-error">
      <AdminMessage tone="error" title="Browser context detail unavailable" message={contextState.message} />
    </div>
  {:else}
    <BrowserContextInspector
      state={contextState}
      {actionState}
      onRefreshContext={refreshContext}
      onDeleteContext={deleteContext}
    />
  {/if}
</div>
