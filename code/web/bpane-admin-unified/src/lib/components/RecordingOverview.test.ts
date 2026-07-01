import { afterEach, describe, expect, it, vi } from 'vitest';

import { toSessionRecordingResource } from '$lib/recordings/recording-client';
import type { RecordingCatalogEntry } from '$lib/recordings/recording-types';
import { sessionResource } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import RecordingOverview from './RecordingOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('RecordingOverview', () => {
  it('renders ready metrics, partial warnings, and refresh action', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(RecordingOverview, {
      state: {
        status: 'ready',
        entries: [entry()],
        failures: [{ sessionId: 'session-missing', message: 'temporary failure' }],
      },
      actionState: { status: 'idle' },
      onRefresh,
    });

    expect(byTestId(target, 'recordings-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'recordings-partial-warning').textContent).toContain('session-missing');

    byTestId(target, 'recordings-refresh').click();

    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it('renders loading, error, and empty states', async () => {
    const loadingTarget = renderComponent(RecordingOverview, {
      state: { status: 'loading' },
    });
    expect(byTestId(loadingTarget, 'recordings-loading').textContent).toContain('Loading recordings');
    await cleanupRenderedComponents();

    const errorTarget = renderComponent(RecordingOverview, {
      state: { status: 'error', message: 'catalog down' },
    });
    expect(byTestId(errorTarget, 'recordings-error').textContent).toContain('catalog down');
    await cleanupRenderedComponents();

    const emptyTarget = renderComponent(RecordingOverview, {
      state: { status: 'ready', entries: [], failures: [] },
    });
    expect(byTestId(emptyTarget, 'recordings-empty').textContent).toContain('No session recordings');
  });
});

function entry(): RecordingCatalogEntry {
  return {
    session: sessionResource({ id: 'session-1' }),
    recording: toSessionRecordingResource({
      id: 'recording-1',
      session_id: 'session-1',
      previous_recording_id: null,
      state: 'ready',
      format: 'webm',
      mime_type: 'video/webm',
      bytes: 1024,
      duration_ms: 2000,
      error: null,
      termination_reason: 'manual_stop',
      artifact_available: true,
      content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
      started_at: '2026-06-21T10:00:00.000Z',
      completed_at: '2026-06-21T10:00:02.000Z',
      created_at: '2026-06-21T10:00:00.000Z',
      updated_at: '2026-06-21T10:00:02.000Z',
    }),
  };
}
