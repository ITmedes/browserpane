import type { ProjectPolicyOptions, ProjectResource } from './project-types';

export type ProjectDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly projectId: string }
  | { readonly status: 'error'; readonly projectId: string; readonly message: string }
  | { readonly status: 'ready'; readonly project: ProjectResource };

export type ProjectActionState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly label: string }
  | { readonly status: 'success'; readonly message: string }
  | { readonly status: 'error'; readonly message: string };

export type ProjectPolicyOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly options: ProjectPolicyOptions }
  | { readonly status: 'error'; readonly message: string };
