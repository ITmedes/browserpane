<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { WorkflowCatalogClient } from '$lib/workflows/workflow-client';
  import type {
    WorkflowActionState,
    WorkflowDetailLoadState,
  } from '$lib/workflows/workflow-detail-state';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowDefinitionInspector from './WorkflowDefinitionInspector.svelte';

  type WorkflowDefinitionDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: WorkflowDefinitionDetailRouteProps = $props();
  let workflowState = $state<WorkflowDetailLoadState>({ status: 'idle' });
  let actionState = $state<WorkflowActionState>({ status: 'idle' });

  onMount(() => {
    const workflowId = currentWorkflowId();
    if (!workflowId) {
      workflowState = {
        status: 'error',
        workflowId: 'unknown',
        message: 'Workflow id is missing from the current route.',
      };
      return;
    }
    void loadWorkflow(workflowId);
  });

  function client(): WorkflowCatalogClient {
    return new WorkflowCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadWorkflow(workflowId: string): Promise<void> {
    workflowState = { status: 'loading', workflowId };
    actionState = { status: 'idle' };
    try {
      const workflowClient = client();
      const [definition, versions] = await Promise.all([
        workflowClient.getDefinition(workflowId),
        workflowClient.listDefinitionVersions(workflowId),
      ]);
      workflowState = { status: 'ready', definition, versions: versions.versions };
    } catch (error) {
      workflowState = {
        status: 'error',
        workflowId,
        message: error instanceof Error ? error.message : 'Unexpected workflow definition detail error.',
      };
    }
  }

  async function refreshWorkflow(): Promise<void> {
    const workflowId = activeWorkflowId();
    if (!workflowId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing workflow definition...' };
    try {
      const workflowClient = client();
      const [definition, versions] = await Promise.all([
        workflowClient.getDefinition(workflowId),
        workflowClient.listDefinitionVersions(workflowId),
      ]);
      workflowState = { status: 'ready', definition, versions: versions.versions };
      actionState = { status: 'success', message: 'Workflow definition refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Workflow definition refresh failed.',
      };
    }
  }

  function currentWorkflowId(): string | null {
    const match = window.location.pathname.match(/\/workflows\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeWorkflowId(): string | null {
    if (workflowState.status === 'ready') {
      return workflowState.definition.id;
    }
    if (workflowState.status === 'loading' || workflowState.status === 'error') {
      return workflowState.workflowId;
    }
    return null;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="workflow-definition-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/workflows"
        data-testid="workflow-definition-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Workflows</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflow definition details</h1>
    </div>
  </header>

  {#if workflowState.status === 'error'}
    <div data-testid="workflow-definition-detail-error">
      <AdminMessage tone="error" title="Workflow definition detail unavailable" message={workflowState.message} />
    </div>
  {:else}
    <WorkflowDefinitionInspector
      state={workflowState}
      {actionState}
      onRefreshWorkflow={refreshWorkflow}
    />
  {/if}
</div>
