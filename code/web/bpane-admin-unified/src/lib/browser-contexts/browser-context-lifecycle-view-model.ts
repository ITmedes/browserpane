import type { BrowserContextResource } from './browser-context-types';

export type BrowserContextLifecycleEligibility = {
  readonly canClone: boolean;
  readonly cloneBlockedReason: string | null;
  readonly canExport: boolean;
  readonly exportBlockedReason: string | null;
  readonly activeSessionId: string | null;
  readonly storageWarning: string | null;
};

export function browserContextLifecycleEligibility(
  context: BrowserContextResource,
): BrowserContextLifecycleEligibility {
  const sharedBlockedReason = lifecycleBlockedReason(context);
  return {
    canClone: sharedBlockedReason === null,
    cloneBlockedReason: sharedBlockedReason,
    canExport: sharedBlockedReason === null,
    exportBlockedReason: sharedBlockedReason,
    activeSessionId: context.usage.active_runtime_session_id ?? null,
    storageWarning: context.usage.profile_storage_limit_exceeded
      ? 'Profile storage is over its configured limit. Export remains available for recovery; choose a larger target limit before reusing a clone or import.'
      : null,
  };
}

function lifecycleBlockedReason(context: BrowserContextResource): string | null {
  if (context.state === 'deleted') {
    return 'Deleted contexts cannot be cloned or exported.';
  }
  if (context.persistence_mode !== 'reusable') {
    return 'Only reusable contexts can be cloned or exported.';
  }
  if (context.usage.active_runtime_session_count > 0) {
    return 'Stop the active browser session before cloning or exporting this context.';
  }
  return null;
}
