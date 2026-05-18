import type { AdminMessageVariant } from './admin-message-types';

export type AdminMessageRole = 'alert' | 'status' | 'note';

export type AdminMessageAccessibility = {
  readonly role: AdminMessageRole;
  readonly ariaLive: 'assertive' | 'polite' | undefined;
  readonly ariaAtomic: 'true' | undefined;
};

export function resolveAdminMessageAccessibility(
  variant: AdminMessageVariant,
  role?: AdminMessageRole,
): AdminMessageAccessibility {
  const resolvedRole = role ?? defaultRole(variant);
  const ariaLive = resolvedRole === 'note'
    ? undefined
    : resolvedRole === 'alert'
      ? 'assertive'
      : 'polite';
  return {
    role: resolvedRole,
    ariaLive,
    ariaAtomic: ariaLive ? 'true' : undefined,
  };
}

function defaultRole(variant: AdminMessageVariant): AdminMessageRole {
  if (variant === 'error' || variant === 'warning') {
    return 'alert';
  }
  if (variant === 'empty') {
    return 'note';
  }
  return 'status';
}
