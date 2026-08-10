import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { recordingPayload } from '$lib/test-utils/recording-fixtures';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionRecordingsRoute from './SessionRecordingsRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  await cleanupRenderedComponents();
});

describe('SessionRecordingsRoute', () => {
  it('loads one segment, updates policy, and downloads the retained WebM', async () => {
    let recordingMode = 'disabled';
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/recording-policy') && init?.method === 'PUT') {
        recordingMode = JSON.parse(String(init.body)).mode;
        return jsonResponse(sessionPayload({ state: 'stopped', totalClients: 0, recordingMode }));
      }
      if (url.endsWith('/recording-playback')) {
        return jsonResponse(playbackPayload({
          state: 'ready',
          segment_count: 1,
          included_segment_count: 1,
          failed_segment_count: 0,
        }));
      }
      if (url.endsWith('/recordings')) {
        return jsonResponse({ recordings: [recordingPayload()] });
      }
      if (url.endsWith('/recordings/recording-1/content')) {
        return new Response('webm bytes', { status: 200 });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ state: 'stopped', totalClients: 0, recordingMode }));
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return jsonResponse(projectPayload({ allow_manual_recordings: false }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:recording');
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const target = renderComponent(SessionRecordingsRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recording-playback-state').textContent).toContain('ready');
    });
    expect(byTestId(target, 'session-subarea-recordings').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-recording-policy-value').textContent).toContain('disabled');
    expect(byTestId(target, 'session-recording-download').textContent).toContain('Download WebM');
    expect(byTestId(target, 'session-recording-manual-policy-state').textContent).toContain('Blocked');
    expect(byTestId(target, 'session-recording-manual-policy-reason').textContent)
      .toContain('project blocks manual recording starts');
    expect(byTestId(target, 'session-recording-project-link').getAttribute('href'))
      .toBe('/admin-new/projects/project-1');
    expect((byTestId(target, 'session-recording-enable') as HTMLButtonElement).disabled).toBe(false);

    byTestId(target, 'session-recording-enable').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recordings-action-success').textContent).toContain('next session runtime');
    });
    expect(byTestId(target, 'session-recording-policy-value').textContent).toContain('always / webm');
    const policyCall = fetchImpl.mock.calls.find((call) => String(call[0]).endsWith('/recording-policy'));
    expect(JSON.parse(String(policyCall?.[1]?.body))).toMatchObject({ mode: 'always', format: 'webm' });

    byTestId(target, 'session-recording-download').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recordings-action-success').textContent).toContain('download started');
    });
    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(createObjectURL.mock.calls[0]?.[0]).toBeTruthy();
    expect(click).toHaveBeenCalledOnce();
    expect(fetchImpl.mock.calls.some((call) => String(call[0]).endsWith('/recording-1/content'))).toBe(true);
  });

  it('shows partial playback and downloads a ZIP for multiple retained segments', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/recording-playback/export')) {
        return new Response('zip bytes', { status: 200 });
      }
      if (url.endsWith('/recording-playback')) {
        return jsonResponse(playbackPayload());
      }
      if (url.endsWith('/recordings')) {
        return jsonResponse({
          recordings: [
            recordingPayload({ id: 'recording-1' }),
            recordingPayload({ id: 'recording-2' }),
            recordingPayload({ id: 'recording-3', state: 'failed', artifactAvailable: false, error: 'artifact unavailable' }),
          ],
        });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0, recordingMode: 'always' }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:playback');
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const target = renderComponent(SessionRecordingsRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recording-playback-summary').textContent).toContain('2/3 segments');
    });
    expect(target.querySelectorAll('[data-testid="session-recording-segment-row"]')).toHaveLength(3);
    expect(byTestId(target, 'session-recording-segment-error').textContent).toContain('artifact unavailable');
    expect(byTestId(target, 'session-recording-download').textContent).toContain('playback ZIP');

    byTestId(target, 'session-recording-download').click();
    await vi.waitFor(() => {
      expect(fetchImpl.mock.calls.some((call) => String(call[0]).endsWith('/recording-playback/export'))).toBe(true);
    });
  });

  it('keeps segment evidence available when playback summary loading fails', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/recording-playback')) {
        return new Response('playback unavailable', { status: 503 });
      }
      if (url.endsWith('/recordings')) {
        return jsonResponse({ recordings: [recordingPayload()] });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }));
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return new Response('project unavailable', { status: 503 });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionRecordingsRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recording-playback-error').textContent).toContain('HTTP 503');
    });
    expect(target.querySelectorAll('[data-testid="session-recording-segment-row"]')).toHaveLength(1);
    expect(byTestId(target, 'session-recording-download').textContent).toContain('Download WebM');
    expect(byTestId(target, 'session-recording-project-warning').textContent).toContain('HTTP 503');
    expect((byTestId(target, 'session-recording-enable') as HTMLButtonElement).disabled).toBe(false);
  });

  it('keeps playback evidence available when segment loading fails', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/recording-playback')) {
        return jsonResponse(playbackPayload());
      }
      if (url.endsWith('/recordings')) {
        return new Response('segments unavailable', { status: 503 });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionRecordingsRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recording-segments-error').textContent).toContain('HTTP 503');
    });
    expect(byTestId(target, 'session-recording-playback-summary').textContent).toContain('2/3 segments');
    expect(byTestId(target, 'session-recording-download').textContent).toContain('playback ZIP');
  });

  it('renders an empty catalog and reports rejected policy changes', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/recording-policy') && init?.method === 'PUT') {
        return new Response('session cannot be changed', { status: 409 });
      }
      if (url.endsWith('/recording-playback')) {
        return jsonResponse(playbackPayload({
          state: 'empty',
          segment_count: 0,
          included_segment_count: 0,
          failed_segment_count: 0,
          included_bytes: 0,
          included_duration_ms: 0,
        }));
      }
      if (url.endsWith('/recordings')) {
        return jsonResponse({ recordings: [] });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ state: 'stopped', totalClients: 0 }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionRecordingsRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => expect(byTestId(target, 'session-recording-segments-empty')).toBeTruthy());
    expect((byTestId(target, 'session-recording-download') as HTMLButtonElement).disabled).toBe(true);
    byTestId(target, 'session-recording-enable').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recordings-action-error').textContent).toContain('HTTP 409');
    });
    expect(byTestId(target, 'session-recording-policy-value').textContent).toContain('disabled');
  });

  it('delegates fatal session authentication failures to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionRecordingsRoute, {
      authContext: authContext({ onAuthenticationFailure }),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-recordings-error').textContent).toContain('HTTP 401');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: { configured: true, authenticated: true, username: 'demo', accessToken: 'token', claims: null },
    authConfig: null,
    accessTokenProvider: async () => 'owner-token',
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function playbackPayload(overrides: Readonly<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    session_id: 'session-1',
    state: 'partial',
    segment_count: 3,
    included_segment_count: 2,
    failed_segment_count: 1,
    active_segment_count: 0,
    missing_artifact_segment_count: 0,
    included_bytes: 24_690,
    included_duration_ms: 122_000,
    manifest_path: '/api/v1/sessions/session-1/recording-playback/manifest',
    export_path: '/api/v1/sessions/session-1/recording-playback/export',
    generated_at: '2026-08-07T10:00:00Z',
    ...overrides,
  };
}

function projectPayload(policy: Readonly<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    id: 'project-1',
    name: 'Support',
    description: 'Support browser work',
    labels: {},
    quotas: {},
    policy,
    state: 'active',
    usage: {
      project_id: 'project-1',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      active_workflow_runs: 0,
      runtime_usage_ms: 30_000,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      retained_storage_bytes: 0,
      alerts: [],
      observed_at: '2026-08-07T10:00:00Z',
    },
    created_at: '2026-08-07T09:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
