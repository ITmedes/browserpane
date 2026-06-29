<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { BrowserContextCatalogClient } from '$lib/browser-contexts/browser-context-client';
  import type { BrowserContextOverviewLoadState } from '$lib/browser-contexts/browser-context-overview-view-model';
  import BrowserContextOverview from './BrowserContextOverview.svelte';

  type BrowserContextOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: BrowserContextOverviewRouteProps = $props();
  let contextState = $state<BrowserContextOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadContexts();
  });

  function client(): BrowserContextCatalogClient {
    return new BrowserContextCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadContexts(): Promise<void> {
    contextState = { status: 'loading' };
    try {
      const response = await client().listBrowserContexts();
      contextState = {
        status: 'ready',
        contexts: response.contexts,
      };
    } catch (error) {
      contextState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected browser context catalog error.',
      };
    }
  }
</script>

<BrowserContextOverview state={contextState} onRefresh={loadContexts} />
