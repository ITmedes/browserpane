import { describe, expect, it } from 'vitest';
import {
  defaultSessionCreateFormState,
  parseSessionCreateLabels,
  validateSessionCreateForm,
} from './session-create-configurator';

describe('session create configurator', () => {
  it('builds the backend-default collaborative command', () => {
    const validation = validateSessionCreateForm(defaultSessionCreateFormState());

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({ owner_mode: 'collaborative' });
    expect(validation.preview).toBe(JSON.stringify({ owner_mode: 'collaborative' }, null, 2));
  });

  it('normalizes idle timeout and labels into a create-session payload', () => {
    const validation = validateSessionCreateForm({
      ownerMode: 'exclusive_browser_owner',
      idleTimeoutSec: '1800',
      labels: 'case=1234\npurpose=import-repro, suite=admin',
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      owner_mode: 'exclusive_browser_owner',
      idle_timeout_sec: 1800,
      labels: {
        case: '1234',
        purpose: 'import-repro',
        suite: 'admin',
      },
    });
  });

  it('rejects unsupported owner modes and invalid idle timeout values', () => {
    const validation = validateSessionCreateForm({
      ownerMode: 'shared',
      idleTimeoutSec: '0',
      labels: '',
    });

    expect(validation.command).toBeNull();
    expect(validation.errors).toEqual([
      'Owner mode "shared" is not supported.',
      'Idle timeout must be a positive whole number of seconds.',
    ]);
    expect(validation.preview).toBe('Fix validation errors to preview the API payload.');
  });

  it('rejects malformed and duplicate labels', () => {
    const errors: string[] = [];

    const labels = parseSessionCreateLabels(
      'case=1234\nmalformed\npurpose=\ncase=5678',
      errors,
    );

    expect(labels).toEqual({ case: '1234' });
    expect(errors).toEqual([
      'Label "malformed" must use key=value.',
      'Label "purpose=" must use non-empty key and value.',
      'Label "case" is duplicated.',
    ]);
  });
});
