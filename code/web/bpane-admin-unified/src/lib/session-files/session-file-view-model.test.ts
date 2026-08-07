import { describe, expect, it } from 'vitest';

import type { SessionFileBindingResource, SessionFileResource } from './session-file-types';
import {
  sessionFileBindingRow,
  sessionFileRow,
  validateSessionMountPath,
} from './session-file-view-model';

describe('session file view model', () => {
  it('formats captured files and bindings for compact evidence rows', () => {
    expect(sessionFileRow(file())).toMatchObject({
      name: 'report.pdf',
      source: 'browser download',
      mediaType: 'application/pdf',
      size: '2.0 KB',
      digest: 'sha256 1234567890abcdef...',
    });
    expect(sessionFileBindingRow(binding())).toMatchObject({
      fileName: 'report.pdf',
      mountPath: 'inputs/report.pdf',
      mode: 'read only',
      state: 'materialized',
      error: null,
    });
  });

  it.each([
    ['', 'Mount path is required.'],
    ['/absolute/file', 'Mount path must be relative.'],
    ['inputs\\file', 'Mount path must use forward slashes.'],
    ['inputs//file', 'Mount path must not contain empty path components.'],
    ['inputs/../file', 'Mount path must not contain traversal components.'],
    ['inputs/\u0007file', 'Mount path must not contain control characters.'],
  ])('rejects unsafe mount path %s', (value, message) => {
    expect(validateSessionMountPath(value)).toEqual({ valid: false, value, message });
  });

  it('normalizes valid paths and rejects duplicate mounts', () => {
    expect(validateSessionMountPath(' inputs/report.pdf ')).toEqual({
      valid: true,
      value: 'inputs/report.pdf',
      message: 'Mount path is valid.',
    });
    expect(validateSessionMountPath('inputs/report.pdf', ['inputs/report.pdf']).message)
      .toBe('Mount path is already bound for this session.');
  });
});

function file(): SessionFileResource {
  return {
    id: 'file-1',
    session_id: 'session-1',
    name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234',
    source: 'browser_download',
    labels: {},
    content_path: '/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}

function binding(): SessionFileBindingResource {
  return {
    id: 'binding-1',
    session_id: 'session-1',
    workspace_id: 'workspace-1',
    file_id: 'file-1',
    file_name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234',
    provenance: null,
    mount_path: 'inputs/report.pdf',
    mode: 'read_only',
    state: 'materialized',
    error: null,
    labels: {},
    content_path: '/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}
