import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BrowserContextResource } from '$lib/browser-contexts/browser-context-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextInspector from './BrowserContextInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('BrowserContextInspector', () => {
  it('renders loading, error, action, and ready details', async () => {
    let target = renderComponent(BrowserContextInspector, {
      state: { status: 'loading', contextId: 'context-1' },
    });
    expect(byTestId(target, 'browser-context-inspector-loading').textContent).toContain('Loading browser context');

    await cleanupRenderedComponents();
    target = renderComponent(BrowserContextInspector, {
      state: { status: 'error', contextId: 'context-1', message: 'not found' },
    });
    expect(byTestId(target, 'browser-context-inspector-error').textContent).toContain('Browser context unavailable');

    await cleanupRenderedComponents();
    target = renderComponent(BrowserContextInspector, {
      state: { status: 'ready', context: context() },
      actionState: { status: 'success', message: 'Browser context refreshed.' },
    });
    expect(byTestId(target, 'browser-context-detail-name').textContent).toContain('Support baseline');
    expect(byTestId(target, 'browser-context-detail-state').textContent).toContain('ready');
    expect(byTestId(target, 'browser-context-detail-retention').textContent).toContain('7d');
    expect(byTestId(target, 'browser-context-detail-storage-limit').textContent).toContain('1.0 MB / 1.0 GB');
    expect(byTestId(target, 'browser-context-action-success').textContent).toContain('Browser context refreshed');
  });

  it('blocks delete while a context has active references', () => {
    const onDeleteContext = vi.fn();
    const target = renderComponent(BrowserContextInspector, {
      state: { status: 'ready', context: context({ visibleSessions: 1 }) },
      onDeleteContext,
    });

    const deleteButton = byTestId(target, 'browser-context-delete') as HTMLButtonElement;
    expect(deleteButton.disabled).toBe(true);
    expect(deleteButton.title).toContain('Visible session references');
    deleteButton.click();
    expect(onDeleteContext).not.toHaveBeenCalled();
  });

  it('blocks clone and export for an active writer and links to the active session', () => {
    const onExportContext = vi.fn();
    const target = renderComponent(BrowserContextInspector, {
      state: {
        status: 'ready',
        context: context({ activeRuntimeSessions: 1, activeRuntimeSessionId: 'session-1' }),
      },
      onExportContext,
    });

    const cloneButton = byTestId(target, 'browser-context-clone') as HTMLButtonElement;
    const exportButton = byTestId(target, 'browser-context-export') as HTMLButtonElement;
    expect(cloneButton.disabled).toBe(true);
    expect(cloneButton.title).toContain('Stop the active browser session');
    expect(exportButton.disabled).toBe(true);
    expect(exportButton.title).toContain('Stop the active browser session');
    expect(byTestId(target, 'browser-context-lifecycle-blocked').textContent).toContain(
      'Stop the active browser session',
    );
    expect(byTestId(target, 'browser-context-active-session').getAttribute('href')).toBe(
      '/admin-new/sessions/session-1',
    );
    exportButton.click();
    expect(onExportContext).not.toHaveBeenCalled();
  });

  it('allows clone and export with inactive references and warns without blocking over-limit recovery', () => {
    const onExportContext = vi.fn();
    const target = renderComponent(BrowserContextInspector, {
      state: {
        status: 'ready',
        context: context({ visibleSessions: 2, profileStorageLimitExceeded: true }),
      },
      onExportContext,
    });

    expect(byTestId(target, 'browser-context-clone').getAttribute('href')).toBe(
      '/admin-new/browser-contexts/context-1/clone',
    );
    const exportButton = byTestId(target, 'browser-context-export') as HTMLButtonElement;
    expect(exportButton.disabled).toBe(false);
    expect(byTestId(target, 'browser-context-storage-warning').textContent).toContain(
      'Export remains available for recovery',
    );
    exportButton.click();
    expect(onExportContext).toHaveBeenCalledOnce();
  });

  it('runs refresh and delete actions for deletable contexts', () => {
    const onRefreshContext = vi.fn();
    const onDeleteContext = vi.fn();
    const target = renderComponent(BrowserContextInspector, {
      state: { status: 'ready', context: context() },
      onRefreshContext,
      onDeleteContext,
    });

    byTestId(target, 'browser-context-refresh-detail').click();
    byTestId(target, 'browser-context-delete').click();

    expect(onRefreshContext).toHaveBeenCalledOnce();
    expect(onDeleteContext).toHaveBeenCalledOnce();
  });
});

function context(options: Partial<{
  readonly visibleSessions: number;
  readonly activeRuntimeSessions: number;
  readonly activeRuntimeSessionId: string | null;
  readonly profileStorageLimitExceeded: boolean;
  readonly state: BrowserContextResource['state'];
}> = {}): BrowserContextResource {
  return {
    id: 'context-1',
    project_id: null,
    project: null,
    name: 'Support baseline',
    description: 'Reusable support profile',
    labels: { team: 'support' },
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: 1073741824,
    state: options.state ?? 'ready',
    usage: {
      visible_session_count: options.visibleSessions ?? 0,
      active_runtime_session_count: options.activeRuntimeSessions ?? 0,
      active_runtime_session_id: options.activeRuntimeSessionId ?? null,
      profile_storage_bytes: 1048576,
      profile_storage_limit_exceeded: options.profileStorageLimitExceeded ?? false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
