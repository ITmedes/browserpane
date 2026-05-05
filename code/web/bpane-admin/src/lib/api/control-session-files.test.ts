import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from './control-client';

const SESSION_FILE = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f7',
  session_id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  name: 'session-upload.txt',
  media_type: 'text/plain',
  byte_count: 11,
  sha256_hex: '64ec88ca00b268e5ba1a35678a1b5316d212f4f366b2477232534a8aeca37f3c',
  source: 'browser_upload',
  labels: { channel: 'file-transfer' },
  content_path: '/api/v1/sessions/019df4d2-f4f7-7b00-9e0c-79683b1c82f6/files/019df4d2-f4f7-7b00-9e0c-79683b1c82f7/content',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

describe('ControlClient session files', () => {
  it('lists session files through the owner-scoped API', async () => {
    const fetchImpl = jsonFetch({ files: [SESSION_FILE] });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932/admin/',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.listSessionFiles('session/with/slash');

    expect(response.files).toHaveLength(1);
    expect(response.files[0]).toMatchObject({
      name: 'session-upload.txt',
      source: 'browser_upload',
      labels: { channel: 'file-transfer' },
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions/session%2Fwith%2Fslash/files'),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          accept: 'application/json',
          authorization: 'Bearer owner-token',
        }),
      }),
    );
  });

  it('loads a single session file resource', async () => {
    const fetchImpl = jsonFetch({ ...SESSION_FILE, media_type: null });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const file = await client.getSessionFile(SESSION_FILE.session_id, 'file/with/slash');

    expect(file.media_type).toBeNull();
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION_FILE.session_id}/files/file%2Fwith%2Fslash`),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('downloads file content with bearer auth instead of anonymous anchors', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => {
      return new Response('hello file', {
        status: 200,
        headers: { 'content-type': 'text/plain' },
      });
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const blob = await client.downloadSessionFileContent(SESSION_FILE);

    expect(await blob.text()).toBe('hello file');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932${SESSION_FILE.content_path}`),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          accept: '*/*',
          authorization: 'Bearer owner-token',
        }),
      }),
    );
  });
});

function jsonFetch(payload: unknown): ReturnType<typeof vi.fn<FetchLike>> {
  return vi.fn<FetchLike>(async () => {
    return new Response(JSON.stringify(payload), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  });
}
