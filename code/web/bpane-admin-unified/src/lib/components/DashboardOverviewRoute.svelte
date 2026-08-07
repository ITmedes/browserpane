<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { BrowserContextCatalogClient } from '$lib/browser-contexts/browser-context-client';
  import {
    type DashboardLoadFailure,
    type DashboardOverviewLoadState,
  } from '$lib/dashboard/dashboard-overview-view-model';
  import { EgressProfileCatalogClient } from '$lib/egress-profiles/egress-profile-client';
  import { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import { RecordingCatalogClient } from '$lib/recordings/recording-client';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionResource } from '$lib/sessions/session-types';
  import { WorkflowRunCatalogClient } from '$lib/workflow-runs/workflow-run-client';
  import { WorkflowCatalogClient } from '$lib/workflows/workflow-client';
  import DashboardOverview from './DashboardOverview.svelte';

  type DashboardOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  type CatalogResult<T> =
    | { readonly ok: true; readonly value: T }
    | { readonly ok: false; readonly failure: DashboardLoadFailure };

  let { authContext }: DashboardOverviewRouteProps = $props();
  let dashboardState = $state<DashboardOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadDashboard();
  });

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  async function loadDashboard(): Promise<void> {
    dashboardState = { status: 'loading' };

    const sessionClient = new SessionCatalogClient(clientOptions());
    const projectClient = new ProjectCatalogClient(clientOptions());
    const contextClient = new BrowserContextCatalogClient(clientOptions());
    const egressClient = new EgressProfileCatalogClient(clientOptions());
    const workspaceClient = new FileWorkspaceCatalogClient(clientOptions());
    const workflowClient = new WorkflowCatalogClient(clientOptions());
    const runClient = new WorkflowRunCatalogClient(clientOptions());
    const recorderClient = new RecordingCatalogClient(clientOptions());

    const [
      sessionsResult,
      projectsResult,
      contextsResult,
      egressResult,
      workspacesResult,
      workflowsResult,
      runsResult,
    ] = await Promise.all([
      loadCatalog('Sessions', '/admin-new/sessions', async () => (await sessionClient.listSessions()).sessions),
      loadCatalog('Projects', '/admin-new/projects', async () => (await projectClient.listProjects()).projects),
      loadCatalog('Browser contexts', '/admin-new/browser-contexts', async () => (await contextClient.listBrowserContexts()).contexts),
      loadCatalog('Egress profiles', '/admin-new/egress', async () => (await egressClient.listEgressProfiles()).profiles),
      loadCatalog('File workspaces', '/admin-new/files/workspaces', async () => (await workspaceClient.listFileWorkspaces()).workspaces),
      loadCatalog('Workflows', '/admin-new/workflows', async () => (await workflowClient.listDefinitions()).workflows),
      loadCatalog('Workflow runs', '/admin-new/workflow-runs', async () => (await runClient.listRuns()).runs),
    ]);

    const topLevelResults = [
      sessionsResult,
      projectsResult,
      contextsResult,
      egressResult,
      workspacesResult,
      workflowsResult,
      runsResult,
    ];

    if (topLevelResults.every((result) => !result.ok)) {
      dashboardState = {
        status: 'error',
        message: topLevelResults
          .map((result) => result.ok ? null : `${result.failure.resource}: ${result.failure.message}`)
          .filter((message): message is string => Boolean(message))
          .join(' · '),
      };
      return;
    }

    const sessions = valueOrEmpty(sessionsResult);
    const recordingResult = sessions.length > 0
      ? await loadCatalog('Recordings', '/admin-new/recordings', async () => {
          return await recorderClient.listRecordingsForSessions(sessions);
        })
      : { ok: true, value: { entries: [], failures: [] } } as const;

    const failures = [
      ...topLevelResults
        .filter((result): result is Extract<typeof result, { readonly ok: false }> => !result.ok)
        .map((result) => result.failure),
      ...(recordingResult.ok ? recordingResult.value.failures.map((failure) => ({
        resource: `Recordings for ${shortIdentifier(failure.sessionId)}`,
        message: failure.message,
        href: '/admin-new/recordings',
      })) : [recordingResult.failure]),
    ];

    dashboardState = {
      status: 'ready',
      snapshot: {
        sessions,
        projects: valueOrEmpty(projectsResult),
        browserContexts: valueOrEmpty(contextsResult),
        egressProfiles: valueOrEmpty(egressResult),
        fileWorkspaces: valueOrEmpty(workspacesResult),
        workflows: valueOrEmpty(workflowsResult),
        workflowRuns: valueOrEmpty(runsResult),
        recordings: recordingResult.ok ? recordingResult.value.entries : [],
      },
      failures,
    };
  }

  async function loadCatalog<T>(
    resource: string,
    href: string,
    loader: () => Promise<T>,
  ): Promise<CatalogResult<T>> {
    try {
      return { ok: true, value: await loader() };
    } catch (error) {
      return {
        ok: false,
        failure: {
          resource,
          href,
          message: error instanceof Error ? error.message : `${resource} could not be loaded.`,
        },
      };
    }
  }

  function valueOrEmpty<T>(result: CatalogResult<readonly T[]>): readonly T[] {
    return result.ok ? result.value : [];
  }

  function shortIdentifier(value: string): string {
    return value.length <= 12 ? value : value.slice(0, 12);
  }
</script>

<DashboardOverview state={dashboardState} onRefresh={loadDashboard} />
