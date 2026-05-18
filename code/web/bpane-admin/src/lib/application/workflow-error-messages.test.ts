import { describe, expect, it } from 'vitest';
import { ControlApiError } from '../api/authenticated-api';
import { workflowOperationErrorMessage } from './workflow-error-messages';

describe('workflowOperationErrorMessage', () => {
  it('maps structured workflow source repository errors to operator guidance', () => {
    const error = new ControlApiError(502, JSON.stringify({
      error: 'failed to access workflow source repository: git ls-remote failed for /workspace',
      code: 'workflow_source_repository_access_failed',
      category: 'workflow_source',
      recovery_hint: 'Configure git safe.directory for /workspace.',
    }));

    expect(workflowOperationErrorMessage(error)).toContain(
      'Workflow source repository is not reachable from the gateway.',
    );
    expect(workflowOperationErrorMessage(error)).toContain(
      'Configure git safe.directory for /workspace.',
    );
  });

  it('falls back to regular error messages', () => {
    expect(workflowOperationErrorMessage(new Error('plain failure'))).toBe('plain failure');
  });
});
