import { describe, expect, it } from 'vitest';
import { ControlApiError, parseControlApiErrorBody } from './authenticated-api';

describe('authenticated API errors', () => {
  it('extracts structured gateway error metadata from JSON bodies', () => {
    const error = new ControlApiError(502, JSON.stringify({
      error: 'failed to access workflow source repository: git ls-remote failed',
      code: 'workflow_source_repository_access_failed',
      category: 'workflow_source',
      recovery_hint: 'Check repository access.',
    }));

    expect(error.message).toBe(
      'BrowserPane control API returned HTTP 502: failed to access workflow source repository: git ls-remote failed',
    );
    expect(error.apiMessage).toBe('failed to access workflow source repository: git ls-remote failed');
    expect(error.apiCode).toBe('workflow_source_repository_access_failed');
    expect(error.apiCategory).toBe('workflow_source');
    expect(error.recoveryHint).toBe('Check repository access.');
  });

  it('keeps plain text error bodies readable', () => {
    expect(parseControlApiErrorBody('denied')).toEqual({ message: 'denied' });
  });
});
