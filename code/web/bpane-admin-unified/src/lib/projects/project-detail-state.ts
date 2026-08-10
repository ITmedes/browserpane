import type { AdminActionState } from '$lib/application/admin-async-state';
import type { SessionResource } from '$lib/sessions/session-types';
import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
import type { ProjectPolicyOptions, ProjectResource } from './project-types';

export type ProjectDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly projectId: string }
  | { readonly status: 'error'; readonly projectId: string; readonly message: string }
  | { readonly status: 'ready'; readonly project: ProjectResource };

export type ProjectActionState = AdminActionState;

export type ProjectPolicyOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly options: ProjectPolicyOptions }
  | { readonly status: 'error'; readonly message: string };

export type ProjectRelatedSessionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly sessions: readonly SessionResource[] }
  | { readonly status: 'error'; readonly message: string };

export type ProjectRelatedWorkflowRunsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly runs: readonly WorkflowRunResource[] }
  | { readonly status: 'error'; readonly message: string };
