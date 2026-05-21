<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type { BrowserContextResource, SessionResource } from '../api/control-types';
  import AdminMessage from '../presentation/AdminMessage.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import BrowserContextCatalogPanel from '../presentation/BrowserContextCatalogPanel.svelte';

  type AdminBrowserContextListRouteProps = {
    readonly controlClient: ControlClient;
  };

  let { controlClient }: AdminBrowserContextListRouteProps = $props();
  let contexts = $state<readonly BrowserContextResource[]>([]);
  let sessions = $state<readonly SessionResource[]>([]);
  let loading = $state(false);
  let deletingContextId = $state<string | null>(null);
  let error = $state<string | null>(null);
  let actionFeedback = $state<AdminMessageFeedback | null>(null);
  let lastRefreshedAt = $state<string | null>(null);

  onMount(() => {
    void refreshCatalog(false);
  });

  async function refreshCatalog(showFeedback = true): Promise<void> {
    loading = true;
    error = null;
    actionFeedback = null;
    try {
      const [contextResponse, sessionResponse] = await Promise.all([
        controlClient.listBrowserContexts(),
        controlClient.listSessions(),
      ]);
      contexts = contextResponse.contexts;
      sessions = sessionResponse.sessions;
      lastRefreshedAt = new Date().toISOString();
      if (showFeedback) {
        actionFeedback = {
          variant: 'success',
          title: 'Browser contexts refreshed',
          message: `${contexts.length} context${contexts.length === 1 ? '' : 's'} refreshed.`,
          testId: 'browser-context-route-message',
        };
      }
    } catch (loadError) {
      error = errorMessage(loadError);
    } finally {
      loading = false;
    }
  }

  async function deleteBrowserContext(contextId: string): Promise<void> {
    deletingContextId = contextId;
    error = null;
    actionFeedback = null;
    try {
      const deleted = await controlClient.deleteBrowserContext(contextId);
      contexts = contexts.map((context) => context.id === deleted.id ? deleted : context);
      actionFeedback = {
        variant: 'success',
        title: 'Browser context deleted',
        message: `Deleted browser context ${deleted.name}.`,
        testId: 'browser-context-route-message',
      };
    } catch (deleteError) {
      actionFeedback = {
        variant: 'error',
        title: 'Browser context delete failed',
        message: errorMessage(deleteError),
        testId: 'browser-context-route-message',
      };
    } finally {
      deletingContextId = null;
    }
  }

  function formatDate(value: string | null): string {
    return value ? new Date(value).toLocaleString() : 'not refreshed';
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected browser context catalog error';
  }
</script>

<section class="grid gap-5" data-testid="browser-context-route">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">Browser contexts</p>
        <h1 class="admin-section-title">Reusable browser profiles</h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <a class="admin-button-ghost" href={`${base}/sessions`}>Sessions</a>
        <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="browser-context-route-refresh"
          disabled={loading}
          onclick={() => void refreshCatalog()}
        >
          Refresh
        </button>
      </div>
    </div>
    <p class="m-0 mt-3 text-sm text-admin-ink/62" data-testid="browser-context-route-last-refresh">
      Last refreshed {formatDate(lastRefreshedAt)}
    </p>
  </div>

  {#if actionFeedback}
    <AdminMessage
      variant={actionFeedback.variant}
      title={actionFeedback.title}
      message={actionFeedback.message}
      testId={actionFeedback.testId}
      compact={true}
    />
  {/if}

  <section class="admin-panel mt-0">
    <BrowserContextCatalogPanel
      {contexts}
      {sessions}
      {loading}
      {error}
      {deletingContextId}
      onRefresh={() => void refreshCatalog()}
      onDeleteContext={(contextId) => void deleteBrowserContext(contextId)}
    />
  </section>
</section>
