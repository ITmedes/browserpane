import { afterEach, describe, expect, it, vi } from 'vitest';

import { sessionResource, sessionStatus } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionInspector from './SessionInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('SessionInspector', () => {
  it('renders detail sections and dispatches lifecycle actions', () => {
    const onConnectPreview = vi.fn();
    const onDisconnectAll = vi.fn();
    const onEnableRecording = vi.fn();
    const target = renderComponent(SessionInspector, {
      state: {
        status: 'ready',
        session: sessionResource({ totalClients: 1, stopAllowed: false }),
        liveStatus: sessionStatus({ totalClients: 1, stopAllowed: false }),
      },
      onConnectPreview,
      onDisconnectAll,
      onEnableRecording,
    });

    expect(byTestId(target, 'session-detail-title').textContent).toContain('session-1');
    expect(byTestId(target, 'session-detail-runtime').textContent).toContain('running');
    expect(byTestId(target, 'session-detail-total-clients').textContent).toContain('1');
    expect((byTestId(target, 'session-stop') as HTMLButtonElement).disabled).toBe(true);

    byTestId(target, 'session-connect-preview').click();
    expect(onConnectPreview).toHaveBeenCalledWith('session-1');

    byTestId(target, 'session-disconnect-all').click();
    expect(onDisconnectAll).toHaveBeenCalledOnce();

    byTestId(target, 'session-enable-recording').click();
    expect(onEnableRecording).toHaveBeenCalledOnce();
    expect((byTestId(target, 'session-disable-recording') as HTMLButtonElement).disabled).toBe(true);
  });

  it('renders stopped sessions with a start-and-connect preview action', () => {
    const onConnectPreview = vi.fn();
    const target = renderComponent(SessionInspector, {
      state: {
        status: 'ready',
        session: sessionResource({ state: 'stopped', runtimeState: 'stopped', totalClients: 0, stopAllowed: false }),
        liveStatus: sessionStatus({ state: 'stopped', runtimeState: 'stopped', totalClients: 0, stopAllowed: false }),
      },
      onConnectPreview,
    });

    const connect = byTestId(target, 'session-connect-preview') as HTMLButtonElement;
    expect(connect.textContent).toContain('Start and connect');
    expect(connect.disabled).toBe(false);
    expect(connect.title).toContain('persisted browser profile');

    connect.click();
    expect(onConnectPreview).toHaveBeenCalledWith('session-1');
  });

  it('dispatches disable recording for sessions with active recording policy', () => {
    const onDisableRecording = vi.fn();
    const target = renderComponent(SessionInspector, {
      state: {
        status: 'ready',
        session: sessionResource({ recordingMode: 'always', totalClients: 0 }),
        liveStatus: sessionStatus({ totalClients: 0 }),
      },
      onDisableRecording,
    });

    expect((byTestId(target, 'session-enable-recording') as HTMLButtonElement).disabled).toBe(true);
    const disable = byTestId(target, 'session-disable-recording') as HTMLButtonElement;
    expect(disable.disabled).toBe(false);
    disable.click();
    expect(onDisableRecording).toHaveBeenCalledOnce();
  });
});
