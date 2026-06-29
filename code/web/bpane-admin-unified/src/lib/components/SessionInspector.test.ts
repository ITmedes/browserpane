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
    const target = renderComponent(SessionInspector, {
      state: {
        status: 'ready',
        session: sessionResource({ totalClients: 1, stopAllowed: false }),
        liveStatus: sessionStatus({ totalClients: 1, stopAllowed: false }),
      },
      onConnectPreview,
      onDisconnectAll,
    });

    expect(byTestId(target, 'session-detail-title').textContent).toContain('session-1');
    expect(byTestId(target, 'session-detail-runtime').textContent).toContain('running');
    expect(byTestId(target, 'session-detail-total-clients').textContent).toContain('1');
    expect((byTestId(target, 'session-stop') as HTMLButtonElement).disabled).toBe(true);

    byTestId(target, 'session-connect-preview').click();
    expect(onConnectPreview).toHaveBeenCalledWith('session-1');

    byTestId(target, 'session-disconnect-all').click();
    expect(onDisconnectAll).toHaveBeenCalledOnce();
  });
});
