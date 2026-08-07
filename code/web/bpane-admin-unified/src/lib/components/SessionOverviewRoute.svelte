<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import SessionOverview from '$lib/components/SessionOverview.svelte';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionActionState, SessionOverviewLoadState } from '$lib/sessions/session-overview-view-model';

  type SessionOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: SessionOverviewRouteProps = $props();
  let sessionState = $state<SessionOverviewLoadState>({ status: 'loading' });
  let actionState = $state<SessionActionState>({ status: 'idle' });

  onMount(() => {
    void loadSessions();
  });

  function client(): SessionCatalogClient {
    return new SessionCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadSessions(): Promise<void> {
    sessionState = { status: 'loading' };
    try {
      const response = await client().listSessions();
      sessionState = {
        status: 'ready',
        sessions: response.sessions,
      };
    } catch (error) {
      sessionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Unexpected session catalog error.'),
      };
    }
  }

  async function refreshSessions(): Promise<void> {
    actionState = { status: 'running', label: 'Refreshing sessions...' };
    try {
      const response = await client().listSessions();
      sessionState = {
        status: 'ready',
        sessions: response.sessions,
      };
      actionState = { status: 'success', message: 'Sessions refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Session refresh failed.'),
      };
    }
  }
</script>

<SessionOverview
  state={sessionState}
  {actionState}
  onRefresh={refreshSessions}
/>
