<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { CredentialBindingCatalogClient } from '$lib/credential-bindings/credential-binding-client';
  import type { CredentialBindingProjectOptionsLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import type {
    CreateCredentialBindingRequest,
    CredentialBindingResource,
  } from '$lib/credential-bindings/credential-binding-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import CredentialBindingCreateForm from './CredentialBindingCreateForm.svelte';

  let {
    authContext,
    navigateToBinding = async (binding: CredentialBindingResource) =>
      goto(`/admin-new/credential-bindings/${encodeURIComponent(binding.id)}`),
  }: {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToBinding?: (binding: CredentialBindingResource) => void | Promise<void>;
  } = $props();
  let actionState = $state<AdminActionState>({ status: 'idle' });
  let projectOptionsState = $state<CredentialBindingProjectOptionsLoadState>({ status: 'idle' });
  const busy = $derived(actionState.status === 'running');
  onMount(() => {
    void loadProjects();
  });
  function client() {
    return new CredentialBindingCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }
  async function loadProjects(): Promise<void> {
    projectOptionsState = { status: 'loading' };
    try {
      projectOptionsState = {
        status: 'ready',
        projects: (await client().listProjectOptions()).projects,
      };
    } catch (error) {
      projectOptionsState = {
        status: 'error',
        message: adminErrorMessage(error, 'Project options load failed.'),
      };
    }
  }
  async function createBinding(request: CreateCredentialBindingRequest): Promise<void> {
    actionState = { status: 'running', label: 'Provisioning credential binding...' };
    try {
      const binding = await client().createCredentialBinding(request);
      actionState = {
        status: 'success',
        message: 'Credential binding created. Secret input was cleared from the active view.',
      };
      await navigateToBinding(binding);
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Credential binding creation failed.'),
      };
    }
  }
</script>

<div
  class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
  data-testid="credential-binding-create-route"
>
  <header class="border-b border-admin-border pb-4">
    <a
      class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
      href="/admin-new/credential-bindings"
      ><ArrowLeft size={16} strokeWidth={1.8} /><span>Credential bindings</span></a
    >
    <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
    <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New credential binding</h1>
  </header>
  <ActionFeedback
    state={actionState}
    successTitle="Credential action completed"
    errorTitle="Credential action failed"
    successTestId="credential-binding-create-success"
    errorTestId="credential-binding-create-error"
    runningTestId="credential-binding-create-running"
  /><CredentialBindingCreateForm disabled={busy} {projectOptionsState} onSave={createBinding} />
</div>
