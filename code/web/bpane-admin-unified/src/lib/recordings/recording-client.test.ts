import { describe, expect, it, vi } from 'vitest';

import { sessionResource } from '$lib/test-utils/session-fixtures';
import { playbackPayload, recordingPayload } from '$lib/test-utils/recording-fixtures';
import {
  RecordingCatalogClient,
  RecordingCatalogError,
  toSessionRecordingPlaybackResource,
  toSessionRecordingListResponse,
} from './recording-client';

describe('RecordingCatalogClient', () => {
  it('delegates authentication failures through the shared transport', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new RecordingCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listSessionRecordings('session-1')).rejects.toMatchObject({
      name: 'RecordingCatalogError',
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('loads session recordings through the authenticated control API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      jsonResponse({ recordings: [recordingPayload()] }, 200));
    const client = new RecordingCatalogClient({
      baseUrl: 'http://localhost:3000',
      accessTokenProvider: async () => 'token',
      fetchImpl,
    });

    const response = await client.listSessionRecordings('session-1');

    expect(response.recordings[0]).toMatchObject({
      id: 'recording-1',
      session_id: 'session-1',
      state: 'ready',
      artifact_available: true,
      content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
    });
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(
      new URL('http://localhost:3000/api/v1/sessions/session-1/recordings'),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token');
  });

  it('loads recordings for visible sessions best effort', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/recordings')) {
        return jsonResponse({ recordings: [recordingPayload()] }, 200);
      }
      return new Response('temporarily unavailable', { status: 503 });
    });
    const client = new RecordingCatalogClient({
      baseUrl: 'http://localhost:3000',
      accessTokenProvider: async () => 'token',
      fetchImpl,
    });

    const response = await client.listRecordingsForSessions([
      sessionResource({ id: 'session-1' }),
      sessionResource({ id: 'session-2' }),
    ]);

    expect(response.entries).toHaveLength(1);
    expect(response.entries[0]?.session.id).toBe('session-1');
    expect(response.failures).toEqual([
      {
        sessionId: 'session-2',
        message: 'Recording catalog request failed with HTTP 503: temporarily unavailable.',
      },
    ]);
  });

  it('downloads recording content and playback exports with bearer auth', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response('binary recording', { status: 200 }));
    const client = new RecordingCatalogClient({
      baseUrl: 'http://localhost:3000',
      accessTokenProvider: async () => 'token',
      fetchImpl,
    });
    const recording = toSessionRecordingListResponse({ recordings: [recordingPayload()] }).recordings[0]!;

    const [recordingBlob, playbackBlob] = await Promise.all([
      client.downloadRecordingContent(recording),
      client.downloadSessionPlaybackExport('session-1'),
    ]);

    expect(await recordingBlob.text()).toBe('binary recording');
    expect(await playbackBlob.text()).toBe('binary recording');
    expect(fetchImpl.mock.calls.map((call) => String(call[0]))).toEqual([
      'http://localhost:3000/api/v1/sessions/session-1/recordings/recording-1/content',
      'http://localhost:3000/api/v1/sessions/session-1/recording-playback/export',
    ]);
    for (const call of fetchImpl.mock.calls) {
      const headers = call[1]?.headers as Headers;
      expect(headers.get('authorization')).toBe('Bearer token');
    }
  });

  it('loads a strict session playback summary through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse(playbackPayload(), 200));
    const client = new RecordingCatalogClient({
      baseUrl: 'http://localhost:3000',
      accessTokenProvider: async () => 'token',
      fetchImpl,
    });

    const playback = await client.getSessionRecordingPlayback('session/one');

    expect(playback).toMatchObject({
      session_id: 'session/one',
      state: 'partial',
      segment_count: 3,
      included_segment_count: 2,
      failed_segment_count: 1,
    });
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(
      new URL('http://localhost:3000/api/v1/sessions/session%2Fone/recording-playback'),
    );
  });

  it('reports invalid recording payloads', () => {
    expect(() => toSessionRecordingListResponse({ recordings: [{ id: '' }] })).toThrow(RecordingCatalogError);
  });

  it('rejects unsupported playback states and negative counters', () => {
    expect(() => toSessionRecordingPlaybackResource(playbackPayload({ state: 'unknown' })))
      .toThrow(RecordingCatalogError);
    expect(() => toSessionRecordingPlaybackResource(playbackPayload({ included_segment_count: -1 })))
      .toThrow(RecordingCatalogError);
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
