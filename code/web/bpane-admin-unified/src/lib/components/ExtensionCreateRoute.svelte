<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ExtensionCatalogClient } from '$lib/extensions/extension-client';
  import type {
    CreateExtensionDefinitionRequest,
    ExtensionDefinitionResource,
  } from '$lib/extensions/extension-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import ExtensionCreateForm from './ExtensionCreateForm.svelte';

  let {
    authContext,
    navigateToExtension = async (extension: ExtensionDefinitionResource) =>
      goto(`/admin-new/extensions/${encodeURIComponent(extension.id)}`),
  }: {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToExtension?: (extension: ExtensionDefinitionResource) => void | Promise<void>;
  } = $props();
  let actionState = $state<AdminActionState>({ status: 'idle' });
  const busy = $derived(actionState.status === 'running');

  async function createExtension(request: CreateExtensionDefinitionRequest): Promise<void> {
    actionState = { status: 'running', label: 'Creating extension reference...' };
    try {
      const extension = await new ExtensionCatalogClient({
        baseUrl: window.location.origin,
        accessTokenProvider: authContext.accessTokenProvider,
        onAuthenticationFailure: authContext.onAuthenticationFailure,
      }).createExtension(request);
      actionState = { status: 'success', message: 'Extension reference created.' };
      await navigateToExtension(extension);
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Extension creation failed.'),
      };
    }
  }
</script>

<div
  class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
  data-testid="extension-create-route"
>
  <header class="border-b border-admin-border pb-4">
    <a
      class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
      href="/admin-new/extensions"
      data-testid="extension-create-back"
      ><ArrowLeft size={16} strokeWidth={1.8} /><span>Approved extensions</span></a
    >
    <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
    <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New approved extension</h1>
  </header>
  <ActionFeedback
    state={actionState}
    successTitle="Extension action completed"
    errorTitle="Extension action failed"
    successTestId="extension-create-success"
    errorTestId="extension-create-error"
    runningTestId="extension-create-running"
  />
  <ExtensionCreateForm disabled={busy} onSave={createExtension} />
</div>
