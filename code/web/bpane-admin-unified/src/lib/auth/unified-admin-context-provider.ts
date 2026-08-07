import { getContext, setContext } from 'svelte';
import type { UnifiedAdminContext } from './unified-admin-context';

const UNIFIED_ADMIN_CONTEXT = Symbol('browserpane.unified-admin-context');

export function provideUnifiedAdminContext(context: UnifiedAdminContext): void {
  setContext(UNIFIED_ADMIN_CONTEXT, context);
}

export function useUnifiedAdminContext(): UnifiedAdminContext {
  const context = getContext<UnifiedAdminContext | undefined>(UNIFIED_ADMIN_CONTEXT);
  if (!context) {
    throw new Error('Unified admin route rendered outside the authenticated shell.');
  }
  return context;
}
