import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { SessionCatalogClient } from '$lib/sessions/session-client';
import { WorkflowRunCatalogClient } from '$lib/workflow-runs/workflow-run-client';

import { ProjectCatalogClient } from './project-client';
import type { ProjectDetailLoadState } from './project-detail-state';
import type { ProjectResource, ProjectUsageResource } from './project-types';

export class ProjectDetailRouteSupport {
  private readonly authContext: UnifiedAdminContext;

  public constructor(authContext: UnifiedAdminContext) {
    this.authContext = authContext;
  }

  public projectClient(): ProjectCatalogClient {
    return new ProjectCatalogClient(this.clientOptions());
  }

  public sessionClient(): SessionCatalogClient {
    return new SessionCatalogClient(this.clientOptions());
  }

  public workflowRunClient(): WorkflowRunCatalogClient {
    return new WorkflowRunCatalogClient(this.clientOptions());
  }

  public currentProjectId(pathname: string): string | null {
    const match = pathname.match(/\/projects\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  public activeProjectId(state: ProjectDetailLoadState): string | null {
    if (state.status === 'ready') {
      return state.project.id;
    }
    if (state.status === 'loading' || state.status === 'error') {
      return state.projectId;
    }
    return null;
  }

  public replaceUsage(
    project: ProjectResource,
    usage: ProjectUsageResource,
  ): ProjectResource {
    return { ...project, usage };
  }

  private clientOptions(): {
    readonly baseUrl: string;
    readonly accessTokenProvider: UnifiedAdminContext['accessTokenProvider'];
    readonly onAuthenticationFailure: UnifiedAdminContext['onAuthenticationFailure'];
  } {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: this.authContext.accessTokenProvider,
      onAuthenticationFailure: this.authContext.onAuthenticationFailure,
    };
  }
}
