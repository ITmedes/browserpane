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

export type FileWorkspaceActionState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly label: string }
  | { readonly status: 'success'; readonly message: string }
  | { readonly status: 'error'; readonly message: string };

export type FileWorkspaceProjectOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly projects: readonly FileWorkspaceProjectResource[] };
