import type {
  BrowserContextProjectResource,
  BrowserContextResource,
} from './browser-context-types';

export type BrowserContextDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly contextId: string }
  | { readonly status: 'error'; readonly contextId: string; readonly message: string }
  | { readonly status: 'ready'; readonly context: BrowserContextResource };

export type BrowserContextActionState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly label: string }
  | { readonly status: 'success'; readonly message: string }
  | { readonly status: 'error'; readonly message: string };

export type BrowserContextProjectOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly projects: readonly BrowserContextProjectResource[] };
