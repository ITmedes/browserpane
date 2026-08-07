import { afterEach, describe, expect, it, vi } from 'vitest';

import { SessionFileClient } from '$lib/session-files/session-file-client';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionTransferFilesPanel from './SessionTransferFilesPanel.svelte';

afterEach(async () => {
  vi.restoreAllMocks();
  await cleanupRenderedComponents();
});

describe('SessionTransferFilesPanel', () => {
  it('loads retained files, shows policy state, refreshes, and downloads content', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/files')) {
        return jsonResponse({ files: [filePayload()] });
      }
      if (url.endsWith('/content')) {
        return new Response('exact file bytes', { status: 200 });
      }
      return new Response('not found', { status: 404 });
    });
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:session-file');
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const target = renderComponent(SessionTransferFilesPanel, {
      client: client(fetchImpl),
      sessionId: 'session-1',
      transferBlocked: true,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-file-name').textContent).toContain('report.pdf');
    });
    expect(byTestId(target, 'session-files-policy-blocked').textContent).toContain('disables');

    byTestId(target, 'session-file-download').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-files-action-success').textContent).toContain('Download started');
    });
    expect(createObjectURL).toHaveBeenCalledWith(expect.any(Blob));
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:session-file');
    expect(click).toHaveBeenCalledOnce();

    byTestId(target, 'session-files-refresh').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-files-action-success').textContent).toContain('Refreshed 1');
    });
  });

  it('renders independent empty and error states', async () => {
    const empty = renderComponent(SessionTransferFilesPanel, {
      client: client(async () => jsonResponse({ files: [] })),
      sessionId: 'session-empty',
      transferBlocked: false,
    });
    await vi.waitFor(() => expect(byTestId(empty, 'session-files-empty')).toBeTruthy());

    const failed = renderComponent(SessionTransferFilesPanel, {
      client: client(async () => new Response('failed', { status: 503 })),
      sessionId: 'session-failed',
      transferBlocked: false,
    });
    await vi.waitFor(() => {
      expect(byTestId(failed, 'session-files-error').textContent).toContain('HTTP 503');
    });
  });
});

function client(fetchImpl: typeof fetch): SessionFileClient {
  return new SessionFileClient({
    baseUrl: 'http://localhost:8080',
    accessTokenProvider: async () => 'token',
    fetchImpl,
  });
}

function filePayload() {
  return {
    id: 'file-1',
    session_id: 'session-1',
    name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234',
    source: 'browser_download',
    labels: {},
    content_path: '/api/v1/sessions/session-1/files/file-1/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
