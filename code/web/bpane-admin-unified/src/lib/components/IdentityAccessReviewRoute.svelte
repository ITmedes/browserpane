<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { IdentityCatalogClient } from '$lib/identity/identity-client';
  import type { IdentityReviewLoadState } from '$lib/identity/identity-review-view-model';
  import type { UpsertIdentityMappingRequest, UpsertServicePrincipalRequest } from '$lib/identity/identity-types';
  import IdentityAccessReviewWorkspace from './IdentityAccessReviewWorkspace.svelte';

  type IdentityAccessReviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: IdentityAccessReviewRouteProps = $props();
  let state = $state<IdentityReviewLoadState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  onMount(() => {
    void loadInitialReview();
  });

  function client(): IdentityCatalogClient {
    return new IdentityCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadInitialReview(): Promise<void> {
    state = { status: 'loading' };
    actionState = { status: 'idle' };
    try {
      state = { status: 'ready', review: await client().getAccessReview() };
    } catch (error) {
      state = { status: 'error', message: adminErrorMessage(error, 'Unexpected identity access review error.') };
    }
  }

  async function refreshReview(): Promise<void> {
    actionState = { status: 'running', label: 'Refreshing identity access review...' };
    try {
      state = { status: 'ready', review: await client().getAccessReview() };
      actionState = { status: 'success', message: 'Identity access review refreshed.' };
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Identity access review refresh failed.') };
    }
  }

  async function createServicePrincipal(request: UpsertServicePrincipalRequest): Promise<boolean> {
    return await mutate('Registering service principal...', 'Service principal registered.', async (api) => {
      await api.createServicePrincipal(request);
    });
  }

  async function updateServicePrincipal(id: string, request: UpsertServicePrincipalRequest): Promise<boolean> {
    return await mutate('Updating service principal...', 'Service principal updated.', async (api) => {
      await api.updateServicePrincipal(id, request);
    });
  }

  async function createIdentityMapping(request: UpsertIdentityMappingRequest): Promise<boolean> {
    return await mutate('Creating identity mapping...', 'Identity mapping created.', async (api) => {
      await api.createIdentityMapping(request);
    });
  }

  async function updateIdentityMapping(id: string, request: UpsertIdentityMappingRequest): Promise<boolean> {
    return await mutate('Updating identity mapping...', 'Identity mapping updated.', async (api) => {
      await api.updateIdentityMapping(id, request);
    });
  }

  async function mutate(
    runningLabel: string,
    successMessage: string,
    operation: (api: IdentityCatalogClient) => Promise<void>,
  ): Promise<boolean> {
    actionState = { status: 'running', label: runningLabel };
    try {
      const api = client();
      await operation(api);
      state = { status: 'ready', review: await api.getAccessReview() };
      actionState = { status: 'success', message: successMessage };
      return true;
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Identity operation failed.') };
      return false;
    }
  }
</script>

<IdentityAccessReviewWorkspace
  {state}
  {actionState}
  onRefresh={refreshReview}
  onCreateServicePrincipal={createServicePrincipal}
  onUpdateServicePrincipal={updateServicePrincipal}
  onCreateIdentityMapping={createIdentityMapping}
  onUpdateIdentityMapping={updateIdentityMapping}
/>
