import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { toSessionRecordingResource } from '$lib/recordings/recording-client';
import type { RecordingCatalogEntry } from '$lib/recordings/recording-types';
import { sessionResource } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import RecordingCatalogTable from './RecordingCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('RecordingCatalogTable', () => {
  it('renders recording rows and filters by lens and search text', async () => {
    const target = renderComponent(RecordingCatalogTable, {
      entries: [
        entry({ recordingId: 'ready-recording', sessionId: 'session-ready', state: 'ready', artifactAvailable: true }),
        entry({ recordingId: 'failed-recording', sessionId: 'session-failed', state: 'failed', artifactAvailable: false }),
      ],
    });

    expect(byTestId(target, 'recordings-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="recordings-list-row"]')).toHaveLength(2);
    expect(byTestId(target, 'recordings-list').textContent).toContain('artifact ready');
    expect((byTestId(target, 'recordings-session-link') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/sessions/session-ready',
    );

    byTestId(target, 'recordings-lens-failed').click();
    await tick();

    expect(byTestId(target, 'recordings-list-count').textContent).toContain('1 of 2');
    expect(byTestId(target, 'recordings-list').textContent).toContain('failed');
    expect(byTestId(target, 'recordings-list').textContent).not.toContain('artifact ready');

    byTestId(target, 'recordings-lens-all').click();
    const search = byTestId(target, 'recordings-search') as HTMLInputElement;
    search.value = 'missing text';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'recordings-filter-empty').textContent).toContain('No recordings match');
  });

  it('dispatches recording and playback download actions for ready artifacts only', async () => {
    const onDownloadRecording = vi.fn();
    const onDownloadPlayback = vi.fn();
    const ready = entry({ recordingId: 'ready-recording', sessionId: 'session-ready', artifactAvailable: true });
    const missing = entry({ recordingId: 'missing-recording', sessionId: 'session-missing', artifactAvailable: false });
    const target = renderComponent(RecordingCatalogTable, {
      entries: [ready, missing],
      onDownloadRecording,
      onDownloadPlayback,
    });

    const downloadButtons = Array.from(target.querySelectorAll('[data-testid="recordings-download"]')) as HTMLButtonElement[];
    const exportButtons = Array.from(target.querySelectorAll('[data-testid="recordings-export"]')) as HTMLButtonElement[];

    expect(downloadButtons[0]?.disabled).toBe(false);
    expect(exportButtons[0]?.disabled).toBe(false);
    expect(downloadButtons[1]?.disabled).toBe(true);
    expect(exportButtons[1]?.disabled).toBe(true);

    downloadButtons[0]?.click();
    exportButtons[0]?.click();

    expect(onDownloadRecording).toHaveBeenCalledWith(ready);
    expect(onDownloadPlayback).toHaveBeenCalledWith(ready);
  });
});

function entry(options: {
  readonly recordingId: string;
  readonly sessionId: string;
  readonly state?: string;
  readonly artifactAvailable: boolean;
}): RecordingCatalogEntry {
  return {
    session: sessionResource({ id: options.sessionId, totalClients: 0 }),
    recording: toSessionRecordingResource({
      id: options.recordingId,
      session_id: options.sessionId,
      previous_recording_id: null,
      state: options.state ?? 'ready',
      format: 'webm',
      mime_type: 'video/webm',
      bytes: 4096,
      duration_ms: 20_000,
      error: options.state === 'failed' ? 'worker exit' : null,
      termination_reason: 'manual_stop',
      artifact_available: options.artifactAvailable,
      content_path: `/api/v1/sessions/${options.sessionId}/recordings/${options.recordingId}/content`,
      started_at: '2026-06-21T10:00:00.000Z',
      completed_at: options.state === 'recording' ? null : '2026-06-21T10:00:20.000Z',
      created_at: '2026-06-21T10:00:00.000Z',
      updated_at: '2026-06-21T10:00:20.000Z',
    }),
  };
}
