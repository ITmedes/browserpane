<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { CredentialBindingCatalogClient } from '$lib/credential-bindings/credential-binding-client';
  import type { CredentialBindingDetailLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import CredentialBindingInspector from './CredentialBindingInspector.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let bindingState = $state<CredentialBindingDetailLoadState>({ status: 'idle' });
  let actionState = $state<AdminActionState>({ status: 'idle' });
  onMount(() => {
    const bindingId = currentBindingId();
    if (!bindingId) {
      bindingState = {
        status: 'error',
        bindingId: 'unknown',
        message: 'Credential binding id is missing from the current route.',
      };
      return;
    }
    void loadBinding(bindingId);
  });
  function client() {
    return new CredentialBindingCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }
  async function loadBinding(bindingId: string): Promise<void> {
    bindingState = { status: 'loading', bindingId };
    try {
      bindingState = { status: 'ready', binding: await client().getCredentialBinding(bindingId) };
    } catch (error) {
      bindingState = {
        status: 'error',
        bindingId,
        message: adminErrorMessage(error, 'Unexpected credential binding detail error.'),
      };
    }
  }
  async function refreshBinding(): Promise<void> {
    const bindingId = activeBindingId();
    if (!bindingId) return;
    actionState = { status: 'running', label: 'Refreshing credential binding...' };
    try {
      bindingState = { status: 'ready', binding: await client().getCredentialBinding(bindingId) };
      actionState = { status: 'success', message: 'Credential binding metadata refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Credential binding refresh failed.'),
      };
    }
  }
  function currentBindingId(): string | null {
    const match = window.location.pathname.match(/\/credential-bindings\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }
  function activeBindingId(): string | null {
    if (bindingState.status === 'ready') return bindingState.binding.id;
    if (bindingState.status === 'loading' || bindingState.status === 'error')
      return bindingState.bindingId;
    return null;
  }
</script>

<div
  class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
  data-testid="credential-binding-detail-route"
>
  <header class="border-b border-admin-border pb-4">
    <a
      class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
      href="/admin-new/credential-bindings"
      ><ArrowLeft size={16} strokeWidth={1.8} /><span>Credential bindings</span></a
    >
    <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
    <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Credential binding details</h1>
  </header>
  {#if bindingState.status === 'error'}<AdminMessage
      tone="error"
      title="Credential binding detail unavailable"
      message={bindingState.message}
      testId="credential-binding-detail-error"
    />{:else}<CredentialBindingInspector
      state={bindingState}
      {actionState}
      onRefresh={refreshBinding}
    />{/if}
</div>
