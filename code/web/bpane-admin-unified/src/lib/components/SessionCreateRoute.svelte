<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionCreateOptionsLoadState } from '$lib/sessions/session-create-view-model';
  import type { CreateSessionRequest, SessionResource } from '$lib/sessions/session-types';
  import AdminMessage from './AdminMessage.svelte';
  import SessionCreateForm from './SessionCreateForm.svelte';

  type SessionCreateRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToSession?: (session: SessionResource) => void;
  };

  type SessionActionState =
    | { readonly status: 'idle' }
    | { readonly status: 'running'; readonly label: string }
    | { readonly status: 'success'; readonly message: string }
    | { readonly status: 'error'; readonly message: string };

  let {
    authContext,
    navigateToSession = defaultNavigateToSession,
  }: SessionCreateRouteProps = $props();
  let actionState = $state<SessionActionState>({ status: 'idle' });
  let optionsState = $state<SessionCreateOptionsLoadState>({ status: 'idle' });

  const busy = $derived(actionState.status === 'running');

  onMount(() => {
    void loadOptions();
  });

  function sessionClient(): SessionCatalogClient {
    return new SessionCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  function projectClient(): ProjectCatalogClient {
    return new ProjectCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadOptions(): Promise<void> {
    optionsState = { status: 'loading' };
    try {
      const client = projectClient();
      const [projectResponse, policyOptions] = await Promise.all([
        client.listProjects(),
        client.listProjectPolicyOptions(),
      ]);
      optionsState = {
        status: 'ready',
        options: {
          projects: projectResponse.projects,
          sessionTemplates: policyOptions.sessionTemplates,
          browserContexts: policyOptions.browserContexts,
          egressProfiles: policyOptions.egressProfiles,
        },
      };
    } catch (error) {
      optionsState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Session option load failed.',
      };
    }
  }

  async function createSession(request: CreateSessionRequest): Promise<void> {
    actionState = { status: 'running', label: 'Creating session...' };
    try {
      const session = await sessionClient().createSession(request);
      actionState = { status: 'success', message: 'Session created.' };
      navigateToSession(session);
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Session creation failed.',
      };
    }
  }

  function defaultNavigateToSession(session: SessionResource): void {
    window.location.assign(`/admin-new/sessions/${encodeURIComponent(session.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-create-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/sessions"
        data-testid="session-create-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New session</h1>
    </div>
  </header>

  {#if actionState.status === 'success'}
    <AdminMessage tone="success" title="Session action completed" message={actionState.message} testId="session-create-success" />
  {:else if actionState.status === 'error'}
    <AdminMessage tone="error" title="Session action failed" message={actionState.message} testId="session-create-error" />
  {:else if actionState.status === 'running'}
    <AdminMessage tone="loading" title={actionState.label} testId="session-create-running" />
  {/if}

  <SessionCreateForm
    disabled={busy}
    {optionsState}
    onSave={createSession}
  />
</div>
