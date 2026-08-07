<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { EgressProfileCatalogClient } from '$lib/egress-profiles/egress-profile-client';
  import type {
    EgressProfileActionState,
    EgressProfileProjectOptionsLoadState,
  } from '$lib/egress-profiles/egress-profile-detail-state';
  import type { EgressProfileResource, UpsertEgressProfileRequest } from '$lib/egress-profiles/egress-profile-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import EgressProfileEditForm from './EgressProfileEditForm.svelte';

  type EgressProfileCreateRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToProfile?: (profile: EgressProfileResource) => void | Promise<void>;
  };

  let {
    authContext,
    navigateToProfile = defaultNavigateToProfile,
  }: EgressProfileCreateRouteProps = $props();
  let actionState = $state<EgressProfileActionState>({ status: 'idle' });
  let projectOptionsState = $state<EgressProfileProjectOptionsLoadState>({ status: 'idle' });

  const busy = $derived(actionState.status === 'running');

  onMount(() => {
    void loadProjectOptions();
  });

  function client(): EgressProfileCatalogClient {
    return new EgressProfileCatalogClient({
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
        message: adminErrorMessage(error, 'Project options load failed.'),
      };
    }
  }

  async function createProfile(request: UpsertEgressProfileRequest): Promise<void> {
    actionState = { status: 'running', label: 'Creating egress profile...' };
    try {
      const profile = await client().createEgressProfile(request);
      actionState = { status: 'success', message: 'Egress profile created.' };
      await navigateToProfile(profile);
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Egress profile creation failed.'),
      };
    }
  }

  async function defaultNavigateToProfile(profile: EgressProfileResource): Promise<void> {
    await goto(`/admin-new/egress/${encodeURIComponent(profile.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="egress-profile-create-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/egress"
        data-testid="egress-profile-create-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Egress profiles</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New egress profile</h1>
    </div>
  </header>

  <ActionFeedback
    state={actionState}
    successTitle="Egress profile action completed"
    errorTitle="Egress profile action failed"
    successTestId="egress-profile-create-success"
    errorTestId="egress-profile-create-error"
    runningTestId="egress-profile-create-running"
  />

  <EgressProfileEditForm
    mode="create"
    disabled={busy}
    {projectOptionsState}
    onSave={createProfile}
  />
</div>
