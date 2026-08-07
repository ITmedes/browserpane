<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ExtensionCatalogClient } from '$lib/extensions/extension-client';
  import type { ExtensionOverviewLoadState } from '$lib/extensions/extension-view-model';
  import ExtensionOverview from './ExtensionOverview.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let extensionState = $state<ExtensionOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadExtensions();
  });

  function client(): ExtensionCatalogClient {
    return new ExtensionCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadExtensions(): Promise<void> {
    extensionState = { status: 'loading' };
    try {
      const response = await client().listExtensions();
      extensionState = { status: 'ready', extensions: response.extensions };
    } catch (error) {
      extensionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Unexpected extension catalog error.'),
      };
    }
  }
</script>

<ExtensionOverview state={extensionState} onRefresh={loadExtensions} />
