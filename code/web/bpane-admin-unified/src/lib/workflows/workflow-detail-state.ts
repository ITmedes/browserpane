import type { AdminActionState } from '$lib/application/admin-async-state';
import type {
  WorkflowDefinitionSourceFileListResponse,
  WorkflowDefinitionSourcePreviewResource,
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

export type WorkflowActionState = AdminActionState;

export type WorkflowSourcePreviewState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly version: string }
  | {
      readonly status: 'ready';
      readonly version: string;
      readonly preview: WorkflowDefinitionSourcePreviewResource;
    }
  | { readonly status: 'unavailable'; readonly version: string; readonly message: string }
  | { readonly status: 'error'; readonly version: string; readonly message: string };

export type WorkflowSourceFileListState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly version: string }
  | {
      readonly status: 'ready';
      readonly version: string;
      readonly response: WorkflowDefinitionSourceFileListResponse;
    }
  | { readonly status: 'unavailable'; readonly version: string; readonly message: string }
  | { readonly status: 'error'; readonly version: string; readonly message: string };
