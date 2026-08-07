<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { CredentialBindingCatalogClient } from '$lib/credential-bindings/credential-binding-client';
  import type { CredentialBindingOverviewLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import CredentialBindingOverview from './CredentialBindingOverview.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let bindingState = $state<CredentialBindingOverviewLoadState>({ status: 'loading' });
  onMount(() => { void loadBindings(); });
  function client() { return new CredentialBindingCatalogClient({ baseUrl: window.location.origin, accessTokenProvider: authContext.accessTokenProvider, onAuthenticationFailure: authContext.onAuthenticationFailure }); }
  async function loadBindings(): Promise<void> {
    bindingState = { status: 'loading' };
    try { bindingState = { status: 'ready', bindings: (await client().listCredentialBindings()).credential_bindings }; }
    catch (error) { bindingState = { status: 'error', message: adminErrorMessage(error, 'Unexpected credential binding catalog error.') }; }
  }
</script>
<CredentialBindingOverview state={bindingState} onRefresh={loadBindings} />
