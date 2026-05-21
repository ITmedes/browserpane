import { describe, expect, it } from 'vitest';
import {
  defaultSessionCreateFormState,
  parseSessionCreateLabels,
  sessionTemplateDefaultsSummary,
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
      templateId: '',
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
      templateId: '',
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

  it('includes the selected template id in the create-session payload', () => {
    const validation = validateSessionCreateForm({
      templateId: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      ownerMode: '',
      idleTimeoutSec: '',
      labels: '',
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
    });
    expect(validation.preview).toContain('"template_id"');
    expect(validation.preview).not.toContain('"owner_mode"');
  });

  it('keeps explicit owner-mode overrides when a template is selected', () => {
    const validation = validateSessionCreateForm({
      templateId: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      owner_mode: 'collaborative',
    });
  });

  it('summarizes selected template defaults for the UI', () => {
    expect(sessionTemplateDefaultsSummary({
      id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      name: 'Support triage',
      description: null,
      labels: {},
      defaults: {
        owner_mode: 'collaborative',
        viewport: { width: 1440, height: 900 },
        idle_timeout_sec: 1800,
        labels: { team: 'support' },
        integration_context: { ticket: 'INC-1234' },
        recording: { mode: 'manual', format: 'webm' },
      },
      version: 1,
      created_at: '2026-05-04T18:00:00Z',
      updated_at: '2026-05-04T18:00:00Z',
    })).toBe(
      'owner=collaborative | idle=1800s | viewport=1440x900 | labels=team=support | integration=ticket | recording=manual',
    );
  });
});
