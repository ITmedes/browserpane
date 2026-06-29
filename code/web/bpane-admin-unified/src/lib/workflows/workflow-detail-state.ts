import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
} from './workflow-types';

export type WorkflowDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly workflowId: string }
  | { readonly status: 'error'; readonly workflowId: string; readonly message: string }
  | {
      readonly status: 'ready';
      readonly definition: WorkflowDefinitionResource;
      readonly versions: readonly WorkflowDefinitionVersionResource[];
    };

export type WorkflowActionState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly label: string }
  | { readonly status: 'success'; readonly message: string }
  | { readonly status: 'error'; readonly message: string };
