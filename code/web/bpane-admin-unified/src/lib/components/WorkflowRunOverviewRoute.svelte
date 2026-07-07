<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { WorkflowRunCatalogClient } from '$lib/workflow-runs/workflow-run-client';
  import type { WorkflowRunOverviewLoadState } from '$lib/workflow-runs/workflow-run-overview-view-model';
  import WorkflowRunOverview from './WorkflowRunOverview.svelte';

  type WorkflowRunOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: WorkflowRunOverviewRouteProps = $props();
  let workflowRunState = $state<WorkflowRunOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadWorkflowRuns();
  });

  function client(): WorkflowRunCatalogClient {
    return new WorkflowRunCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadWorkflowRuns(): Promise<void> {
    workflowRunState = { status: 'loading' };
    try {
      const response = await client().listRuns();
      workflowRunState = { status: 'ready', runs: response.runs };
    } catch (error) {
      workflowRunState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected workflow run catalog error.',
      };
    }
  }
</script>

<WorkflowRunOverview state={workflowRunState} onRefresh={loadWorkflowRuns} />
