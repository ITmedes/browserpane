<script lang="ts">
  import { onMount } from 'svelte';
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
        message: error instanceof Error ? error.message : 'Unexpected session catalog error.',
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
        message: error instanceof Error ? error.message : 'Session refresh failed.',
      };
    }
  }

  async function createSession(): Promise<void> {
    actionState = { status: 'running', label: 'Creating session...' };
    try {
      const created = await client().createSession({
        labels: {
          bpane_admin_surface: 'unified',
        },
      });
      if (sessionState.status === 'ready') {
        sessionState = {
          status: 'ready',
          sessions: [created, ...sessionState.sessions.filter((session) => session.id !== created.id)],
        };
      } else {
        sessionState = { status: 'ready', sessions: [created] };
      }
      actionState = { status: 'success', message: `Session ${created.id} created.` };
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Session creation failed.',
      };
    }
  }
</script>

<SessionOverview
  state={sessionState}
  {actionState}
  onRefresh={refreshSessions}
  onCreateSession={createSession}
/>
