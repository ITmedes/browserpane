import { describe, expect, it } from 'vitest';
import { resolveAdminMessageAccessibility } from './admin-message-accessibility';

describe('resolveAdminMessageAccessibility', () => {
  it('announces error and warning feedback assertively', () => {
    expect(resolveAdminMessageAccessibility('error')).toEqual({
      role: 'alert',
      ariaLive: 'assertive',
      ariaAtomic: 'true',
    });
    expect(resolveAdminMessageAccessibility('warning')).toEqual({
      role: 'alert',
      ariaLive: 'assertive',
      ariaAtomic: 'true',
    });
  });

  it('announces normal feedback politely', () => {
    expect(resolveAdminMessageAccessibility('success')).toEqual({
      role: 'status',
      ariaLive: 'polite',
      ariaAtomic: 'true',
    });
  });

  it('keeps empty states out of live regions by default', () => {
    expect(resolveAdminMessageAccessibility('empty')).toEqual({
      role: 'note',
      ariaLive: undefined,
      ariaAtomic: undefined,
    });
  });

  it('keeps explicit notes out of live regions', () => {
    expect(resolveAdminMessageAccessibility('info', 'note')).toEqual({
      role: 'note',
      ariaLive: undefined,
      ariaAtomic: undefined,
    });
  });
});
