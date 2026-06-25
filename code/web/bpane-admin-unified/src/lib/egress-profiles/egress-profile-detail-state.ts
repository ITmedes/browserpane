import type { EgressProfileProjectResource, EgressProfileResource } from './egress-profile-types';

export type EgressProfileDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly profileId: string }
  | { readonly status: 'error'; readonly profileId: string; readonly message: string }
  | { readonly status: 'ready'; readonly profile: EgressProfileResource };

export type EgressProfileActionState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly label: string }
  | { readonly status: 'success'; readonly message: string }
  | { readonly status: 'error'; readonly message: string };

export type EgressProfileProjectOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly projects: readonly EgressProfileProjectResource[] }
  | { readonly status: 'error'; readonly message: string };
