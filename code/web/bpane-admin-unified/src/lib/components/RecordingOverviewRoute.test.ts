import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import RecordingOverviewRoute from './RecordingOverviewRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('RecordingOverviewRoute', () => {
  it('loads visible session recordings and downloads a selected artifact', async () => {
    const createObjectURL = vi.fn(() => 'blob:recording');
    const revokeObjectURL = vi.fn();
    Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: createObjectURL });
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeObjectURL });
    const anchorClick = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions') && init?.method === 'GET') {
        return jsonResponse({ sessions: [sessionPayload({ id: 'session-1' })] }, 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/recordings') && init?.method === 'GET') {
        return jsonResponse({ recordings: [recordingPayload()] }, 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/recordings/recording-1/content') && init?.method === 'GET') {
        return new Response('recording bytes', { status: 200 });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(RecordingOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'recordings-list').textContent).toContain('recording-1');
    });

    byTestId(target, 'recordings-download').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'recordings-action-success').textContent).toContain('Download started');
    });
    expect(createObjectURL).toHaveBeenCalledWith(expect.any(Blob));
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:recording');
    expect(anchorClick).toHaveBeenCalled();
    const recordingCalls = fetchImpl.mock.calls.filter((call) => String(call[0]).includes('/recordings'));
    expect(recordingCalls.map((call) => [String(call[0]), call[1]?.method])).toEqual([
      ['http://localhost:3000/api/v1/sessions/session-1/recordings', 'GET'],
      ['http://localhost:3000/api/v1/sessions/session-1/recordings/recording-1/content', 'GET'],
    ]);
    for (const call of fetchImpl.mock.calls) {
      const headers = call[1]?.headers as Headers;
      expect(headers.get('authorization')).toBe('Bearer shell-token');
    }
  });

  it('shows partial catalog warnings for per-session recording list failures', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions') && init?.method === 'GET') {
        return jsonResponse({
          sessions: [
            sessionPayload({ id: 'session-1' }),
            sessionPayload({ id: 'session-2' }),
          ],
        }, 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/recordings') && init?.method === 'GET') {
        return jsonResponse({ recordings: [recordingPayload()] }, 200);
      }
      if (url.endsWith('/api/v1/sessions/session-2/recordings') && init?.method === 'GET') {
        return new Response('temporary failure', { status: 503 });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(RecordingOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'recordings-partial-warning').textContent).toContain('session-2');
    });
    expect(byTestId(target, 'recordings-list').textContent).toContain('recording-1');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(RecordingOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'recordings-error').textContent).toContain('Session catalog request failed');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: overrides.accessTokenProvider ?? (async () => 'token'),
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function recordingPayload() {
  return {
    id: 'recording-1',
    session_id: 'session-1',
    previous_recording_id: null,
    state: 'ready',
    format: 'webm',
    mime_type: 'video/webm',
    bytes: 12_345,
    duration_ms: 61_000,
    error: null,
    termination_reason: 'manual_stop',
    artifact_available: true,
    content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
    started_at: '2026-06-21T10:00:00.000Z',
    completed_at: '2026-06-21T10:01:01.000Z',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:01:01.000Z',
  };
}
