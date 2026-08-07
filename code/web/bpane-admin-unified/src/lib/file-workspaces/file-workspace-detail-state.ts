import type { AdminActionState } from '$lib/application/admin-async-state';
import type {
  FileWorkspaceFileResource,
  FileWorkspaceProjectResource,
  FileWorkspaceResource,
} from './file-workspace-types';

export type FileWorkspaceDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly workspaceId: string }
  | { readonly status: 'error'; readonly workspaceId: string; readonly message: string }
  | {
      readonly status: 'ready';
      readonly workspace: FileWorkspaceResource;
      readonly files: readonly FileWorkspaceFileResource[];
    };

export type FileWorkspaceActionState = AdminActionState;

export type FileWorkspaceProjectOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly projects: readonly FileWorkspaceProjectResource[] };
