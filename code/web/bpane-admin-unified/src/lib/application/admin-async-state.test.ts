import { describe, expect, it } from 'vitest';
import { adminErrorMessage, type AdminActionState, type AdminLoadState } from './admin-async-state';

describe('admin async state', () => {
  it('keeps action and load states as DOM-free discriminated unions', () => {
    const action: AdminActionState = { status: 'running', label: 'Saving...' };
    const load: AdminLoadState<{ readonly id: string }> = { status: 'ready', value: { id: 'project-1' } };

    expect(action).toEqual({ status: 'running', label: 'Saving...' });
    expect(load).toEqual({ status: 'ready', value: { id: 'project-1' } });
  });

  it('uses readable errors and a deterministic fallback for unknown failures', () => {
    expect(adminErrorMessage(new Error('Conflict.'), 'Request failed.')).toBe('Conflict.');
    expect(adminErrorMessage(new Error('   '), 'Request failed.')).toBe('Request failed.');
    expect(adminErrorMessage({ error: 'hidden' }, 'Request failed.')).toBe('Request failed.');
  });
});
