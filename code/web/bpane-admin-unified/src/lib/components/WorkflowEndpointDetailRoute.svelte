<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { WorkflowEndpointCatalogClient } from '$lib/workflow-endpoints/workflow-endpoint-client';
  import type {
    UpsertWorkflowEndpointGrantRequest,
    WorkflowEndpointDetailLoadState,
  } from '$lib/workflow-endpoints/workflow-endpoint-types';
  import WorkflowEndpointDetail from './WorkflowEndpointDetail.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let state = $state<WorkflowEndpointDetailLoadState>({ status: 'idle' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  onMount(() => {
    const route = currentRoute();
    if (!route) {
      state = { status: 'error', projectId: 'unknown', endpointKey: 'unknown', message: 'Project id or endpoint key is missing from the current route.' };
      return;
    }
    void loadDetail(route.projectId, route.endpointKey);
  });

  function client(): WorkflowEndpointCatalogClient {
    return new WorkflowEndpointCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadDetail(projectId: string, endpointKey: string, preserve = false): Promise<void> {
    if (!preserve) state = { status: 'loading', projectId, endpointKey };
    try {
      const endpointClient = client();
      const [endpoint, grantResponse] = await Promise.all([
        endpointClient.getEndpoint(projectId, endpointKey),
        endpointClient.listGrants(projectId, endpointKey),
      ]);
      state = { status: 'ready', endpoint, grants: grantResponse.grants };
    } catch (error) {
      state = { status: 'error', projectId, endpointKey, message: adminErrorMessage(error, 'Workflow endpoint detail failed.') };
    }
  }

  async function runAction(label: string, action: (projectId: string, endpointKey: string) => Promise<void>, success: string): Promise<void> {
    const route = activeRoute();
    if (!route) return;
    actionState = { status: 'running', label };
    try {
      await action(route.projectId, route.endpointKey);
      await loadDetail(route.projectId, route.endpointKey, true);
      actionState = { status: 'success', message: success };
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Workflow endpoint action failed.') };
    }
  }

  async function refresh(): Promise<void> {
    const route = activeRoute();
    if (!route) return;
    actionState = { status: 'running', label: 'Refreshing endpoint and grants...' };
    await loadDetail(route.projectId, route.endpointKey, true);
    if (state.status === 'ready') actionState = { status: 'success', message: 'Endpoint and grants refreshed.' };
    else actionState = { status: 'error', message: state.message };
  }

  async function grant(request: UpsertWorkflowEndpointGrantRequest): Promise<void> {
    await runAction(
      'Saving narrow endpoint grant...',
      async (projectId, endpointKey) => { await client().upsertGrant(projectId, endpointKey, request); },
      'Service principal endpoint grant saved.',
    );
  }

  async function revoke(grantId: string): Promise<void> {
    await runAction(
      'Revoking narrow endpoint grant...',
      async (projectId, endpointKey) => { await client().revokeGrant(projectId, endpointKey, grantId); },
      'Service principal endpoint grant revoked.',
    );
  }

  function currentRoute(): { projectId: string; endpointKey: string } | null {
    const match = window.location.pathname.match(/\/workflow-endpoints\/([^/]+)\/([^/]+)\/?$/);
    return match?.[1] && match[2]
      ? { projectId: decodeURIComponent(match[1]), endpointKey: decodeURIComponent(match[2]) }
      : null;
  }

  function activeRoute(): { projectId: string; endpointKey: string } | null {
    if (state.status === 'ready') return { projectId: state.endpoint.project_id, endpointKey: state.endpoint.endpoint_key };
    if (state.status === 'loading' || state.status === 'error') return { projectId: state.projectId, endpointKey: state.endpointKey };
    return null;
  }
</script>

<WorkflowEndpointDetail
  {state}
  {actionState}
  onRefresh={refresh}
  onActivate={() => runAction('Activating workflow endpoint...', async (projectId, endpointKey) => { await client().activateEndpoint(projectId, endpointKey); }, 'Workflow endpoint activated for granted callers.')}
  onDisable={() => runAction('Disabling new invocations...', async (projectId, endpointKey) => { await client().disableEndpoint(projectId, endpointKey); }, 'Workflow endpoint disabled. Existing invocation evidence remains readable.')}
  onGrant={grant}
  onRevoke={revoke}
/>
