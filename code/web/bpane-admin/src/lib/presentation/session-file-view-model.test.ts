import { describe, expect, it } from 'vitest';
import type { SessionFileResource } from '../api/control-types';
import { SessionFileViewModelBuilder } from './session-file-view-model';

const FILE: SessionFileResource = {
  id: 'file-1',
  session_id: 'session-1',
  name: 'upload.txt',
  media_type: null,
  byte_count: 1536,
  sha256_hex: '1234567890abcdef9999',
  source: 'browser_upload',
  labels: {},
  content_path: '/api/v1/sessions/session-1/files/file-1/content',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:00:00Z',
};

describe('SessionFileViewModelBuilder', () => {
  it('maps file resources to display-only card data', () => {
    expect(SessionFileViewModelBuilder.card(FILE)).toMatchObject({
      id: 'file-1',
      name: 'upload.txt',
      source: 'browser upload',
      size: '1.5 KB',
      mediaType: 'application/octet-stream',
      digest: 'sha256 1234567890abcdef...',
    });
  });
});
