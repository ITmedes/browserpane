<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { EgressProfileCatalogClient } from '$lib/egress-profiles/egress-profile-client';
  import type { EgressProfileOverviewLoadState } from '$lib/egress-profiles/egress-profile-overview-view-model';
  import EgressProfileOverview from './EgressProfileOverview.svelte';

  type EgressProfileOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: EgressProfileOverviewRouteProps = $props();
  let profileState = $state<EgressProfileOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadProfiles();
  });

  function client(): EgressProfileCatalogClient {
    return new EgressProfileCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadProfiles(): Promise<void> {
    profileState = { status: 'loading' };
    try {
      const response = await client().listEgressProfiles();
      profileState = {
        status: 'ready',
        profiles: response.profiles,
      };
    } catch (error) {
      profileState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected egress profile catalog error.',
      };
    }
  }
</script>

<EgressProfileOverview state={profileState} onRefresh={loadProfiles} />
