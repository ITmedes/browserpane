<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { EgressProfileCatalogClient } from '$lib/egress-profiles/egress-profile-client';
  import type {
    EgressProfileActionState,
    EgressProfileDetailLoadState,
    EgressProfileProjectOptionsLoadState,
  } from '$lib/egress-profiles/egress-profile-detail-state';
  import type { UpsertEgressProfileRequest } from '$lib/egress-profiles/egress-profile-types';
  import AdminMessage from './AdminMessage.svelte';
  import EgressProfileInspector from './EgressProfileInspector.svelte';

  type EgressProfileDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: EgressProfileDetailRouteProps = $props();
  let profileState = $state<EgressProfileDetailLoadState>({ status: 'idle' });
  let actionState = $state<EgressProfileActionState>({ status: 'idle' });
  let projectOptionsState = $state<EgressProfileProjectOptionsLoadState>({ status: 'idle' });

  onMount(() => {
    const profileId = currentProfileId();
    if (!profileId) {
      profileState = {
        status: 'error',
        profileId: 'unknown',
        message: 'Egress profile id is missing from the current route.',
      };
      return;
    }
    void loadProfile(profileId);
    void loadProjectOptions();
  });

  function client(): EgressProfileCatalogClient {
    return new EgressProfileCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadProfile(profileId: string): Promise<void> {
    profileState = { status: 'loading', profileId };
    actionState = { status: 'idle' };
    try {
      const profile = await client().getEgressProfile(profileId);
      profileState = { status: 'ready', profile };
    } catch (error) {
      profileState = {
        status: 'error',
        profileId,
        message: adminErrorMessage(error, 'Unexpected egress profile detail error.'),
      };
    }
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

  async function refreshProfile(): Promise<void> {
    const profileId = activeProfileId();
    if (!profileId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing egress profile...' };
    try {
      const profile = await client().getEgressProfile(profileId);
      profileState = { status: 'ready', profile };
      actionState = { status: 'success', message: 'Egress profile refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Egress profile refresh failed.'),
      };
    }
  }

  async function saveProfile(request: UpsertEgressProfileRequest): Promise<void> {
    if (profileState.status !== 'ready') {
      return;
    }
    const profileId = profileState.profile.id;
    actionState = { status: 'running', label: 'Saving egress profile...' };
    try {
      const profile = await client().updateEgressProfile(profileId, request);
      profileState = { status: 'ready', profile };
      actionState = { status: 'success', message: 'Egress profile saved.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Egress profile save failed.'),
      };
    }
  }

  function currentProfileId(): string | null {
    const match = window.location.pathname.match(/\/egress\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeProfileId(): string | null {
    if (profileState.status === 'ready') {
      return profileState.profile.id;
    }
    if (profileState.status === 'loading' || profileState.status === 'error') {
      return profileState.profileId;
    }
    return null;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="egress-profile-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/egress"
        data-testid="egress-profile-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Egress profiles</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Egress profile details</h1>
    </div>
  </header>

  {#if profileState.status === 'error'}
    <div data-testid="egress-profile-detail-error">
      <AdminMessage tone="error" title="Egress profile detail unavailable" message={profileState.message} />
    </div>
  {:else}
    <EgressProfileInspector
      state={profileState}
      {actionState}
      {projectOptionsState}
      onRefreshProfile={refreshProfile}
      onSaveProfile={saveProfile}
    />
  {/if}
</div>
