<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ExtensionCatalogClient } from '$lib/extensions/extension-client';
  import type { ExtensionDetailLoadState } from '$lib/extensions/extension-view-model';
  import type { CreateExtensionVersionRequest } from '$lib/extensions/extension-types';
  import AdminMessage from './AdminMessage.svelte';
  import ExtensionInspector from './ExtensionInspector.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let extensionState = $state<ExtensionDetailLoadState>({ status: 'idle' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  onMount(() => {
    const extensionId = currentExtensionId();
    if (!extensionId) {
      extensionState = { status: 'error', extensionId: 'unknown', message: 'Extension id is missing from the current route.' };
      return;
    }
    void loadExtension(extensionId);
  });

  function client(): ExtensionCatalogClient {
    return new ExtensionCatalogClient({ baseUrl: window.location.origin, accessTokenProvider: authContext.accessTokenProvider, onAuthenticationFailure: authContext.onAuthenticationFailure });
  }

  async function loadExtension(extensionId: string): Promise<void> {
    extensionState = { status: 'loading', extensionId };
    try {
      extensionState = { status: 'ready', extension: await client().getExtension(extensionId) };
    } catch (error) {
      extensionState = { status: 'error', extensionId, message: adminErrorMessage(error, 'Unexpected extension detail error.') };
    }
  }

  async function refreshExtension(): Promise<void> {
    const extensionId = activeExtensionId();
    if (!extensionId) return;
    actionState = { status: 'running', label: 'Refreshing extension...' };
    try {
      extensionState = { status: 'ready', extension: await client().getExtension(extensionId) };
      actionState = { status: 'success', message: 'Extension refreshed.' };
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Extension refresh failed.') };
    }
  }

  async function setEnabled(enabled: boolean): Promise<void> {
    const extensionId = activeExtensionId();
    if (!extensionId) return;
    actionState = { status: 'running', label: enabled ? 'Enabling extension...' : 'Disabling extension...' };
    try {
      extensionState = { status: 'ready', extension: await client().setExtensionEnabled(extensionId, enabled) };
      actionState = { status: 'success', message: enabled ? 'Extension enabled.' : 'Extension disabled.' };
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Extension state change failed.') };
    }
  }

  async function publishVersion(request: CreateExtensionVersionRequest): Promise<void> {
    const extensionId = activeExtensionId();
    if (!extensionId) return;
    actionState = { status: 'running', label: `Publishing extension version ${request.version}...` };
    try {
      const extensionClient = client();
      await extensionClient.createExtensionVersion(extensionId, request);
      extensionState = { status: 'ready', extension: await extensionClient.getExtension(extensionId) };
      actionState = { status: 'success', message: `Extension version ${request.version} published.` };
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Extension version publication failed.') };
    }
  }

  function currentExtensionId(): string | null {
    const match = window.location.pathname.match(/\/extensions\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeExtensionId(): string | null {
    if (extensionState.status === 'ready') return extensionState.extension.id;
    if (extensionState.status === 'loading' || extensionState.status === 'error') return extensionState.extensionId;
    return null;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="extension-detail-route">
  <header class="border-b border-admin-border pb-4">
    <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/extensions" data-testid="extension-detail-back"><ArrowLeft size={16} strokeWidth={1.8} /><span>Approved extensions</span></a>
    <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
    <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Extension details</h1>
  </header>
  {#if extensionState.status === 'error'}
    <AdminMessage tone="error" title="Extension detail unavailable" message={extensionState.message} testId="extension-detail-error" />
  {:else}
    <ExtensionInspector state={extensionState} {actionState} onRefresh={refreshExtension} onSetEnabled={setEnabled} onPublishVersion={publishVersion} />
  {/if}
</div>
