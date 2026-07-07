<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { WorkflowCatalogClient } from '$lib/workflows/workflow-client';
  import type {
    WorkflowOverviewLoadState,
    WorkflowVersionMap,
  } from '$lib/workflows/workflow-overview-view-model';
  import type { WorkflowDefinitionResource } from '$lib/workflows/workflow-types';
  import {
    BROWSERPANE_TOUR_DEFINITION,
    BROWSERPANE_TOUR_VERSION,
    hiddenWorkflowDefinitions,
    includeHiddenWorkflowDefinitions,
    isBrowserPaneTourDefinition,
    visibleWorkflowDefinitions,
  } from '$lib/workflows/workflow-visibility';
  import WorkflowOverview from './WorkflowOverview.svelte';

  type WorkflowOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: WorkflowOverviewRouteProps = $props();
  let workflowState = $state<WorkflowOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadWorkflows();
  });

  function client(): WorkflowCatalogClient {
    return new WorkflowCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadWorkflows(): Promise<void> {
    workflowState = { status: 'loading' };
    try {
      const workflowClient = client();
      const response = await workflowClient.listDefinitions();
      const withTemplate = await ensureBrowserPaneTourTemplate(workflowClient, response.workflows);
      const includeHidden = includeHiddenWorkflowDefinitions();
      const definitions = includeHidden ? withTemplate : visibleWorkflowDefinitions(withTemplate);
      const hiddenCount = includeHidden ? 0 : hiddenWorkflowDefinitions(withTemplate).length;
      workflowState = {
        status: 'ready',
        definitions,
        versions: await loadLatestVersions(workflowClient, definitions),
        hiddenCount,
        includeHidden,
      };
    } catch (error) {
      workflowState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected workflow catalog error.',
      };
    }
  }

  async function ensureBrowserPaneTourTemplate(
    workflowClient: WorkflowCatalogClient,
    definitions: readonly WorkflowDefinitionResource[],
  ): Promise<readonly WorkflowDefinitionResource[]> {
    const existingTemplate = definitions.find(isBrowserPaneTourDefinition);
    if (!existingTemplate) {
      const definition = await workflowClient.createDefinition(BROWSERPANE_TOUR_DEFINITION);
      await workflowClient.createDefinitionVersion(definition.id, BROWSERPANE_TOUR_VERSION);
      return [await workflowClient.getDefinition(definition.id), ...definitions];
    }
    if (existingTemplate.latest_version) {
      return definitions;
    }
    await workflowClient.createDefinitionVersion(existingTemplate.id, BROWSERPANE_TOUR_VERSION);
    const refreshed = await workflowClient.getDefinition(existingTemplate.id);
    return [
      refreshed,
      ...definitions.filter((definition) => definition.id !== existingTemplate.id),
    ];
  }

  async function loadLatestVersions(
    workflowClient: WorkflowCatalogClient,
    definitions: readonly WorkflowDefinitionResource[],
  ): Promise<WorkflowVersionMap> {
    const entries = await Promise.all(definitions.map(async (definition) => {
      if (!definition.latest_version) {
        return [definition.id, null] as const;
      }
      try {
        const version = await workflowClient.getDefinitionVersion(definition.id, definition.latest_version);
        return [definition.id, version] as const;
      } catch {
        return [definition.id, null] as const;
      }
    }));
    return Object.fromEntries(entries);
  }
</script>

<WorkflowOverview state={workflowState} onRefresh={loadWorkflows} />
